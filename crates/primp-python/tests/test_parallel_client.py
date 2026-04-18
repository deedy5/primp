"""Regression test for a first-call deadlock on parallel Client construction.

Several threads concurrently constructing ``primp.Client(impersonate="random")``
used to deadlock: each construction runs TLS / root-CA / impersonation setup
in Rust, and the log/tracing callsites inside reqwest + hyper + rustls
forward through ``pyo3-log`` back into Python's ``logging.getLogger``. With
every thread holding the GIL across the whole builder, those re-entries
serialized against Python's ``logging._lock`` and the GIL in a way that
stalled indefinitely (faulthandler showed workers parked inside
``logging/__init__.py:getLogger`` while inside ``primp.Client()``).

The fix releases the GIL across the reqwest build (``py.detach``) in
``Client::new`` / ``AsyncClient::new``, which is the pattern documented in
vorner/pyo3-log#65. This test locks in that behaviour.
"""
import threading

import primp


def test_parallel_client_new_does_not_deadlock():
    num_threads = 8
    barrier = threading.Barrier(num_threads)
    results: list[str] = []
    results_lock = threading.Lock()

    def worker() -> None:
        barrier.wait()
        try:
            primp.Client(impersonate="random", impersonate_os="random", timeout=5)
            status = "ok"
        except Exception as exc:  # pragma: no cover - diagnostic only
            status = f"err:{exc!r}"
        with results_lock:
            results.append(status)

    threads = [
        threading.Thread(target=worker, name=f"W{i}") for i in range(num_threads)
    ]
    for t in threads:
        t.start()
    for t in threads:
        t.join(timeout=20)

    stuck = [t.name for t in threads if t.is_alive()]
    assert not stuck, f"deadlock: threads still alive after 20s: {stuck}"
    assert results == ["ok"] * num_threads, results
