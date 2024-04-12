# pyreqwest_impersonate

HTTP requests with impersonating web browsers.</br>
Impersonate browsers headers, `TLS/JA3` and `HTTP/2` fingerprints.</br>
Binding to the Rust `reqwest_impersonate` library.

## Installation

```python
pip install -U pyreqwest_impersonate
```

## Usage
### I. Client

A blocking client for making HTTP requests with specific configurations.

Attributes:
- `timeout` (float, optional): The timeout for the HTTP requests in seconds. Default is 30.
- `proxy` (str, optional): The proxy URL to use for the HTTP requests. Default is None.
- `impersonate` (str, optional): The identifier for the entity to impersonate. Default is None.

```python3
from pyreqwest_impersonate import Client
client = Client(
    timeout=10,
    proxy="socks5://127.0.0.1:9150",
    impersonate="chrome_123",
)
```
example:
```python
from pyreqwest_impersonate import Client

client = Client(impersonate="chrome_123")
response = client.request("GET", "https://httpbin.org/anything")

print(response.text)
print(response.status_code)
print(response.url)
print(response.headers)
print(response.cookies)

resp = Client(impersonate="chrome_123").request("GET", "https://tls.browserleaks.com/json")
print(resp.text)

resp = Client(impersonate="chrome_123").request("GET", "https://check.ja3.zone/")
print(resp.text)
```
### II. AsyncClient

TODO
___

### Impersonate 
Variants of the `impersonate` parameter:
```python3
"chrome_99"
"chrome_100"
"chrome_101"
"chrome_104"
"chrome_105"
"chrome_106"
"chrome_108"
"chrome_107"
"chrome_109"
"chrome_114"
"chrome_116"
"chrome_117"
"chrome_118"
"chrome_119"
"chrome_120"
"chrome_123"
"safari_12"
"safari_15_3" 
"safari_15_5" 
"safari_15_6_1"
"safari_16"
"safari_16_5"
"safari_17_2_1"
"okhttp_3_9"
"okhttp_3_11"
"okhttp_3_13"
"okhttp_3_14"
"okhttp_4_9"
"okhttp_4_10"
"okhttp_5"
"edge_99"
"edge_101"
"edge_120"
```