# Exceptions

Exception hierarchy for error handling in primp.

## Hierarchy

```
PrimpError (base exception)
├── BuilderError          # Client/request builder errors (invalid URL, headers)
├── RequestError          # Generic request errors
│   ├── ConnectError     # Connection errors (DNS, proxy, SSL, network)
│   └── TimeoutError     # Request timeout errors
├── StatusError           # HTTP status errors (4xx/5xx responses)
├── RedirectError         # Too many redirects
├── BodyError             # Body/stream errors
├── DecodeError           # Content decoding errors
└── UpgradeError          # Protocol upgrade errors
```

## Exception Attributes

All exceptions inherit from `PrimpError` and share the following attributes:

| Exception | Attributes | Description |
|-----------|------------|-------------|
| `PrimpError` | `url: str \| None` | Base exception with optional URL |
| `BuilderError` | `url: str \| None` | Invalid URL, headers, or other builder errors |
| `RequestError` | `url: str \| None` | Generic request errors |
| `ConnectError` | `url: str \| None` | DNS failures, proxy errors, SSL errors, network issues |
| `TimeoutError` | `url: str \| None` | Request or connection timeout |
| `StatusError` | `status_code: int`, `url: str \| None` | HTTP 4xx/5xx status errors |
| `RedirectError` | `url: str \| None` | Redirect limit exceeded |
| `BodyError` | `url: str \| None` | Body/stream I/O errors |
| `DecodeError` | `url: str \| None` | Content decoding errors (gzip, deflate, etc.) |
| `UpgradeError` | `url: str \| None` | Protocol upgrade errors |

## Exception Details

### PrimpError

Base exception for all primp errors. All other primp exceptions inherit from this class.

```python
import primp

try:
    resp = client.get(url)
except primp.PrimpError as e:
    print(f"Request failed: {e}")
    if e.url:
        print(f"URL: {e.url}")
```

### BuilderError

Raised for client/request builder errors, including:
- Invalid URL format (missing scheme, malformed URL)
- Invalid header names or values
- Empty URL

```python
import primp

client = primp.Client()

# Invalid URL (missing scheme)
try:
    client.get("example.com")  # Missing http:// or https://
except primp.BuilderError as e:
    print(f"Builder error: {e}")

# Invalid header
try:
    client.get("https://httpbin.org/get", headers={"X-Test": "value\nwith-newline"})
except primp.BuilderError as e:
    print(f"Invalid header: {e}")

# Catch as PrimpError (parent class)
try:
    client.get("://invalid")
except primp.PrimpError as e:
    print(f"Caught as PrimpError: {e}")
```

### RequestError

Generic request errors. This is the parent class for `ConnectError` and `TimeoutError`.

```python
import primp

try:
    resp = client.get(url)
except primp.RequestError as e:
    print(f"Request error: {e}")
```

### ConnectError

Raised for connection-related failures, including:
- DNS resolution failures
- Connection refused
- Proxy connection errors
- SSL/TLS certificate errors
- Network unreachable

```python
import primp

# Connection refused
try:
    client = primp.Client()
    resp = client.get("http://localhost:1", timeout=0.5)
except primp.ConnectError as e:
    print(f"Connection error: {e}")

# DNS failure
try:
    resp = client.get("https://nonexistent-domain-12345.com")
except primp.ConnectError as e:
    print(f"DNS/Connection error: {e}")

# SSL certificate error
try:
    client = primp.Client(verify=True)
    resp = client.get("https://expired.badssl.com/")
except primp.ConnectError as e:
    print(f"SSL/Connection error: {e}")

# Proxy error (also raised as ConnectError)
try:
    client = primp.Client(proxy="http://invalid-proxy:9999")
    resp = client.get("https://httpbin.org/get")
except primp.ConnectError as e:
    print(f"Proxy/Connection error: {e}")

# Can be caught as RequestError (parent class)
try:
    resp = client.get("http://localhost:1")
except primp.RequestError as e:
    print(f"Caught as RequestError: {e}")
```

### TimeoutError

Raised when a request exceeds the specified timeout.

```python
import primp

# Request timeout
try:
    client = primp.Client()
    resp = client.get("https://httpbin.org/delay/15", timeout=2)
except primp.TimeoutError as e:
    print(f"Timeout: {e}")

# Can be caught as RequestError (parent class)
try:
    resp = client.get("https://httpbin.org/delay/15", timeout=2)
except primp.RequestError as e:
    print(f"Caught as RequestError: {e}")
```

### StatusError

Raised for HTTP 4xx/5xx status responses. Has an additional `status_code` attribute.

