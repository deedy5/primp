![Python >= 3.8](https://img.shields.io/badge/python->=3.8-red.svg) [![](https://badgen.net/github/release/deedy5/pyreqwest-impersonate)](https://github.com/deedy5/pyreqwest-impersonate/releases) [![](https://badge.fury.io/py/primp.svg)](https://pypi.org/project/primp) [![Downloads](https://static.pepy.tech/badge/primp/week)](https://pepy.tech/project/primp) [![CI](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml)
# 🪞PRIMP
**🪞PRIMP** = **P**ython **R**equests **IMP**ersonate

The fastest python HTTP client that can impersonate web browsers.</br>
Provides precompiled wheels:</br>
  * 🐧 linux|musl: `amd64`, `aarch64` (⚠️warning: linux `aarch64` build is `manylinux_2_34` compatible - `ubuntu>=22.04`, `debian>=12`);</br> 
  * 🪟 windows: `amd64`;</br>
  * 🍏 macos: `amd64`, `aarch64`.</br>

## Table of Contents

- [Installation](#installation)
- [Benchmark](#benchmark)
- [Usage](#usage)
  - [I. Client](#i-client)
    - [Client methods](#client-methods)
    - [Response object](#response-object)
    - [Examples](#examples)
  - [II. AsyncClient](#ii-asyncclient)

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
        auth (tuple, optional): A tuple containing the username and password for basic authentication. Default is None.
        auth_bearer (str, optional): Bearer token for authentication. Default is None.
        params (dict, optional): Default query parameters to include in all requests. Default is None.
        headers (dict, optional): Default headers to send with requests. If `impersonate` is set, this will be ignored.
        cookies (dict, optional): - An optional map of cookies to send with requests as the `Cookie` header.
        timeout (float, optional): HTTP request timeout in seconds. Default is 30.
        cookie_store (bool, optional): Enable a persistent cookie store. Received cookies will be preserved and included 
            in additional requests. Default is True.
        referer (bool, optional): Enable or disable automatic setting of the `Referer` header. Default is True.
        proxy (str, optional): Proxy URL for HTTP requests. Example: "socks5://127.0.0.1:9150". Default is None.
        impersonate (str, optional): Entity to impersonate. Example: "chrome_124". Default is None.
            Chrome: "chrome_100","chrome_101","chrome_104","chrome_105","chrome_106","chrome_107","chrome_108",
                "chrome_109","chrome_114","chrome_116","chrome_117","chrome_118","chrome_119","chrome_120",
                "chrome_123","chrome_124","chrome_126","chrome_127","chrome_128","chrome_129","chrome_130"
            Safari: "safari_ios_16.5","safari_ios_17.2","safari_ios_17.4.1","safari_15.3","safari_15.5","safari_15.6.1",
                "safari_16","safari_16.5","safari_17.0","safari_17.2.1","safari_17.4.1","safari_17.5","safari_18", 
                "safari_ipad_18"
            OkHttp: "okhttp_3.9","okhttp_3.11","okhttp_3.13","okhttp_3.14","okhttp_4.9","okhttp_4.10","okhttp_5"
            Edge: "edge_101","edge_122","edge_127"
        follow_redirects (bool, optional): Whether to follow redirects. Default is True.
        max_redirects (int, optional): Maximum redirects to follow. Default 20. Applies if `follow_redirects` is True.
        verify (bool, optional): Verify SSL certificates. Default is True.
        ca_cert_file (str, optional): Path to CA certificate store. Default is None.
        http1 (bool, optional): Use only HTTP/1.1. Default is None.
        http2 (bool, optional): Use only HTTP/2. Default is None.
         
    """
```

#### Client methods

The `Client` class provides a set of methods for making HTTP requests: `get`, `head`, `options`, `delete`, `post`, `put`, `patch`, each of which internally utilizes the `request()` method for execution. The parameters for these methods closely resemble those in `httpx`.
```python
def get(
    url: str, 
    params: Optional[Dict[str, str]] = None, 
    headers: Optional[Dict[str, str]] = None, 
    cookies: Optional[Dict[str, str]] = None, 
    auth: Optional[Tuple[str, Optional[str]]] = None, 
    auth_bearer: Optional[str] = None, 
    timeout: Optional[float] = 30,
):
    """Performs a GET request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
        headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
        cookies (Optional[Dict[str, str]]): - An optional map of cookies to send with requests as the `Cookie` header.
        auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password 
            for basic authentication. Default is None.
        auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (Optional[float]): The timeout for the request in seconds. Default is 30.

    """
```
```python
def post(
    url: str, 
    params: Optional[Dict[str, str]] = None, 
    headers: Optional[Dict[str, str]] = None, 
    cookies: Optional[Dict[str, str]] = None, 
    content: Optional[bytes] = None, 
    data: Optional[Dict[str, str]] = None, 
    json: Any = None, 
    files: Optional[Dict[str, bytes]] = None, 
    auth: Optional[Tuple[str, Optional[str]]] = None, 
    auth_bearer: Optional[str] = None, 
    timeout: Optional[float] = 30,
):
    """Performs a POST request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
        headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
        cookies (Optional[Dict[str, str]]): - An optional map of cookies to send with requests as the `Cookie` header.
        content (Optional[bytes]): The content to send in the request body as bytes. Default is None.
        data (Optional[Dict[str, str]]): The form data to send in the request body. Default is None.
        json (Any): A JSON serializable object to send in the request body. Default is None.
        files (Optional[Dict[str, bytes]]): A map of file fields to file contents to be sent as multipart/form-data. Default is None.
        auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password 
            for basic authentication. Default is None.
        auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (Optional[float]): The timeout for the request in seconds. Default is 30.

    """
```
#### Response object
```python
resp.content
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

#### Examples

```python
import primp

client = primp.Client(impersonate="chrome_127")

# GET request
resp = client.get("https://tls.peet.ws/api/all")
print(resp.json())

# GET request with passing params and setting timeout
params = {"param1": "value1", "param2": "value2"}
resp = client.post(url="https://httpbin.org/anything", params=params, timeout=10)
print(r.text)

# POST Binary Request Data
content = b"some_data"
resp = client.post(url="https://httpbin.org/anything", content=content)
print(r.text)

# POST Form Encoded Data
data = {"key1": "value1", "key2": "value2"}
resp = client.post(url="https://httpbin.org/anything", data=data)
print(r.text)

# POST JSON Encoded Data
json = {"key1": "value1", "key2": "value2"}
resp = client.post(url="https://httpbin.org/anything", json=json)
print(r.text)

# POST Multipart-Encoded Files
files = {'file1': open('file1.txt', 'rb').read(), 'file2': open('file2.txt', 'rb').read()}
r = client.post("https://httpbin.org/post", files=files)
print(r.text)

# Authentication using user/password
auth = ("user", "password")
resp = client.post(url="https://httpbin.org/anything", auth=auth)
print(r.text)

# Authentication using auth bearer
auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
resp = client.post(url="https://httpbin.org/anything", auth_bearer=auth_bearer)
print(r.text)

# Using proxy
resp = primp.Client(proxy="http://127.0.0.1:8080").get("https://tls.peet.ws/api/all")
print(resp.json())

# Using custom CA certificate store: file or certifi.where()
resp = primp.Client(ca_cert_file="/cert/cacert.pem").get("https://tls.peet.ws/api/all")
print(resp.json())
resp = primp.Client(ca_cert_file=certifi.where()).get("https://tls.peet.ws/api/all")
print(resp.json())

# You can also use convenience functions that use a default Client instance under the hood:
# primp.get() | primp.head() | primp.options() | primp.delete() | primp.post() | primp.patch() | primp.put()
# These functions can accept the `impersonate` parameter:
resp = primp.get("https://httpbin.org/anything", impersonate="chrome_127")
print(r.text)
```

### II. AsyncClient

TODO

