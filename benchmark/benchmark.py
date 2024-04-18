import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from importlib.metadata import version
import pandas as pd
import requests
import httpx
import tls_client
import pyreqwest_impersonate
import curl_cffi.requests

results = []
PACKAGES = [
    ("requests", requests.Session),
    ("httpx", httpx.Client),
    ("tls_client", tls_client.Session),
    ("curl_cffi", curl_cffi.requests.Session),
    ("pyreqwest_impersonate", pyreqwest_impersonate.Client),
]


def add_package_version(packages):
    return [(f"{name} {version(name)}", classname) for name, classname in packages]


def session_get_test(session_class, requests_number):
    s = session_class()
    for _ in range(requests_number):
        s.get(url).text


PACKAGES = add_package_version(PACKAGES)

# one thread
requests_number = 2000
for response_size in ["5k", "50k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\nOne worker, {response_size=}, {requests_number=}")
    for name, session_class in PACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        session_get_test(session_class, requests_number)
        dur = round(time.perf_counter() - start, 3)
        cpu_dur = round(time.process_time() - cpu_start, 3)
        results.append(
            {
                "name": name,
                "threads": 1,
                "size": response_size,
                "time": dur,
                "cpu_time": cpu_dur,
            }
        )
        print(f"    name: {name:<30} time: {dur} cpu_time: {cpu_dur}")


# multiple threads
requests_number = 500
threads_number = 4
for response_size in ["5k", "50k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\n{threads_number} workers, {response_size=}, {requests_number=}")
    for name, session_class in PACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        with ThreadPoolExecutor(threads_number) as executor:
            futures = [
                executor.submit(session_get_test, session_class, requests_number)
                for _ in range(threads_number)
            ]
            for f in as_completed(futures):
                f.result()
        dur = round(time.perf_counter() - start, 3)
        cpu_dur = round(time.process_time() - cpu_start, 3)
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
pivot_df = pivot_df[
    ["name", "threads"]
    + [col for col in pivot_df.columns if col not in ["name", "threads"]]
]
unique_threads = pivot_df["threads"].unique()
for thread in unique_threads:
    thread_df = pivot_df[pivot_df["threads"] == thread]
    print(f"\nTable for {thread} threads:")
    print(thread_df.to_string(index=False))
    thread_df.to_csv(f"{thread}_threads.csv", index=False)
