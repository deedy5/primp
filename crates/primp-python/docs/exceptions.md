# Exceptions

Exception hierarchy for error handling in primp.

## Hierarchy

```
PrimpError (base exception)
├── BuilderError          # Client/request builder errors
├── RequestError          # Generic request errors
│   ├── ConnectError     # Connection errors (DNS, proxy, SSL)
│   └── TimeoutError     # Request timeout
├── StatusError           # HTTP 4xx/5xx (has status_code attribute)
├── RedirectError         # Too many redirects
├── BodyError             # Body/stream errors
├── DecodeError           # Content decoding errors
└── UpgradeError          # Protocol upgrade errors
```

## Attributes

| Exception | Attributes | Description |
|-----------|------------|-------------|
| `PrimpError` | `url: str \| None` | Base exception |
| `BuilderError` | `url` | Invalid URL, headers |
| `RequestError` | `url` | Generic request errors |
| `ConnectError` | `url` | DNS, proxy, SSL, network |
| `TimeoutError` | `url` | Request/connection timeout |
| `StatusError` | `status_code: int`, `url` | HTTP 4xx/5xx |
| `RedirectError` | `url` | Redirect limit exceeded |
| `BodyError` | `url` | Body/stream I/O errors |
| `DecodeError` | `url` | gzip/deflate/zstd decoding |
| `UpgradeError` | `url` | Protocol upgrade errors |

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
    client.get("https://nonexistent-domain-12345.com")
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

### StatusError

```python
resp = client.get("https://httpbin.org/status/404")
try:
    resp.raise_for_status()
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error: {e}")
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

`response.json()` raises standard `json.JSONDecodeError`, not a primp exception:

```python
import json

try:
    data = resp.json()
except json.JSONDecodeError as e:
    print(f"JSON decode error: {e}")
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
    print(f"HTTP {e.status_code} error")
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
