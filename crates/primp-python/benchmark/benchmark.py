import asyncio
import time
from importlib.metadata import version
from io import BytesIO

import aiohttp
import curl_cffi.requests
import httpx
import matplotlib.pyplot as plt
import numpy as np
import pycurl
import requests

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
        self.c.setopt(pycurl.ENCODING, "gzip")
        self.c.perform()
        self.content = buffer.getvalue()
        return self

    @property
    def text(self):
        return self.content.decode("utf-8")


# Store results for image generation (nested dict: name -> size -> {time, cpu_time})
results_data = {"sync_no_session": {}, "sync_session": {}, "async_session": {}}

PACKAGES = [
    ("primp", primp.Client),
    ("curl_cffi", curl_cffi.requests.Session),
    ("pycurl", PycurlSession),
    ("requests", requests.Session),
    ("httpx", httpx.Client),
]
AsyncPACKAGES = [
    ("primp", primp.AsyncClient),
    ("curl_cffi", curl_cffi.requests.AsyncSession),
    ("aiohttp", aiohttp.ClientSession),
    ("httpx", httpx.AsyncClient),
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
    async def aget(s, url, semaphore):
        async with semaphore:
            if session_class.__module__ == "aiohttp.client":
                async with s.get(url) as resp:
                    text = await resp.text()
                    return text
            else:
                resp = await s.get(url)
                return resp.text

    if session_class.__module__ == "httpx":
        limits = httpx.Limits(max_connections=100, max_keepalive_connections=100)
        timeout = httpx.Timeout(connect=30.0, read=30.0, write=30.0, pool=30.0)
        semaphore = asyncio.Semaphore(100)
        async with session_class(limits=limits, timeout=timeout) as s:
            tasks = [aget(s, url, semaphore) for _ in range(requests_number)]
            await asyncio.gather(*tasks)
    else:
        semaphore = asyncio.Semaphore(1000)
        async with session_class() as s:
            tasks = [aget(s, url, semaphore) for _ in range(requests_number)]
            await asyncio.gather(*tasks)


def plot_data(data, ax, title):
    """Plot data for a single session type."""
    names = list(data.keys())
    sizes = ["5k", "50k", "200k"]
    
    x = np.arange(len(names))
    width = 0.125  # Narrower bars to fit 6 bars per group
    
    # Plot time bars (first 3)
    for i, size in enumerate(sizes):
        values = [data[name].get(size, {}).get("time", 0) for name in names]
        rects = ax.bar(x + i * width, values, width, label=f"Time {size}")
        ax.bar_label(rects, padding=3, fontsize=8, rotation=90)
    
    # Plot cpu_time bars (next 3)
    for i, size in enumerate(sizes):
        values = [data[name].get(size, {}).get("cpu_time", 0) for name in names]
        rects = ax.bar(x + (i + 3) * width, values, width, label=f"CPU Time {size}")
        ax.bar_label(rects, padding=3, fontsize=8, rotation=90)
    
    ax.set_ylabel("Time (s)", fontsize=10)
    ax.set_title(title, fontsize=11)
    ax.set_xticks(x + 3 * width - width / 2)
    ax.set_xticklabels(names, rotation=0, ha="center", fontsize=9)
    ax.legend(loc="upper left", ncols=6, prop={"size": 8})
    
    # Adjust y-axis
    y_min, y_max = ax.get_ylim()
    ax.set_ylim(y_min, y_max * 1.2)


def generate_image():
    """Generate the benchmark image from in-memory data."""
    fig, (ax1, ax2, ax3) = plt.subplots(3, 1, figsize=(10, 7), layout="constrained")
    
    plot_data(results_data["sync_no_session"], ax1, "Session=False | Requests: 500 | Response: gzip, utf-8, size 5Kb, 50Kb, 200Kb")
    plot_data(results_data["sync_session"], ax2, "Session=True | Requests: 500 | Response: gzip, utf-8, size 5Kb, 50Kb, 200Kb")
    plot_data(results_data["async_session"], ax3, "Session=Async | Requests: 500 | Response: gzip, utf-8, size 5Kb, 50Kb, 200Kb")
    
    plt.savefig("benchmark.jpg", format="jpg", dpi=80, bbox_inches="tight")
    print("\nBenchmark image saved to benchmark.jpg")


PACKAGES = add_package_version(PACKAGES)
AsyncPACKAGES = add_package_version(AsyncPACKAGES)
requests_number = 500

# Sync - No Session
print(f"\n{'='*60}")
print(f"Threads=1, session=False, requests={requests_number}")
print(f"{'='*60}")
for response_size in ["5k", "50k", "200k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\n{response_size}:")
    for name, session_class in PACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        get_test(session_class, requests_number)
        dur = round(time.perf_counter() - start, 2)
        cpu_dur = round(time.process_time() - cpu_start, 2)
        if name not in results_data["sync_no_session"]:
            results_data["sync_no_session"][name] = {}
        results_data["sync_no_session"][name][response_size] = {"time": dur, "cpu_time": cpu_dur}
        print(f"  {name:<30} time: {dur}s cpu_time: {cpu_dur}s")

# Sync - With Session
print(f"\n{'='*60}")
print(f"Threads=1, session=True, requests={requests_number}")
print(f"{'='*60}")
for response_size in ["5k", "50k", "200k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\n{response_size}:")
    for name, session_class in PACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        session_get_test(session_class, requests_number)
        dur = round(time.perf_counter() - start, 2)
        cpu_dur = round(time.process_time() - cpu_start, 2)
        if name not in results_data["sync_session"]:
            results_data["sync_session"][name] = {}
        results_data["sync_session"][name][response_size] = {"time": dur, "cpu_time": cpu_dur}
        print(f"  {name:<30} time: {dur}s cpu_time: {cpu_dur}s")

# Async
print(f"\n{'='*60}")
print(f"Threads=1, session=Async, requests={requests_number}")
print(f"{'='*60}")
for response_size in ["5k", "50k", "200k"]:
    url = f"http://127.0.0.1:8000/{response_size}"
    print(f"\n{response_size}:")
    for name, session_class in AsyncPACKAGES:
        start = time.perf_counter()
        cpu_start = time.process_time()
        asyncio.run(async_session_get_test(session_class, requests_number))
        dur = round(time.perf_counter() - start, 2)
        cpu_dur = round(time.process_time() - cpu_start, 2)
        if name not in results_data["async_session"]:
            results_data["async_session"][name] = {}
        results_data["async_session"][name][response_size] = {"time": dur, "cpu_time": cpu_dur}
        print(f"  {name:<30} time: {dur}s cpu_time: {cpu_dur}s")

# Generate image from in-memory data
generate_image()

