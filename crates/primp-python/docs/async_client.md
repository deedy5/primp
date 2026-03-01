# AsyncClient (Asynchronous)

Async HTTP client that can impersonate web browsers.

## Constructor

Same parameters as [Client](client.md).

```python
primp.AsyncClient(
    auth=None,
    auth_bearer=None,
    params=None,
    headers=None,
    cookie_store=True,
    referer=True,
    proxy=None,
    timeout=None,
    impersonate=None,
    impersonate_os=None,
    follow_redirects=True,
    max_redirects=20,
    verify=True,
    ca_cert_file=None,
    https_only=False,
    http2_only=False,
)
```

## Context Manager

Use as async context manager for proper resource cleanup:

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        resp = await client.get("https://httpbin.org/get")
        print(resp.text)

asyncio.run(main())
```

## Methods

All request methods return awaitable futures:

```python
# HTTP methods
await client.get(url, content=None, data=None, json=None, files=None, ...)
await client.head(url, ...)
await client.options(url, ...)
await client.delete(url, ...)
await client.post(url, content=None, data=None, json=None, files=None, ...)
await client.put(url, ...)
await client.patch(url, ...)

# Cookie management
client.set_cookies(url, cookies)
cookies = client.get_cookies(url)

# Header management
headers = client.headers
client.headers = {...}
client.headers_update({...})

# Proxy management
proxy = client.proxy
client.proxy = "http://127.0.0.1:8080"
```

## Concurrent Requests

```python
import asyncio
import primp

async def fetch(client, url):
    resp = await client.get(url)
    return resp.text

async def main():
    urls = [
        "https://httpbin.org/get",
        "https://httpbin.org/ip",
        "https://httpbin.org/headers",
    ]
    
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        tasks = [fetch(client, url) for url in urls]
        results = await asyncio.gather(*tasks)
        print(results)

asyncio.run(main())
```

## Response Properties

All response properties work identically for sync and async (no await needed):

```python
resp = await client.get(url)

# All properties are sync - no await needed!
content = resp.content       # bytes
text = resp.text             # str
json = resp.json()           # Any (method, not property)
headers = resp.headers       # dict
cookies = resp.cookies       # dict
encoding = resp.encoding     # str

# Also sync
url = resp.url               # str
status_code = resp.status_code  # int

# HTML conversion (sync)
markdown = resp.text_markdown
plain = resp.text_plain
rich = resp.text_rich
```

## Streaming

The AsyncResponse object is an async iterator, so you can use `async for` directly:

```python
async def stream_response():
    async with primp.AsyncClient() as client:
        resp = await client.get("https://example.com/large-file")
        
        # Async streaming with async for
        async for chunk in resp:
            print(chunk)
```

## Streaming Methods Reference

| Sync Method | Async Method | Description |
|:------------|:-------------|:------------|
| `.read()` | `.aread()` | Read remaining content into memory |
| `.iter_bytes(chunk_size)` | `.aiter_bytes(chunk_size)` | Iterate over byte chunks |
| `.iter_text(chunk_size)` | `.aiter_text(chunk_size)` | Iterate over decoded text chunks |
| `.iter_lines()` | `.aiter_lines()` | Iterate over lines |
| `.next()` | `.anext()` | Get next chunk explicitly |
| `.close()` | `.aclose()` | Close response and release resources |

## Exceptions

AsyncClient raises the same exceptions as the synchronous Client. All exceptions inherit from `PrimpError` and include a `url` attribute for debugging.

### Exception Hierarchy

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

See [exceptions.md](exceptions.md) for complete documentation of all exception types and attributes.

### Basic Error Handling

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        try:
            resp = await client.get("https://httpbin.org/get")
            resp.raise_for_status()
            print(resp.text)
        except primp.PrimpError as e:
            print(f"Request failed: {e}")
            if e.url:
                print(f"URL: {e.url}")

asyncio.run(main())
```

### Catching Specific Exceptions

Catch more specific exceptions before general ones:

```python
import asyncio
import primp

async def fetch_with_retry(url: str):
    async with primp.AsyncClient(timeout=10) as client:
        try:
            resp = await client.get(url)
            resp.raise_for_status()
            return resp.text
        except primp.TimeoutError:
            print("Request timed out - consider increasing timeout")
        except primp.ConnectError as e:
            print(f"Connection failed (DNS, proxy, SSL, or network): {e}")
        except primp.StatusError as e:
            print(f"HTTP {e.status_code} error")
        except primp.PrimpError as e:
            print(f"Other primp error: {e}")
    return None

asyncio.run(fetch_with_retry("https://httpbin.org/get"))
```

### Handling Request Errors

Use `RequestError` to catch both `ConnectError` and `TimeoutError`:

```python
import asyncio
import primp

async def fetch(url: str):
    async with primp.AsyncClient(timeout=10) as client:
        try:
            resp = await client.get(url)
            return resp.text
        except primp.RequestError as e:
            # Catches both ConnectError and TimeoutError
            print(f"Request failed: {e}")
            return None

asyncio.run(fetch("https://httpbin.org/get"))
```

### Concurrent Requests with Error Handling

Handle errors individually for each concurrent request:

```python
import asyncio
import primp

async def fetch_safe(client: primp.AsyncClient, url: str):
    """Fetch URL and return (url, result_or_error) tuple."""
    try:
        resp = await client.get(url, timeout=5)
        resp.raise_for_status()
        return (url, resp.text)
    except primp.TimeoutError:
        return (url, "TIMEOUT")
    except primp.ConnectError:
        return (url, "CONNECTION_ERROR")
    except primp.StatusError as e:
        return (url, f"HTTP_{e.status_code}")
    except primp.PrimpError as e:
        return (url, f"ERROR: {e}")

async def main():
    urls = [
        "https://httpbin.org/get",
        "https://httpbin.org/delay/10",  # Will timeout
        "https://httpbin.org/status/404",  # Will return 404
        "https://nonexistent.invalid/",  # DNS error
    ]
    
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        tasks = [fetch_safe(client, url) for url in urls]
        results = await asyncio.gather(*tasks)
        
        for url, result in results:
            print(f"{url}: {result}")

asyncio.run(main())
```

### Streaming Error Handling

Handle errors during async streaming:

```python
import asyncio
import primp

async def stream_with_error_handling(url: str):
    async with primp.AsyncClient(timeout=30) as client:
        try:
            resp = await client.get(url)
            
            # Stream with error handling
            async for chunk in resp:
                try:
                    process_chunk(chunk)
                except Exception as e:
                    print(f"Error processing chunk: {e}")
                    break
                    
        except primp.TimeoutError:
            print("Timeout while streaming")
        except primp.BodyError as e:
            print(f"Body/stream error: {e}")
        except primp.DecodeError as e:
            print(f"Content decoding failed: {e}")

def process_chunk(chunk: bytes):
    # Your processing logic here
    pass

asyncio.run(stream_with_error_handling("https://example.com/large-file"))
```

### Complete Async Error Handling Example

```python
import asyncio
import primp

async def robust_fetch(url: str):
    """Fetch URL with comprehensive error handling."""
    async with primp.AsyncClient(
        impersonate="chrome_145",
        timeout=10,
        follow_redirects=True,
        max_redirects=5,
    ) as client:
        try:
            resp = await client.get(url)
            resp.raise_for_status()
            data = resp.json()
            return data
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
    return None

asyncio.run(robust_fetch("https://httpbin.org/get"))
