![Python >= 3.8](https://img.shields.io/badge/python->=3.8-red.svg) [![](https://badgen.net/github/release/deedy5/pyreqwest-impersonate)](https://github.com/deedy5/pyreqwest-impersonate/releases) [![](https://badge.fury.io/py/pyreqwest_impersonate.svg)](https://pypi.org/project/pyreqwest_impersonate) [![Downloads](https://static.pepy.tech/badge/pyreqwest_impersonate/week)](https://pepy.tech/project/pyreqwest_impersonate) [![CI](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml)
# Pyreqwest_impersonate

The fastest python HTTP client that can impersonate web browsers by mimicking their headers and `TLS/JA3/JA4/HTTP2` fingerprints.</br>
Binding to the Rust `reqwest_impersonate` library.</br>
🏁 Check the [benchmark](https://github.com/deedy5/pyreqwest_impersonate/tree/main/benchmark) for more details.


Provides precompiled wheels:
- [x] Linux:  `amd64`, `aarch64`, `armv7`.
- [x] Windows: `amd64`.
- [x] MacOS:  `amd64`, `aarch64`.

## Table of Contents

- [Installation](#installation)
- [Key Features](#key-features)
- [Usage](#usage)
 - [I. Client](#i-client)
 - [II. AsyncClient](#ii-asyncclient)


## Installation

```python
pip install -U pyreqwest_impersonate
```

## Key Features
- `Impersonate`: The `Client` offers an `impersonate` option, enabling it to mimic web browsers by replicating their headers and `TLS/JA3/JA4/HTTP2` fingerprints. This feature is crucial for avoiding detection as a bot and potential blocking by websites.
- `Thread-safe`: The `Client` is designed to be thread-safe, allowing it to be safely used in multithreaded environments.
- `High Performance`: The attributes of the `Response` object are executed in Rust, which is known for its high performance. This ensures that operations like accessing headers, decoding text, or parsing JSON are very fast.
- `Lazy Execution`: All attributes of the `Response` object are executed lazily. This means that the actual computation or data retrieval happens only when you access the attribute, not when the `Response` object is created.
- `Automatic Character Encoding Detection`: The `Response` object intelligently detects the character encoding of the response body from the "Content-Type" header. If the encoding is not specified, it defaults to "UTF-8".

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
        timeout (float, optional): HTTP request timeout in seconds. Default is 30.
        cookie_store (bool, optional): Enable a persistent cookie store. Received cookies will be preserved and included 
            in additional requests. Default is True.
        referer (bool, optional): Enable or disable automatic setting of the `Referer` header. Default is True.
        proxy (str, optional): Proxy URL for HTTP requests. Example: "socks5://127.0.0.1:9150". Default is None.
        impersonate (str, optional): Entity to impersonate. Example: "chrome_123". Default is None.
            Chrome: "chrome_99","chrome_100","chrome_101","chrome_104","chrome_105","chrome_106","chrome_108", 
                "chrome_107","chrome_109","chrome_114","chrome_116","chrome_117","chrome_118","chrome_119", 
                "chrome_120","chrome_123"
            Safari: "safari_12","safari_15_3","safari_15_5","safari_15_6_1","safari_16","safari_16_5","safari_17_2_1"
            OkHttp: "okhttp_3_9","okhttp_3_11","okhttp_3_13","okhttp_3_14","okhttp_4_9","okhttp_4_10","okhttp_5"
            Edge: "edge_99","edge_101","edge_120"
        follow_redirects (bool, optional): Whether to follow redirects. Default is True.
        max_redirects (int, optional): Maximum redirects to follow. Default 20. Applies if `follow_redirects` is True.
        verify (bool, optional): Verify SSL certificates. Default is False.
        http1 (bool, optional): Use only HTTP/1.1. Default is None.
        http2 (bool, optional): Use only HTTP/2. Default is None.
         
    """
```

#### Client Methods

The `Client` class provides a set of methods for making HTTP requests: `get`, `head`, `options`, `delete`, `post`, `put`, `patch`, each of which internally utilizes the `request()` method for execution. The parameters for these methods closely resemble those in `httpx`.
```python
def get(url, *, params=None, headers=None, auth=None, auth_bearer=None, timeout=None):
    """Performs a GET request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
        headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
        auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password 
            for basic authentication. Default is None.
        auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (Optional[float]): The timeout for the request in seconds. Default is 30.

    """
```
```python
def post(url, *, params=None, headers=None, content=None, data=None, files=None, auth=None, auth_bearer=None, timeout=None):
    """Performs a POST request to the specified URL.

    Args:
        url (str): The URL to which the request will be made.
        params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
        headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
        content (Optional[bytes]): The content to send in the request body as bytes. Default is None.
        data (Optional[Dict[str, str]]): The form data to send in the request body. Default is None.
        files (Optional[Dict[str, str]]): A map of file fields to file paths to be sent as multipart/form-data. Default is None.
        auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password 
            for basic authentication. Default is None.
        auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
        timeout (Optional[float]): The timeout for the request in seconds. Default is 30.

    """
```

#### Example

```python
import pyreqwest_impersonate as pri

client = pri.Client(impersonate="chrome_123")

# get request
resp = client.get("https://tls.peet.ws/api/all")
print(resp.json())

# post request
data = {"key1": "value1", "key2": "value2"}
auth = ("user", "password")
resp = client.post(url="https://httpbin.org/anything", data=data, auth=auth)
print(resp.content)
print(resp.cookies)
print(resp.headers)
print(resp.json())
print(resp.status_code)
print(resp.text)
print(resp.url)

# You can also use convenience functions that use a default Client instance under the hood:
# pri.get() | pri.head() | pri.options() | pri.delete() | pri.post | pri.patch | pri.put
resp = pri.get("https://httpbin.org/anything") # Default Client does not impersonate a browser
resp = pri.Client(impersonate="chrome_123").get("https://httpbin.org/anything")  
```
### II. AsyncClient

TODO

