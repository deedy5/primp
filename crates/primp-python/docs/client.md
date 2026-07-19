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
    timeout=None,           # Total request timeout in seconds
    connect_timeout=None,   # Connection establishment timeout in seconds
    read_timeout=None,      # Response body read timeout in seconds
    impersonate=None,       # Browser to impersonate
    impersonate_os=None,    # OS to impersonate
    follow_redirects=True,  # Follow redirects
    max_redirects=20,       # Max redirects
    verify=True,            # Verify SSL certificates
    ca_cert_file=None,      # Path to CA certificate
    https_only=False,       # HTTPS only mode
    http2_only=False,       # HTTP/2 only mode
    dns_resolver=None,      # DNS resolver: str, list[str], or None (see docs/dns.md)
    base_url=None,          # Base URL for relative paths
    cookies=None,           # Initial cookies to send with all requests
)
```

## Browser Impersonation

| Browser | Versions |
|---------|----------|
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome_147`, `chrome_148`, `chrome_149`, `chrome_150`, `chrome_151`, `chrome_152`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari_26.4`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge_147`, `edge_148`, `edge_149`, `edge_150`, `edge_151`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox_149`, `firefox_150`, `firefox_151`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera_130`, `opera_131`, `opera_132`, `opera_133`, `opera_134`, `opera_135`, `opera` |
| Random | `random` |

## OS Impersonation

`android`, `ios`, `linux`, `macos`, `windows`, `random`

## Methods

### HTTP Methods

```python
client.get(url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=False)
client.head(url, ...)
client.options(url, ...)
client.delete(url, ...)
client.post(url, params=None, headers=None, cookies=None, content=None, data=None, json=None, files=None, auth=None, auth_bearer=None, timeout=None, read_timeout=None, follow_redirects=None, stream=False)
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
| `timeout` | float | Total timeout in seconds |
| `read_timeout` | float | Read timeout in seconds (max gap between bytes) |
| `follow_redirects` | bool | Per-request redirect policy override (`None` = client default) |
| `stream` | bool | Return a streaming response (iterate `iter_bytes()`/`iter_text()`) |

### Cookie Management

```python
client.set_cookies(url="https://example.com", cookies={"name": "value"})
cookies = client.get_cookies(url="https://example.com")
```

### Header Management

```python
headers = client.headers
client.headers = {"User-Agent": "Custom"}     # Replace all
client.headers_update({"X-Custom": "value"})  # Merge
```

### Proxy Management

```python
proxy = client.proxy
client.proxy = "http://127.0.0.1:8080"
# Clear the proxy
client.proxy = None
```

### Redirects

The `follow_redirects` and `max_redirects` constructor parameters set the
default redirect policy. A per-request `follow_redirects=True` on a
`get`/`post`/etc. call overrides the redirect policy for that request
while still honoring the client-level `max_redirects` setting.

```python
client = primp.Client(follow_redirects=True, max_redirects=5)
# follow_redirects=True uses max_redirects=5 (not a hard-coded value)
resp = client.get("https://httpbin.org/redirect/1", follow_redirects=True)
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PRIMP_PROXY` | Default proxy URL |
| `PRIMP_CA_BUNDLE` | Path to CA certificate |

## Exceptions

See [exceptions.md](exceptions.md) for the full exception hierarchy and handling examples.
