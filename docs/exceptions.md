# Exceptions

Exception hierarchy for error handling.

## Hierarchy

```
RequestException (base)
├── HTTPError                    # HTTP 4xx/5xx status codes
├── ConnectionError              # All connection-related failures
│   ├── ConnectTimeout          # Connection establishment timeout
│   ├── SSLError                # SSL/TLS certificate errors
│   └── ProxyError              # Proxy connection errors
├── Timeout                      # All timeout-related failures
│   └── ReadTimeout             # Data read timeout
├── TooManyRedirects            # Redirect limit exceeded
├── RequestError                 # Request building/sending errors
│   ├── InvalidURL              # URL format errors
│   └── InvalidHeader           # Header format errors
├── BodyError                    # Body-related errors
│   ├── StreamConsumedError     # Stream already consumed
│   └── ChunkedEncodingError    # Chunked encoding errors
├── DecodeError                  # Decoding errors
│   └── ContentDecodingError    # Content decode errors
├── JSONError                    # JSON-related errors
│   ├── InvalidJSONError        # Invalid JSON in request
│   └── JSONDecodeError         # JSON decode in response
```

## Common Exceptions

### RequestException

Base exception for all request errors. `PrimpError` is an alias for `RequestException`, so you can use either name.

```python
try:
    resp = client.get(url)
except primp.RequestException as e:
    print(f"Request failed: {e}")

# Or use the alias
try:
    resp = client.get(url)
except primp.PrimpError as e:
    print(f"Request failed: {e}")
```

### HTTPError

Raised for HTTP 4xx/5xx responses.

```python
try:
    resp = client.get(url)
except primp.HTTPError as e:
    print(f"HTTP error: {e}")
```

### ConnectionError

Network connection failures.

```python
try:
    resp = client.get(url)
except primp.ConnectionError as e:
    print(f"Connection failed: {e}")
```

### Timeout

Request timeout errors.

```python
try:
    resp = client.get(url, timeout=5)
except primp.Timeout as e:
    print(f"Request timed out: {e}")
except primp.ConnectTimeout as e:
    print(f"Connection timed out: {e}")
except primp.ReadTimeout as e:
    print(f"Read timed out: {e}")
```

### ProxyError

Proxy configuration errors.

```python
try:
    client = primp.Client(proxy="http://bad-proxy:8080")
    resp = client.get(url)
except primp.ProxyError as e:
    print(f"Proxy error: {e}")
```

### SSLError

SSL/TLS certificate errors.

```python
try:
    resp = client.get(url)
except primp.SSLError as e:
    print(f"SSL error: {e}")
```

### JSONDecodeError

JSON parsing failures.

```python
try:
    data = resp.json()
except primp.JSONDecodeError as e:
    print(f"Invalid JSON: {e}")
```

## Best Practices

```python
import primp

try:
    resp = client.get(url, timeout=10)
    resp.raise_for_status()
    data = resp.json()
except primp.Timeout:
    print("Request timed out")
except primp.ConnectionError:
    print("Connection failed")
except primp.HTTPError as e:
    print(f"HTTP error: {e}")
except primp.JSONDecodeError:
    print("Invalid JSON response")
except primp.RequestException as e:
    print(f"Request failed: {e}")
```
