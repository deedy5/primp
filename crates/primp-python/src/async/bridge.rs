//! Rust-future → Python-coroutine bridge.
//!
//! `future_into_py` returns a bare `asyncio.Future` (rejected by
//! `asyncio.create_task`) and leaks its tokio task past interpreter teardown
//! (a pending future polled post-finalize panics; release builds abort).
//! This returns a real coroutine and tracks every task for an exit hook.

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use pyo3::exceptions::PyBaseException;
use pyo3::prelude::*;
use pyo3::sync::PyOnceLock;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use pyo3_async_runtimes::tokio::{get_current_locals, get_runtime, scope};
use tokio::sync::oneshot;

/// JoinHandles of in-flight bridge tasks; aborted by the interpreter-exit hook.
static ACTIVE_TASKS: Mutex<Vec<tokio::task::JoinHandle<()>>> = Mutex::new(Vec::new());

/// Set by the exit hook before any abort; delivery closures started after this
/// skip touching Python entirely (the interpreter is being torn down).
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Delivery closures on the blocking pool; the exit hook waits for zero
/// (GIL released) so none touches Python after finalize.
static ACTIVE_DELIVERIES: AtomicUsize = AtomicUsize::new(0);

/// `async def _wrap_awaitable(fut)` — creates real coroutines from futures.
static WRAPPER: PyOnceLock<Py<PyAny>> = PyOnceLock::new();

/// Called by `asyncio.Future.add_done_callback`: cancels the Rust future when
/// the Python future is cancelled (mirrors pyo3-async-runtimes' PyDoneCallback).
#[pyclass]
struct DoneCallback {
    cancel_tx: Option<oneshot::Sender<()>>,
}

#[pymethods]
impl DoneCallback {
    fn __call__(&mut self, future: &Bound<'_, PyAny>) -> PyResult<()> {
        let cancelled = future
            .call_method0(pyo3::intern!(future.py(), "cancelled"))?
            .is_truthy()?;
        if cancelled {
            if let Some(tx) = self.cancel_tx.take() {
                let _ = tx.send(());
            }
        }
        Ok(())
    }
}

/// Completion callback scheduled on the event loop: sets the result unless the
/// future was cancelled in the meantime (mirrors the crate's CheckedCompletor).
#[pyclass]
struct Completer;

#[pymethods]
impl Completer {
    #[pyo3(signature = (future, complete, value))]
    fn __call__(
        &self,
        future: &Bound<'_, PyAny>,
        complete: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let cancelled = future
            .call_method0(pyo3::intern!(future.py(), "cancelled"))?
            .is_truthy()?;
        if cancelled {
            return Ok(());
        }
        complete.call1((value,))?;
        Ok(())
    }
}

