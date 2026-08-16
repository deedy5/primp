# Exceptions

Exception hierarchy for error handling in primp.

## Hierarchy

```
PrimpError (base exception)
├── BuilderError          # Client/request builder errors
├── RequestError          # Generic request errors
│   ├── ConnectError     # Connection errors (proxy, SSL, network — NOT DNS)
│   ├── TimeoutError     # Request timeout
│   └── DNSError         # DNS resolution failure (not a timeout)
│       └── DNSTimeoutError  # DNS resolution timeout (also a TimeoutError)
├── StatusError           # HTTP 4xx/5xx (status_code in args[0])
├── RedirectError         # Too many redirects
├── BodyError             # Body/stream errors
├── DecodeError           # Content decoding errors
└── UpgradeError          # Protocol upgrade errors
```

`DNSTimeoutError` subclasses **both** `DNSError` and `TimeoutError` (like
`JSONDecodeError`), so a DNS lookup timeout is catchable via either parent.

## Arguments

All exceptions store their context in `args` (the standard Python exception tuple).

| Exception | `args` | Description |
|-----------|--------|-------------|
| `PrimpError` | `(message,)` or `(message, url)` | Base exception (fallback) |
| `BuilderError` | `(message,)` or `(message, url: str \| None)` | Invalid URL, headers |
| `RequestError` | `(message,)` | Generic request errors |
| `ConnectError` | `(message,)` | Proxy, SSL, network (DNS errors classify as `DNSError`, see below) |
| `TimeoutError` | `(message,)` | Request/connection timeout |
| `DNSError` | `(message,)` | DNS resolution failure (NXDOMAIN, resolver error) — not a timeout |
| `DNSTimeoutError` | `(message,)` | DNS resolution timeout (subclass of both `DNSError` and `TimeoutError`) |
| `StatusError` | `(status_code: int, message: str, url: str \| None)` | HTTP 4xx/5xx |
| `RedirectError` | `(message, url: str \| None)` | Redirect limit exceeded |
| `BodyError` | `(message,)` or `(message, url: str \| None)` | Body/stream I/O errors; `(message,)` for body-collection errors, `(message, url)` otherwise |
| `DecodeError` | `(message,)` or `(message, url: str \| None)` | gzip/deflate/zstd decoding; `(message,)` for body-collection errors, `(message, url)` otherwise |
| `UpgradeError` | `(message, url: str \| None)` | Protocol upgrade errors |

Access values via `e.args[0]`, `e.args[1]`, etc. The status code for `StatusError` is `e.args[0]`.

## Examples

### BuilderError

```python
try:
    client.get("example.com")  # Missing http:// or https://
except primp.BuilderError as e:
    print(f"Builder error: {e}")
```

### ConnectError

```python
try:
    client.get("http://127.0.0.1:1/")  # Nothing listening -> connect refused
except primp.ConnectError as e:
    print(f"Connection error: {e}")
```

### TimeoutError

```python
try:
    client.get("https://httpbin.org/delay/15", timeout=2)
except primp.TimeoutError as e:
    print(f"Timeout: {e}")
```

### DNSError

```python
try:
    client.get("https://no-such-hostname.invalid")  # NXDOMAIN
except primp.DNSError as e:
    print(f"DNS resolution failed: {e}")
```

### DNSTimeoutError

A DNS lookup that times out is raised as `DNSTimeoutError` — a subclass of
**both** `DNSError` and `TimeoutError` — so it is caught by either:

```python
# Catch all DNS problems (failures AND timeouts)
try:
    client.get("https://slow-dns.invalid")
except primp.DNSError as e:
    print(f"DNS problem (failure or timeout): {e}")

# Or treat it as a timeout specifically
try:
    client.get("https://slow-dns.invalid")
except primp.TimeoutError as e:
    print(f"Timed out (request or DNS): {e}")
```

### StatusError

```python
resp = client.get("https://httpbin.org/status/404")
try:
    resp.raise_for_status()
except primp.StatusError as e:
    # e.args = (status_code, message, url)
    print(f"HTTP {e.args[0]} error: {e}")
```

### RedirectError

```python
client = primp.Client(max_redirects=2)
try:
    client.get("https://httpbin.org/redirect/10")
except primp.RedirectError as e:
    print(f"Too many redirects: {e}")
```

### DecodeError

```python
try:
    resp = client.get("https://example.com/invalid-gzip")
    content = resp.content
except primp.DecodeError as e:
    print(f"Decode error: {e}")
```

### JSON Decode Errors

`response.json()` raises a combined `JSONDecodeError` inheriting from **both**
`DecodeError` (a `PrimpError` subclass) and `json.JSONDecodeError` (a `ValueError`
subclass). This mirrors the `requests` library pattern — the error is catchable
via `except PrimpError`, `except DecodeError`, or `except json.JSONDecodeError`:

```python
import json
import primp

# Catch as PrimpError (works for all primp errors)
try:
    data = resp.json()
except primp.PrimpError as e:
    print(f"Request/parse error: {e}")

# Catch as json.JSONDecodeError (stdlib-style, preserves .doc/.pos/.lineno/.colno)
try:
    data = resp.json()
except json.JSONDecodeError as e:
    print(f"JSON decode error at line {e.lineno}: {e}")
```

## Best Practices

### Catch Specific Exceptions First

```python
try:
    resp = client.get(url, timeout=10)
    resp.raise_for_status()
    data = resp.json()
except primp.TimeoutError:
    print("Request timed out")
except primp.ConnectError:
    print("Connection failed")
except primp.StatusError as e:
    # e.args = (status_code, message, url)
    print(f"HTTP {e.args[0]} error")
except primp.PrimpError as e:
    print(f"Other error: {e}")
```

### Use RequestError for Network Errors

`RequestError` catches both `ConnectError` and `TimeoutError`:

```python
try:
    resp = client.get(url, timeout=10)
except primp.RequestError as e:
    print(f"Network error: {e}")
```
