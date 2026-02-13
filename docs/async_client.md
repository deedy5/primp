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
        print(await resp.text)

asyncio.run(main())
```

## Methods

All methods return awaitable futures:

```python
# HTTP methods
await client.get(url, ...)
await client.head(url, ...)
await client.options(url, ...)
await client.delete(url, ...)
await client.post(url, content=None, data=None, json=None, files=None, ...)
await client.put(url, ...)
await client.patch(url, ...)

# Cookie management
await client.set_cookies(url, cookies)
cookies = await client.get_cookies(url)

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
    return await resp.text

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

All response properties are awaitable:

```python
resp = await client.get(url)

# Await these properties
content = await resp.content
text = await resp.text
json = await resp.json
headers = await resp.headers
cookies = await resp.cookies
encoding = await resp.encoding

# Sync properties
url = resp.url
status_code = resp.status_code
```

## Streaming

```python
async def stream_response():
    async with primp.AsyncClient() as client:
        resp = await client.get("https://example.com/large-file")
        async for chunk in await resp.stream():
            print(chunk)
```