/// Interpreter-exit hook: abort in-flight tasks, then wait (GIL released)
/// for running deliveries — else a closure polls pyo3 after finalize
/// (`panic = "abort"` in release → SIGABRT at exit).
#[pyfunction]
pub fn _primp_shutdown(py: Python<'_>) {
    // Stop `log!` forwarding: a tokio-worker log after finalize reaches
    // pyo3_log's `Python::attach` and panics.
    log::set_max_level(log::LevelFilter::Off);

    SHUTTING_DOWN.store(true, Ordering::SeqCst);
    let tasks = {
        let mut guard = ACTIVE_TASKS.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *guard)
    };
    for handle in tasks {
        handle.abort();
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let _: () = py.detach(|| {
        while ACTIVE_DELIVERIES.load(Ordering::SeqCst) > 0 {
            if std::time::Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    });
}

/// Register the wrapper coroutine and the exit hook (called from module init).
pub fn init(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let dict = m.dict();
    py.run(
        c"import asyncio
async def _wrap_awaitable(fut):
    try:
        return await fut
    except asyncio.CancelledError:
        fut.cancel()
        raise
",
        Some(&dict),
        Some(&dict),
    )?;

    let _ = WRAPPER.set(py, m.getattr("_wrap_awaitable")?.unbind());
    m.add_function(wrap_pyfunction!(_primp_shutdown, m)?)?;
    py.import("atexit")?
        .call_method1("register", (m.getattr("_primp_shutdown")?,))?;
    Ok(())
}

/// Deliver the future's result on the event loop, preserving contextvars.
fn set_result(
    event_loop: &Bound<'_, PyAny>,
    context: &Bound<'_, PyAny>,
    future: &Bound<'_, PyAny>,
    result: PyResult<Py<PyAny>>,
) -> PyResult<()> {
    let py = event_loop.py();
    let kwargs = PyDict::new(py);
    kwargs.set_item(pyo3::intern!(py, "context"), context)?;

    let (complete, value) = match result {
        Ok(val) => (future.getattr(pyo3::intern!(py, "set_result"))?, Ok(val)),
        Err(err) => (
            future.getattr(pyo3::intern!(py, "set_exception"))?,
            err.into_bound_py_any(py).map(Into::into),
        ),
    };

    event_loop.call_method(
        pyo3::intern!(py, "call_soon_threadsafe"),
        (Completer, future, complete, value?),
        Some(&kwargs),
    )?;
    Ok(())
}

/// Error type produced by a bridge task body, delivered to the guarded
/// closure for GIL-side conversion.
pub(crate) trait BridgeError: Send + 'static {
    /// Convert into a Python exception; called with the GIL held inside the
    /// delivery closure (never from the tokio worker).
    fn into_pyerr(self, py: Python<'_>) -> PyErr;
    /// Error for the cancellation branch of the task's `select!`.
    fn cancelled_err() -> Self;
}

/// `PyErr` values built GIL-free in the task body (`new_err`/`PyErr::new`
/// need no interpreter access) are delivered as-is.
impl BridgeError for PyErr {
    fn into_pyerr(self, _py: Python<'_>) -> PyErr {
        self
    }
    fn cancelled_err() -> Self {
        PyBaseException::new_err("primp: python future cancelled before completion")
    }
}

/// Task-body error carrying either an already-built (GIL-free) `PyErr` or a
/// Rust error deferred for conversion inside the guarded delivery closure —
/// error conversion must never run `Python::attach` from a tokio worker
/// (post-finalize attach is the documented abort class; the delivery wait
/// guarantees no closure touches Python after finalize).
#[derive(Debug)]
pub(crate) enum BridgeTaskError {
    Ready(PyErr),
    Deferred(crate::error::PrimpErrorEnum),
}

impl BridgeError for BridgeTaskError {
    fn into_pyerr(self, py: Python<'_>) -> PyErr {
        match self {
            BridgeTaskError::Ready(err) => err,
            BridgeTaskError::Deferred(err) => crate::error::convert_primp_error(py, err),
        }
    }
    fn cancelled_err() -> Self {
        BridgeTaskError::Ready(PyBaseException::new_err(
            "primp: python future cancelled before completion",
        ))
    }
}

/// Convert a Rust future into a Python coroutine (awaitable, cancellable,
/// `asyncio.create_task`-compatible) instead of a bare asyncio.Future.
pub fn future_into_coroutine<F, T, E>(py: Python<'_>, fut: F) -> PyResult<Bound<'_, PyAny>>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: for<'a> IntoPyObject<'a> + Send + 'static,
    E: BridgeError,
{
    use pyo3::IntoPyObjectExt;

    let locals = get_current_locals(py)?;
    let event_loop = locals.event_loop(py);

    let py_fut = event_loop.call_method0(pyo3::intern!(py, "create_future"))?;

    let (cancel_tx, cancel_rx) = oneshot::channel();
    py_fut.call_method1(
        pyo3::intern!(py, "add_done_callback"),
        (DoneCallback {
            cancel_tx: Some(cancel_tx),
        },),
    )?;

    let context = locals.context(py);
    let future_tx: Py<PyAny> = py_fut.clone().into();
    let event_loop_py: Py<PyAny> = event_loop.clone().unbind();
    let context_py: Py<PyAny> = context.unbind();

    let handle = get_runtime().spawn(scope(locals, async move {
        let result = tokio::select! {
            r = fut => r,
            _ = cancel_rx => Err(E::cancelled_err()),
        };

        let _ = get_runtime()
            .spawn_blocking(move || {
                // Increment BEFORE the shutdown check so the exit hook's
                // wait cannot observe zero mid-delivery; aborted-but-queued
                // closures skip Python via the flag.
                ACTIVE_DELIVERIES.fetch_add(1, Ordering::SeqCst);
                if SHUTTING_DOWN.load(Ordering::SeqCst) {
                    ACTIVE_DELIVERIES.fetch_sub(1, Ordering::SeqCst);
                    return;
                }
                let delivered: Result<(), PyErr> = Python::attach(move |py| {
                    let py_fut = future_tx.bind(py);
                    let cancelled = py_fut
                        .call_method0(pyo3::intern!(py, "cancelled"))
                        .and_then(|c| c.is_truthy())
                        .unwrap_or(false);
                    if cancelled {
                        return Ok(());
                    }
                    // Convert errors here, under the GIL and inside the
                    // shutdown-guarded delivery — never on the tokio worker.
                    let result = match result {
                        Ok(val) => match val.into_py_any(py) {
                            Ok(value) => Ok(value),
                            Err(err) => Err(err),
                        },
                        Err(err) => Err(err.into_pyerr(py)),
                    };
                    let delivered = match result {
                        Ok(value) => set_result(
                            event_loop_py.bind(py),
                            context_py.bind(py),
                            py_fut,
                            Ok(value),
                        ),
                        Err(err) => set_result(
                            event_loop_py.bind(py),
                            context_py.bind(py),
                            py_fut,
                            Err(err),
                        ),
                    };
                    if let Err(e) = delivered {
                        e.print_and_set_sys_last_vars(py);
                    }
                    Ok(())
                });
                // `attach` fails (not panics) when the interpreter cannot be
                // entered (rare shutdown race); nothing to deliver then.
                let _ = delivered;
                ACTIVE_DELIVERIES.fetch_sub(1, Ordering::SeqCst);
            })
            .await;
    }));

    let mut tasks = ACTIVE_TASKS.lock().unwrap_or_else(|e| e.into_inner());
    tasks.retain(|h| !h.is_finished());
    tasks.push(handle);

    let wrapper = WRAPPER.get(py).ok_or_else(|| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("primp: async bridge not initialized")
    })?;
    wrapper.bind(py).call1((py_fut,))
}
