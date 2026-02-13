import asyncio
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from importlib.metadata import version
from io import BytesIO

import aiohttp
import curl_cffi.requests
import httpx
import pandas as pd
import pycurl
import requests
import tls_client

import primp


class PycurlSession:
    def __init__(self):
        self.c = pycurl.Curl()
        self.content = None

    def __del__(self):
        self.close()

    def close(self):
        self.c.close()

    def get(self, url):
        buffer = BytesIO()
        self.c.setopt(pycurl.URL, url)
        self.c.setopt(pycurl.WRITEDATA, buffer)
        self.c.setopt(pycurl.ENCODING, "gzip")  # Automatically handle gzip encoding
        self.c.perform()
        self.content = buffer.getvalue()
        return self

    @property
    def text(self):
        return self.content.decode("utf-8")


results = []
PACKAGES = [
    ("requests", requests.Session),
    ("httpx", httpx.Client),
    ("tls_client", tls_client.Session),
    ("curl_cffi", curl_cffi.requests.Session),
    ("pycurl", PycurlSession),
    ("primp", primp.Client),
]
AsyncPACKAGES = [
    ("aiohttp", aiohttp.ClientSession),
    ("httpx", httpx.AsyncClient),
    ("curl_cffi", curl_cffi.requests.AsyncSession),
    ("primp", primp.AsyncClient),
]


def add_package_version(packages):
    return [(f"{name} {version(name)}", classname) for name, classname in packages]


def get_test(session_class, requests_number):
    for _ in range(requests_number):
        s = session_class()
        try:
            s.get(url).text
        finally:
            if hasattr(s, "close"):
                s.close()


def session_get_test(session_class, requests_number):
    s = session_class()
    try:
        for _ in range(requests_number):
            s.get(url).text
    finally:
        if hasattr(s, "close"):
            s.close()


async def async_session_get_test(session_class, requests_number):
    async def aget(s, url):
        if session_class.__module__ == "aiohttp.client":
            async with s.get(url) as resp:
                text = await resp.text()
                return text
        else:
            resp = await s.get(url)
            return resp.text

    async with session_class() as s:
        tasks = [aget(s, url) for _ in range(requests_number)]
        await asyncio.gather(*tasks)


PACKAGES = add_package_version(PACKAGES)
AsyncPACKAGES = add_package_version(AsyncPACKAGES)
requests_number = 400

# Sync
for session in [False, True]:
    for response_size in ["5k", "50k", "200k"]:
        url = f"http://127.0.0.1:8000/{response_size}"
        print(f"\nThreads=1, {session=}, {response_size=}, {requests_number=}")
        for name, session_class in PACKAGES:
            start = time.perf_counter()
            cpu_start = time.process_time()
            if session:
                session_get_test(session_class, requests_number)
            else:
                get_test(session_class, requests_number)
            dur = round(time.perf_counter() - start, 2)
            cpu_dur = round(time.process_time() - cpu_start, 2)
            results.append(
                {
                    "name": name,
                    "session": session,
                    "size": response_size,
                    "time": dur,
                    "cpu_time": cpu_dur,
                }
            )
            print(f"    name: {name:<30} time: {dur} cpu_time: {cpu_dur}")

# Async
for response_size in ["5k", "50k", "200k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\nThreads=1, session=Async, {response_size=}, {requests_number=}")

    for name, session_class in AsyncPACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        asyncio.run(async_session_get_test(session_class, requests_number))
        dur = round(time.perf_counter() - start, 2)
        cpu_dur = round(time.process_time() - cpu_start, 2)

        results.append(
            {
                "name": name,
                "session": "Async",
                "size": response_size,
                "time": dur,
                "cpu_time": cpu_dur,
            }
        )

        print(f"    name: {name:<30} time: {dur} cpu_time: {cpu_dur}")

df = pd.DataFrame(results)
pivot_df = df.pivot_table(
    index=["name", "session"],
    columns="size",
    values=["time", "cpu_time"],
    aggfunc="mean",
)
pivot_df.reset_index(inplace=True)
pivot_df.columns = [" ".join(col).strip() for col in pivot_df.columns.values]
pivot_df = pivot_df[["name", "session"] + [col for col in pivot_df.columns if col not in ["name", "session"]]]
print(pivot_df)

for session in [False, True, "Async"]:
    session_df = pivot_df[pivot_df["session"] == session]
    print(f"\nThreads=1 {session=}:")
    print(session_df.to_string(index=False))
    session_df.to_csv(f"{session=}.csv", index=False)

########################################################
# Not for generating image, just to check multithreading working
# Multiple threads
threads_numbers = [5, 32]
for threads_number in threads_numbers:
    for response_size in ["5k", "50k", "200k"]:
        url = f"http://127.0.0.1:8000/{response_size}"
        print(f"\nThreads={threads_number}, session=True, {response_size=}, {requests_number=}")
        for name, session_class in PACKAGES:
            start = time.perf_counter()
            cpu_start = time.process_time()
            with ThreadPoolExecutor(threads_number) as executor:
                futures = [
                    executor.submit(
                        session_get_test,
                        session_class,
                        int(requests_number / threads_number),
                    )
                    for _ in range(threads_number)
                ]
                for f in as_completed(futures):
                    f.result()
            dur = round(time.perf_counter() - start, 2)
            cpu_dur = round(time.process_time() - cpu_start, 2)
            results.append(
                {
                    "name": name,
                    "threads": threads_number,
                    "size": response_size,
                    "time": dur,
                    "cpu_time": cpu_dur,
                }
            )
            print(f"    name: {name:<30} time: {dur} cpu_time: {cpu_dur}")


df = pd.DataFrame(results)
pivot_df = df.pivot_table(
    index=["name", "threads"],
    columns="size",
    values=["time", "cpu_time"],
    aggfunc="mean",
)
pivot_df.reset_index(inplace=True)
pivot_df.columns = [" ".join(col).strip() for col in pivot_df.columns.values]
pivot_df = pivot_df[["name", "threads"] + [col for col in pivot_df.columns if col not in ["name", "threads"]]]
unique_threads = pivot_df["threads"].unique()
for thread in unique_threads:
    thread_df = pivot_df[pivot_df["threads"] == thread]
    print(f"\nThreads={thread} session=True")
    print(thread_df.to_string(index=False))
