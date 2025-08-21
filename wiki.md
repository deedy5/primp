# PRIMP Wiki

## Overview

PRIMP is a Python HTTP client powered by Rust, offering advanced browser impersonation, proxy support (SOCKS5 and HTTP), cookie management, streaming, redirect handling, and more. This wiki documents all Python-accessible classes, methods, arguments, and features.

---

## Exposed Python Classes

- **Client**: Synchronous HTTP client.
- **AsyncClient**: Asynchronous HTTP client.
- **Response**: Represents an HTTP response.

---

## Initialization & Configuration

You can configure the client with various options:

```python
import primp

client = primp.Client(
    impersonate="chrome_137",
    impersonate_os="windows",
    headers={"User-Agent": "Custom-Agent"},
    proxy="socks5://user:pass@proxy.example.com:9151",  # SOCKS5 proxy
    timeout=30.0,
    cookie_store=True,           # Enables automatic cookie handling
    referer=True,
    follow_redirects=True,       # Enable automatic redirect following
    max_redirects=20,            # Maximum number of redirects to follow
    verify=False,
    https_only=True,             # Only allow HTTPS requests
    http2_only=False,            # Allow both HTTP/1.1 and HTTP/2
    auth=("username", "password"),
    auth_bearer="your-token-here"
)
```

For asynchronous usage:

```python
import primp

async with primp.AsyncClient(
    proxy="http://user:pass@proxy.example.com:8080",  # HTTP proxy
    cookie_store=True,
    follow_redirects=True,
    max_redirects=10,
    https_only=True,
    http2_only=False,
) as client:
    ...
```

---

## Proxy Support

You can use both SOCKS5 and HTTP proxies:

```python
# SOCKS5 proxy
client = primp.Client(proxy="socks5://user:pass@proxy.example.com:9151")

# HTTP proxy
client = primp.Client(proxy="http://user:pass@proxy.example.com:8080")
```

For asynchronous usage:

```python
async with primp.AsyncClient(proxy="http://proxy.example.com:8080") as client:
    response = await client.get("https://example.com")
    print(response.text)
```

---

## Cookie Management

### Automatic Cookie Handling

If you set `cookie_store=True`, PRIMP will automatically handle cookies for you:
- Cookies from server responses are stored and sent with future requests to matching domains.
- Cookies are updated naturally as you interact with websites, just like a browser.

### Manual Cookie Management

Set cookies for a domain:

```python
client.set_cookies("https://www.example.com", {"session_id": "abc123"})
```

Get cookies for a domain:

```python
cookies = client.get_cookies("https://www.example.com")
print(cookies)
```

Update cookies for a domain (overwrite or add new cookies):

```python
# Get current cookies
cookies = client.get_cookies("https://www.example.com")

# Update or add a cookie
cookies["new_cookie"] = "new_value"

# Set updated cookies
client.set_cookies("https://www.example.com", cookies)
```

### Natural Cookie Update Example

```python
import primp
import asyncio

async def main():
    async with primp.AsyncClient(cookie_store=True) as client:
        # Initial request, cookies will be set by the server
        response = await client.get("https://www.example.com")
        print("Cookies after first request:", client.get_cookies("https://www.example.com"))

        # Subsequent requests will automatically send updated cookies
        response = await client.get("https://www.example.com")
        print("Cookies after second request:", client.get_cookies("https://www.example.com"))

        # You can also manually update cookies if needed
        cookies = client.get_cookies("https://www.example.com")
        cookies["manual_cookie"] = "value"
        client.set_cookies("https://www.example.com", cookies)

        # The updated cookies will be sent with the next request
        response = await client.get("https://www.example.com/mypage)
        print("Cookies after manual update:", client.get_cookies("https://www.example.com"))

asyncio.run(main())
```

---

## Methods & Properties

### Client / AsyncClient

| Method/Property         | Description                                      |
|------------------------|--------------------------------------------------|
| `request`              | Make an HTTP request (all methods supported).    |
| `get`, `post`, ...     | Convenience methods for HTTP verbs.              |
| `set_cookies`          | Set cookies for a domain.                        |
| `get_cookies`          | Retrieve cookies for a domain.                   |
| `headers`              | Get/set default headers.                         |
| `headers_update`       | Update headers with a dictionary.                |
| `proxy`                | Get/set proxy URL.                               |
| `auth`                 | Set basic authentication.                        |
| `auth_bearer`          | Set bearer token authentication.                 |

#### Supported Arguments

