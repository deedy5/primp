![Python >= 3.8](https://img.shields.io/badge/python->=3.8-red.svg) [![](https://badgen.net/github/release/deedy5/pyreqwest-impersonate)](https://github.com/deedy5/pyreqwest-impersonate/releases) [![](https://badge.fury.io/py/primp.svg)](https://pypi.org/project/primp) [![Downloads](https://static.pepy.tech/badge/primp/week)](https://pepy.tech/project/primp) [![CI](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml)
# 🪞PRIMP
**🪞PRIMP** = **P**ython **R**equests **IMP**ersonate

The fastest python HTTP client that can impersonate web browsers.</br>
Provides precompiled wheels:</br>
  * 🐧 linux: `amd64`, `aarch64`, `armv7` (⚠️aarch64 and armv7 builds are `manylinux_2_34` compatible - `ubuntu>=22.04`, `debian>=12`);</br>
  * 🐧 musllinux: `amd64`, `aarch64`;</br>
  * 🪟 windows: `amd64`;</br>
  * 🍏 macos: `amd64`, `aarch64`.</br>

## Table of Contents

- [Installation](#installation)
- [Benchmark](#benchmark)
- [Usage](#usage)
  - [I. Client](#i-client)
    - [Client methods](#client-methods)
    - [Response object](#response-object)
    - [Devices](#devices)
    - [Examples](#examples)
  - [II. AsyncClient](#ii-asyncclient)
- [Disclaimer](#disclaimer)

## Installation

```python
pip install -U primp
```

## Benchmark

![](https://github.com/deedy5/primp/blob/main/benchmark.jpg?raw=true)

## Usage
### I. Client

HTTP client that can impersonate web browsers.
```python
class Client:
    """Initializes an HTTP client that can impersonate web browsers.

    Args:
        auth (tuple[str, str| None] | None): Username and password for basic authentication. Default is None.
        auth_bearer (str | None): Bearer token for authentication. Default is None.
        params (dict[str, str] | None): Default query parameters to include in all requests. Default is None.
        headers (dict[str, str] | None): Default headers to send with requests. If `impersonate` is set, this will be ignored.
        timeout (float | None): HTTP request timeout in seconds. Default is 30.
        cookie_store (bool | None): Enable a persistent cookie store. Received cookies will be preserved and included
            in additional requests. Default is True.
        referer (bool | None): Enable or disable automatic setting of the `Referer` header. Default is True.
        proxy (str | None): Proxy URL for HTTP requests. Example: "socks5://127.0.0.1:9150". Default is None.
        impersonate (str | None): Entity to impersonate. Example: "chrome_124". Default is None.
            Chrome: "chrome_100","chrome_101","chrome_104","chrome_105","chrome_106","chrome_107","chrome_108",
                "chrome_109","chrome_114","chrome_116","chrome_117","chrome_118","chrome_119","chrome_120",
                "chrome_123","chrome_124","chrome_126","chrome_127","chrome_128","chrome_129","chrome_130",
                "chrome_131","chrome_133"
            Safari: "safari_ios_16.5","safari_ios_17.2","safari_ios_17.4.1","safari_ios_18.1.1",
                "safari_15.3","safari_15.5","safari_15.6.1","safari_16","safari_16.5","safari_17.0",
                "safari_17.2.1","safari_17.4.1","safari_17.5","safari_18","safari_18.2","safari_ipad_18"
            OkHttp: "okhttp_3.9","okhttp_3.11","okhttp_3.13","okhttp_3.14","okhttp_4.9","okhttp_4.10","okhttp_5"
            Edge: "edge_101","edge_122","edge_127","edge_131"
            Firefox: "firefox_109","firefox_117","firefox_128","firefox_133","firefox_135"
            Select random: "random"
        impersonate_os (str | None): impersonate OS. Example: "windows". Default is "linux".
            Android: "android", iOS: "ios", Linux: "linux", Mac OS: "macos", Windows: "windows"
            Select random: "random"
        follow_redirects (bool | None): Whether to follow redirects. Default is True.
        max_redirects (int | None): Maximum redirects to follow. Default 20. Applies if `follow_redirects` is True.
        verify (bool | None): Verify SSL certificates. Default is True.
        ca_cert_file (str | None): Path to CA certificate store. Default is None.
        https_only` (bool | None): Restrict the Client to be used with HTTPS only requests. Default is `false`.
        http2_only` (bool | None): If true - use only HTTP/2; if false - use only HTTP/1. Default is `false`.

    """
```

#### Client methods

The `Client` class provides a set of methods for making HTTP requests: `get`, `head`, `options`, `delete`, `post`, `put`, `patch`, each of which internally utilizes the `request()` method for execution. The parameters for these methods closely resemble those in `httpx`.
```python
def get(
    url: str,
    params: dict[str, str] | None = None,
    headers: dict[str, str] | None = None,
    cookies: dict[str, str] | None = None,
    auth: tuple[str, str| None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = 30,
):
    """Performs a GET request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (dict[str, str] | None): A map of query parameters to append to the URL. Default is None.
        headers (dict[str, str] | None): A map of HTTP headers to send with the request. Default is None.
        cookies (dict[str, str] | None): - An optional map of cookies to send with requests as the `Cookie` header.
        auth (tuple[str, str| None] | None): A tuple containing the username and an optional password
            for basic authentication. Default is None.
        auth_bearer (str | None): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (float | None): The timeout for the request in seconds. Default is 30.

    """
```
```python
def post(
    url: str,
    params: dict[str, str] | None = None,
    headers: dict[str, str] | None = None,
    cookies: dict[str, str] | None = None,
    content: bytes | None = None,
    data: dict[str, Any] | None = None,
    json: Any | None = None,
    files: dict[str, str] | None = None,
    auth: tuple[str, str| None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = 30,
):
    """Performs a POST request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (dict[str, str] | None): A map of query parameters to append to the URL. Default is None.
        headers (dict[str, str] | None): A map of HTTP headers to send with the request. Default is None.
        cookies (dict[str, str] | None): - An optional map of cookies to send with requests as the `Cookie` header.
        content (bytes | None): The content to send in the request body as bytes. Default is None.
        data (dict[str, Any] | None): The form data to send in the request body. Default is None.
        json (Any | None): A JSON serializable object to send in the request body. Default is None.
        files (dict[str, str] | None): A map of file fields to file paths to be sent as multipart/form-data. Default is None.
        auth (tuple[str, str| None] | None): A tuple containing the username and an optional password
            for basic authentication. Default is None.
        auth_bearer (str | None): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (float | None): The timeout for the request in seconds. Default is 30.

    """
```
#### Response object
```python
resp.content
resp.stream()  # stream the response body in chunks of bytes
resp.cookies
resp.encoding
resp.headers
resp.json()
resp.status_code
resp.text
resp.text_markdown  # html is converted to markdown text
resp.text_plain  # html is converted to plain text
resp.text_rich  # html is converted to rich text
resp.url
```

#### Devices

##### Impersonate

- Chrome: `chrome_100`，`chrome_101`，`chrome_104`，`chrome_105`，`chrome_106`，`chrome_107`，`chrome_108`，`chrome_109`，`chrome_114`，`chrome_116`，`chrome_117`，`chrome_118`，`chrome_119`，`chrome_120`，`chrome_123`，`chrome_124`，`chrome_126`，`chrome_127`，`chrome_128`，`chrome_129`，`chrome_130`，`chrome_131`, `chrome_133`

- Edge: `edge_101`，`edge_122`，`edge_127`, `edge_131`

- Safari: `safari_ios_17.2`，`safari_ios_17.4.1`，`safari_ios_16.5`，`safari_ios_18.1.1`, `safari_15.3`，`safari_15.5`，`safari_15.6.1`，`safari_16`，`safari_16.5`，`safari_17.0`，`safari_17.2.1`，`safari_17.4.1`，`safari_17.5`，`safari_18`，`safari_18.2`, `safari_ipad_18`

- OkHttp: `okhttp_3.9`，`okhttp_3.11`，`okhttp_3.13`，`okhttp_3.14`，`okhttp_4.9`，`okhttp_4.10`，`okhttp_5`

- Firefox: `firefox_109`, `firefox_117`, `firefox_128`, `firefox_133`, `firefox_135`

##### Impersonate OS

- `android`, `ios`, `linux`, `macos`, `windows`

#### Examples

```python
import primp

# Impersonate
client = primp.Client(impersonate="chrome_131", impersonate_os="windows")

# Update headers
headers = {"Referer": "https://cnn.com/"}
client.headers_update(headers)

# GET request
resp = client.get("https://tls.peet.ws/api/all")

# GET request with passing params and setting timeout
params = {"param1": "value1", "param2": "value2"}
resp = client.post(url="https://httpbin.org/anything", params=params, timeout=10)

# Stream response
resp = client.get("https://nytimes")
for chunk in resp.stream():
    print(chunk)

# Cookies set
cookies = {"c1_n": "c1_value", "c2_n": "c2_value"}
client.set_cookies(url="https://nytimes.com", cookies) # set cookies for a specific domain
client.get("https://nytimes.com/", cookies=cookies)  # set cookies in request

# Cookies get
cookies = client.get_cookies(url="https://nytimes.com")  # get cookies for a specific domain
cookies = resp.cookies  # get cookies from response

# POST Binary Request Data
content = b"some_data"
resp = client.post(url="https://httpbin.org/anything", content=content)

# POST Form Encoded Data
data = {"key1": "value1", "key2": "value2"}
resp = client.post(url="https://httpbin.org/anything", data=data)

# POST JSON Encoded Data
json = {"key1": "value1", "key2": "value2"}
resp = client.post(url="https://httpbin.org/anything", json=json)

# POST Multipart-Encoded Files
files = {'file1': '/home/root/file1.txt', 'file2': 'home/root/file2.txt'}
resp = client.post("https://httpbin.org/post", files=files)

# Authentication using user/password
resp = client.post(url="https://httpbin.org/anything", auth=("user", "password"))

# Authentication using auth bearer
resp = client.post(url="https://httpbin.org/anything", auth_bearer="bearerXXXXXXXXXXXXXXXXXXXX")

# Using proxy or env var PRIMP_PROXY
resp = primp.Client(proxy="http://127.0.0.1:8080")  # set proxy in Client
export PRIMP_PROXY="socks5://127.0.0.1:1080"  # set proxy as environment variable

# Using custom CA certificate store:
#(!!!Primp already built with the Mozilla's latest trusted root certificates)
resp = primp.Client(ca_cert_file="/cert/cacert.pem")
resp = primp.Client(ca_cert_file=certifi.where())
export PRIMP_CA_BUNDLE="/home/user/Downloads/cert.pem"  # set as environment variable

# You can also use convenience functions that use a default Client instance under the hood:
# primp.get() | primp.head() | primp.options() | primp.delete() | primp.post() | primp.patch() | primp.put()
# These functions can accept the `impersonate` parameter:
resp = primp.get("https://httpbin.org/anything", impersonate="chrome_131", impersonate_os="android")
```

### II. AsyncClient

`primp.AsyncClient()` is an asynchronous wrapper around the `primp.Client` class, offering the same functions, behavior, and input arguments.

```python3
import asyncio
import logging

import primp

async def aget_text(url):
    async with primp.AsyncClient(impersonate="chrome_131") as client:
        resp = await client.get(url)
        return resp.text

async def main():
    urls = ["https://nytimes.com/", "https://cnn.com/", "https://abcnews.go.com/"]
    tasks = [aget_text(u) for u in urls]
    results = await asyncio.gather(*tasks)

if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    asyncio.run(main())
```

## Disclaimer

This tool is for educational purposes only. Use it at your own risk.
