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

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_146") as client:
        resp = await client.get("https://httpbin.org/get")
        print(resp.text)

asyncio.run(main())
```

## Methods

All request methods return awaitable futures. Same parameters as [Client](client.md).

```python
await client.get(url, ...)
await client.head(url, ...)
await client.options(url, ...)
await client.delete(url, ...)
await client.post(url, content=None, data=None, json=None, files=None, ...)
await client.put(url, ...)
await client.patch(url, ...)
```

### Cookie / Header / Proxy Management

Same as synchronous [Client](client.md).

## Concurrent Requests

```python
import asyncio
import primp

async def main():
    urls = [
        "https://httpbin.org/get",
        "https://httpbin.org/ip",
        "https://httpbin.org/headers",
    ]

    async with primp.AsyncClient(impersonate="chrome_146") as client:
        tasks = [client.get(url) for url in urls]
        responses = await asyncio.gather(*tasks)
        for resp in responses:
            print(f"{resp.url}: {resp.status_code}")

asyncio.run(main())
```

## Response

See [response.md](response.md) for response properties, streaming, and async iteration.

## Exceptions

See [exceptions.md](exceptions.md) for the full exception hierarchy and handling examples.