- `impersonate`: Browser fingerprint (e.g., `chrome_137`, `random`)
- `impersonate_os`: OS fingerprint (e.g., `windows`, `random`)
- `headers`: Default headers
- `proxy`: Proxy URL (SOCKS5 or HTTP)
- `timeout`: Request timeout (seconds)
- `verify`: SSL verification
- `follow_redirects`: Follow HTTP redirects automatically
- `max_redirects`: Maximum redirects to follow
- `auth`: Basic auth tuple
- `auth_bearer`: Bearer token
- `cookie_store`: Enable persistent cookie store
- `referer`: Automatically handle referer headers
- `https_only`: Force HTTPS connections
- `http2_only`: Allow both HTTP/1.1 and HTTP/2

---

## Redirect Handling

- `follow_redirects=True`: The client will automatically follow HTTP redirects (3xx responses).
- `max_redirects`: Limits the number of redirects to prevent infinite loops.

### Example

```python
client = primp.Client(
    follow_redirects=True,
    max_redirects=5
)

response = client.get("http://example.com/redirect")
print(response.status_code)
print(response.url)  # Final URL after redirects
```

---

## HTTPS and HTTP/2 Options

- `https_only=True`: Forces all requests to use HTTPS. Use this for secure data transfer.
- `http2_only=False`: Allows both HTTP/1.1 and HTTP/2. Set to `True` to force HTTP/2 only.

### Example Usage

```python
client = primp.Client(
    https_only=True,      # Only allow HTTPS requests
    http2_only=False,     # Allow both HTTP/1.1 and HTTP/2
)
```

---

## Response Handling

After making a request, you receive a `Response` object:

| Property/Method      | Description                                  |
|----------------------|----------------------------------------------|
| `status_code`        | HTTP status code (e.g., 200, 404)            |
| `headers`            | Response headers (dict)                      |
| `cookies`            | Response cookies (dict)                      |
| `content`            | Raw response bytes                           |
| `text`               | Response body as text                        |
| `json()`             | Parse response body as JSON                  |
| `text_plain`         | Plain text rendering                         |
| `text_markdown`      | Markdown rendering                           |
| `text_rich`          | Rich text rendering                          |
| `encoding`           | Get/set response encoding                    |
| `stream()`           | Stream response content (async iterator)     |

### Example

```python
response = await client.get("https://www.example.com/intro")

print("Status code:", response.status_code)
print("Headers:", response.headers)
print("Cookies:", response.cookies)
print("Content (bytes):", response.content)
print("Text:", response.text)
print("JSON:", response.json())
print("Encoding:", response.encoding)

# Change encoding if needed
response.encoding = "utf-8"

# Streaming response
async for chunk in response.stream():
    print("Chunk:", chunk)
```

---

## Custom Headers

```python
await client.headers_update({
    "Sec-Fetch-Dest": "document",
    "Sec-Fetch-Mode": "navigate",
    "Sec-Fetch-Site": "none",
    "Sec-Fetch-User": "?1"
})
```

---

## File Uploads

```python
response = await client.post(
    "https://httpbin.org/post",
    files={"file1": "/path/to/file1.txt"}
)
print(response.json())
```

---

## Authentication

```python
client = primp.AsyncClient(auth=("username", "password"))
client = primp.AsyncClient(auth_bearer="your-token-here")
```

---

## Streaming Responses

```python
async for chunk in response.stream():
    print("Chunk:", chunk)
```

---

## Error Handling

Custom exceptions are raised for network errors, timeouts, and proxy issues:

```python
try:
    response = await client.get("https://example.com")
except primp.PrimpError as e:
    print(f"Request failed: {e}")
```

---

## Real-World Workflow Example

```python
import primp
import random

async with primp.AsyncClient(
    impersonate=random.choice(["chrome_137", "random"]),
    impersonate_os=random.choice(["windows", "random"]),
    proxy="http://user:pass@proxy.example.com:8080",
    timeout=30.0,
    cookie_store=True,
    referer=True,
    follow_redirects=True,
    max_redirects=20,
    verify=False,
    https_only=True,
    http2_only=False,
) as client:
    await client.headers_update({
        "Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        "Accept-Language": "en-US,en;q=0.9",
        "DNT": "1",
    })
    response = await client.get("https://www.example.com/hz/wishlist/intro")
    print(response.status_code, response.text)
    # Cookies will update naturally as you interact with the site
    print("Current cookies:", client.get_cookies("https://www.example.com"))
    # You can manually update cookies if needed
    cookies = client.get_cookies("https://www.example.com")
    cookies["manual_cookie"] = "value"
    client.set_cookies("https://www.example.com", cookies)
```

---

## Environment Variables

- `PRIMP_CA_BUNDLE`: Path to custom CA bundle
- `PRIMP_PROXY`: Default proxy URL

---

## Reference

See the [API stubs](primp/primp.pyi) for full method signatures.

---

*This wiki covers all Python features exposed by the Rust implementation, including initialization, configuration, proxy support (SOCKS5 and HTTP), cookie management (automatic and manual, including natural updating), redirect handling, HTTPS/HTTP2 options, response handling, headers, file uploads, authentication, streaming, error handling, and real-world