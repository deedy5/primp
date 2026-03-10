# Client (Synchronous)

HTTP client that can impersonate web browsers.

## Constructor

```python
primp.Client(
    auth=None,              # (username, password) for basic auth
    auth_bearer=None,       # Bearer token for auth
    params=None,            # Default query parameters
    headers=None,           # Default headers (ignored if impersonate set)
    cookie_store=True,      # Persistent cookie store
    referer=True,           # Auto-set Referer header
    proxy=None,             # Proxy URL (e.g., "socks5://127.0.0.1:1080")
    timeout=None,           # Request timeout in seconds
    impersonate=None,       # Browser to impersonate
    impersonate_os=None,    # OS to impersonate
    follow_redirects=True,  # Follow redirects
    max_redirects=20,       # Max redirects
    verify=True,            # Verify SSL certificates
    ca_cert_file=None,      # Path to CA certificate
    https_only=False,       # HTTPS only mode
    http2_only=False,       # HTTP/2 only mode
)
```

## Browser Impersonation

| Browser | Versions |
|---------|----------|
| Chrome | `chrome_144`, `chrome_145` |
| Safari | `safari_18.5`, `safari_26` |
| Edge | `edge_144`, `edge_145` |
| Firefox | `firefox_140`, `firefox_146` |
| Opera | `opera_126`, `opera_127` |
| Random | `random` |

## OS Impersonation

`android`, `ios`, `linux`, `macos`, `windows`, `random`

## Methods

### HTTP Methods

```python
client.get(url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=30)
client.head(url, ...)
client.options(url, ...)
client.delete(url, ...)
client.post(url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=30)
client.put(url, ...)
client.patch(url, ...)
```

### Request Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `url` | str | Target URL |
| `params` | dict | Query parameters |
| `headers` | dict | Request headers |
| `cookies` | dict | Cookies to send |
| `content` | bytes | Raw body content |
| `data` | dict | Form data |
| `json` | Any | JSON body |
| `files` | dict | File paths for multipart |
| `auth` | tuple | (username, password) |
| `auth_bearer` | str | Bearer token |
| `timeout` | float | Timeout in seconds |

### Cookie Management

```python
# Set cookies for domain
client.set_cookies(url="https://example.com", cookies={"name": "value"})

# Get cookies for domain
cookies = client.get_cookies(url="https://example.com")
```

### Header Management

```python
# Get headers
headers = client.headers

# Set headers (replaces all)
client.headers = {"User-Agent": "Custom"}

# Update headers (merges)
client.headers_update({"X-Custom": "value"})
```

### Proxy Management

```python
# Get current proxy
proxy = client.proxy

# Set proxy
client.proxy = "http://127.0.0.1:8080"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PRIMP_PROXY` | Default proxy URL |
| `PRIMP_CA_BUNDLE` | Path to CA certificate |

## Exceptions

All exceptions inherit from `PrimpError` and include a `url: str | None` attribute.

### Exception Hierarchy

```
PrimpError (base exception)
├── BuilderError          # Client/request builder errors
├── RequestError          # Generic request errors
│   ├── ConnectError     # Connection errors (DNS, proxy, SSL)
│   └── TimeoutError     # Timeout errors
├── StatusError           # HTTP status errors (status_code attribute)
├── RedirectError         # Too many redirects
├── BodyError             # Body/stream errors
├── DecodeError           # Content decoding errors
└── UpgradeError          # Protocol upgrade errors
```

See [exceptions.md](exceptions.md) for full exception documentation.

### Basic Error Handling

```python
import primp

client = primp.Client()

try:
    resp = client.get("https://example.com")
    resp.raise_for_status()
except primp.PrimpError as e:
    print(f"Request failed: {e}")
    if e.url:
        print(f"URL: {e.url}")
```

### Catching Specific Exceptions

```python
import primp

client = primp.Client(timeout=10)

try:
    resp = client.get("https://httpbin.org/status/404")
    resp.raise_for_status()
except primp.TimeoutError:
    print("Request timed out")
except primp.ConnectError:
    print("Connection failed (DNS, proxy, SSL, or network)")
except primp.StatusError as e:
    print(f"HTTP {e.status_code} error")
except primp.PrimpError as e:
    print(f"Other error: {e}")
```

### Handling Request Errors

Use `RequestError` to catch both `ConnectError` and `TimeoutError`:

```python
import primp

client = primp.Client()

try:
    resp = client.get("https://example.com", timeout=10)
except primp.RequestError as e:
    # Catches ConnectError and TimeoutError
    print(f"Request failed: {e}")
```

### Complete Error Handling Pattern

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
