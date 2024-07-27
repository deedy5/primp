import gzip
import time
from io import BytesIO

from importlib.metadata import version
import pandas as pd
import requests
import httpx
import tls_client
import pycurl
import primp
import curl_cffi.requests


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


PACKAGES = add_package_version(PACKAGES)

requests_number = 2000
for session in [False, True]:
    for response_size in ["5k", "50k", "200k"]:
        url = f"http://127.0.0.1:8000/{response_size}"
        print(f"\n{session=}, {response_size=}, {requests_number=}")
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

df = pd.DataFrame(results)
pivot_df = df.pivot_table(
    index=["name", "session"],
    columns="size",
    values=["time", "cpu_time"],
    aggfunc="mean",
)
pivot_df.reset_index(inplace=True)
pivot_df.columns = [" ".join(col).strip() for col in pivot_df.columns.values]
pivot_df = pivot_df[
    ["name", "session"]
    + [col for col in pivot_df.columns if col not in ["name", "session"]]
]
print(pivot_df)

for session in [False, True]:
    session_df = pivot_df[pivot_df["session"] == session]
    print(f"\nTable for {session=}:")
    print(session_df.to_string(index=False))
    session_df.to_csv(f"{session=}.csv", index=False)
