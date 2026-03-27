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
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera` |
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
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `PRIMP_PROXY` | Default proxy URL |
| `PRIMP_CA_BUNDLE` | Path to CA certificate |

## Exceptions

See [exceptions.md](exceptions.md) for the full exception hierarchy and handling examples.
