![Python >= 3.8](https://img.shields.io/badge/python->=3.8-red.svg) [![](https://badgen.net/github/release/deedy5/pyreqwest-impersonate)](https://github.com/deedy5/pyreqwest-impersonate/releases) [![](https://badge.fury.io/py/pyreqwest_impersonate.svg)](https://pypi.org/project/pyreqwest_impersonate) [![Downloads](https://static.pepy.tech/badge/pyreqwest_impersonate/week)](https://pepy.tech/project/pyreqwest_impersonate) [![CI](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml)
# Pyreqwest_impersonate

HTTP client that can impersonate web browsers, mimicking their headers and `TLS/JA3/JA4/HTTP2` fingerprints.</br>
Binding to the Rust `reqwest_impersonate` library.

Provides precompiled wheels:
- [x] Linux:  `amd64`, `aarch64`.
- [x] Windows: `amd64`.
- [x] MacOS:  `amd64`, `aarch64`.

## Installation

```python
pip install -U pyreqwest_impersonate
```

## Usage
### I. Client

A blocking HTTP client that can impersonate web browsers.
```python3
class Client:
    """Initializes a blocking HTTP client that can impersonate web browsers.
    
    Args:
        auth (tuple, optional): A tuple containing the username and password for basic authentication. Default is None.
        auth_bearer (str, optional): Bearer token for authentication. Default is None.
        params (dict, optional): Default query parameters to include in all requests. Default is None.
        headers (dict, optional): Default headers to send with requests. If `impersonate` is set, this will be ignored.
        timeout (float, optional): HTTP request timeout in seconds. Default is 30.
        proxy (str, optional): Proxy URL for HTTP requests. Example: "socks5://127.0.0.1:9150". Default is None.
        impersonate (str, optional): Entity to impersonate. Example: "chrome_123". Default is None.
            Chrome: "chrome_99","chrome_100","chrome_101","chrome_104","chrome_105","chrome_106","chrome_108", 
                "chrome_107","chrome_109","chrome_114","chrome_116","chrome_117","chrome_118","chrome_119", 
                "chrome_120","chrome_123"
            Safari: "safari_12","safari_15_3","safari_15_5","safari_15_6_1","safari_16","safari_16_5","safari_17_2_1"
            OkHttp: "okhttp_3_9","okhttp_3_11","okhttp_3_13","okhttp_3_14","okhttp_4_9","okhttp_4_10","okhttp_5"
            Edge: "edge_99","edge_101","edge_120"
        follow_redirects (bool, optional): Whether to follow redirects. Default is False.
        max_redirects (int, optional): Maximum redirects to follow. Default 20. Applies if `follow_redirects` is True.
        verify (bool, optional): Verify SSL certificates. Default is True.
        http1 (bool, optional): Use only HTTP/1.1. Default is None.
        http2 (bool, optional): Use only HTTP/2. Default is None.
    """
```

### Client Methods

The `Client` class provides a set of methods for making HTTP requests: `get`, `head`, `options`, `delete`, `post`, `put`, `patch`, each of which internally utilizes the `request()` method for execution. The parameters for these methods closely resemble those in `httpx`.
```python
get(url, *, params=None, headers=None, auth=None, auth_bearer=None, timeout=None)

Performs a GET request to the specified URL.

- url (str): The URL to which the request will be made.
- params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
- headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
- auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password for basic authentication. Default is None.
- auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
- timeout (Optional[float]): The timeout for the request in seconds. Default is 30.
```
```python
post(url, *, params=None, headers=None, content=None, data=None, files=None, auth=None, auth_bearer=None, timeout=None)

Performs a POST request to the specified URL.

- url (str): The URL to which the request will be made.
- params (Optional[Dict[str, str]]): A map of query parameters to append to the URL. Default is None.
- headers (Optional[Dict[str, str]]): A map of HTTP headers to send with the request. Default is None.
- content (Optional[bytes]): The content to send in the request body as bytes. Default is None.
- data (Optional[Dict[str, str]]): The form data to send in the request body. Default is None.
- files (Optional[Dict[str, str]]): A map of file fields to file paths to be sent as multipart/form-data. Default is None.
- auth (Optional[Tuple[str, Optional[str]]]): A tuple containing the username and an optional password for basic authentication. Default is None.
- auth_bearer (Optional[str]): A string representing the bearer token for bearer token authentication. Default is None.
- timeout (Optional[float]): The timeout for the request in seconds. Default is 30.
```

#### Examples:

_Client.get()_
```python
from pyreqwest_impersonate import Client

client = Client(impersonate="chrome_123")
resp = client.get("https://tls.peet.ws/api/all")
print(resp.text)
print(resp.status_code)
print(resp.url)
print(resp.headers)
print(resp.cookies)
```
_Client.post()_
```python
from pyreqwest_impersonate import Client

data = {"key1": "value1", "key2": "value2"}
auth = ("user", "password")
resp = Client().post(url="https://httpbin.org/anything", data=data, auth=auth)
print(resp.text)
```
### II. AsyncClient

TODO