```python
import primp

# Using raise_for_status()
client = primp.Client()
response = client.get("https://httpbin.org/status/404")
try:
    response.raise_for_status()
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error: {e}")
    if e.url:
        print(f"URL: {e.url}")

# Different status codes
try:
    response = client.get("https://httpbin.org/status/500")
    response.raise_for_status()
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error")

# Can be caught as PrimpError (parent class)
try:
    response = client.get("https://httpbin.org/status/403")
    response.raise_for_status()
except primp.PrimpError as e:
    print(f"Caught as PrimpError: {e}")
```

### RedirectError

Raised when the number of redirects exceeds the configured limit.

```python
import primp

# Too many redirects
try:
    client = primp.Client(follow_redirects=True, max_redirects=2)
    resp = client.get("https://httpbin.org/redirect/10")
except primp.RedirectError as e:
    print(f"Too many redirects: {e}")

# Can be caught as PrimpError (parent class)
try:
    resp = client.get("https://httpbin.org/redirect/10")
except primp.PrimpError as e:
    print(f"Caught as PrimpError: {e}")
```

### BodyError

Raised for body/stream I/O errors, such as:
- Timeout while reading/writing body
- Connection errors while streaming body
- Request body stream errors

```python
import primp

try:
    # Streaming a large body that encounters an error
    resp = client.get(url, stream=True)
    for chunk in resp.iter_bytes():
        # Error during streaming
        pass
except primp.BodyError as e:
    print(f"Body error: {e}")
```

### DecodeError

Raised when content decoding fails, such as:
- Invalid gzip content
- Invalid deflate content
- Broken transfer encoding

```python
import primp

# Invalid gzip content (server returns Content-Encoding: gzip but invalid data)
try:
    client = primp.Client()
    response = client.get("https://example.com/invalid-gzip")
    content = response.content  # DecodeError raised here
except primp.DecodeError as e:
    print(f"Decode error: {e}")

# Can be caught as PrimpError (parent class)
try:
    response = client.get("https://example.com/invalid-deflate")
    content = response.content
except primp.PrimpError as e:
    print(f"Caught as PrimpError: {e}")
```

### UpgradeError

Raised for protocol upgrade errors (e.g., WebSocket upgrade failures).

```python
import primp

try:
    resp = client.get(url)
except primp.UpgradeError as e:
    print(f"Protocol upgrade error: {e}")
```

## Best Practices

### Catching Specific Exceptions

Catch more specific exceptions before general ones:

```python
import primp

try:
    resp = client.get(url, timeout=10)
    resp.raise_for_status()
    data = resp.json()
except primp.TimeoutError:
    print("Request timed out")
except primp.ConnectError:
    print("Connection failed (DNS, proxy, SSL, or network)")
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error")
except primp.PrimpError as e:
    print(f"Other primp error: {e}")
```

### Handling All Request Errors

Use `RequestError` to catch both `ConnectError` and `TimeoutError`:

```python
import primp

try:
    resp = client.get(url, timeout=10)
except primp.RequestError as e:
    # Catches both ConnectError and TimeoutError
    print(f"Request failed: {e}")
```

### Complete Error Handling Example

```python
import primp

client = primp.Client(impersonate="chrome_145", timeout=10)

try:
    resp = client.get("https://httpbin.org/status/200")
    resp.raise_for_status()
    data = resp.json()
except primp.BuilderError as e:
    print(f"Invalid request configuration: {e}")
except primp.TimeoutError as e:
    print(f"Request timed out: {e}")
except primp.ConnectError as e:
    print(f"Connection failed: {e}")
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error: {e}")
except primp.RedirectError as e:
    print(f"Too many redirects: {e}")
except primp.DecodeError as e:
    print(f"Content decoding failed: {e}")
except primp.BodyError as e:
    print(f"Body/stream error: {e}")
except primp.PrimpError as e:
    print(f"Other primp error: {e}")
```

### JSON Decode Errors

Note that JSON decode errors from `response.json()` are standard Python exceptions, not primp exceptions:

```python
import json
import primp

resp = client.get("https://httpbin.org/html")
try:
    data = resp.json()
except json.JSONDecodeError as e:
    # JSON decode errors are standard Python exceptions
    print(f"JSON decode error: {e}")
```

### Accessing Exception Attributes

```python
import primp

try:
    resp = client.get("https://httpbin.org/status/404")
    resp.raise_for_status()
except primp.StatusError as e:
    print(f"Status code: {e.status_code}")
    print(f"URL: {e.url}")
    print(f"Message: {e}")
```
