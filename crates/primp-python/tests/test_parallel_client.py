"""Regression test for a first-call deadlock on parallel Client construction."""

from concurrent.futures import ThreadPoolExecutor
import threading
import asyncio

import primp


class AsyncBarrier:
    """Simple async barrier implementation for Python <3.11 compatibility."""
    def __init__(self, parties):
        self.parties = parties
        self._count = 0
        self._event = asyncio.Event()
        self._lock = asyncio.Lock()

    async def wait(self):
        async with self._lock:
            self._count += 1
            if self._count == self.parties:
                self._event.set()
        await self._event.wait()


def test_parallel_client_new_does_not_deadlock():
    num_threads = 8
    barrier = threading.Barrier(num_threads)

    def worker() -> str:
        barrier.wait()
        try:
            primp.Client(impersonate="random", impersonate_os="random", timeout=5)
            return "ok"
        except Exception as exc:
            return f"err:{exc!r}"

    with ThreadPoolExecutor(max_workers=num_threads) as executor:
        futures = [executor.submit(worker) for _ in range(num_threads)]
        results = [f.result(timeout=20) for f in futures]

    assert results == ["ok"] * num_threads, results


async def test_parallel_asyncclient_new_does_not_deadlock():
    num_tasks = 8
    barrier = AsyncBarrier(num_tasks)

    async def worker() -> str:
        await barrier.wait()
        try:
            primp.AsyncClient(impersonate="random", impersonate_os="random", timeout=5)
            return "ok"
        except Exception as exc:
            return f"err:{exc!r}"

    results = await asyncio.gather(*[worker() for _ in range(num_tasks)])
    assert results == ["ok"] * num_tasks, results