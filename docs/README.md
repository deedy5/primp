# PRIMP Documentation

**PRIMP** = **P**ython **R**equests **IMP**ersonate

The fastest Python HTTP client that can impersonate web browsers.

## Installation

```bash
pip install -U primp
```

## Features

- Browser impersonation (Chrome, Safari, Edge, Firefox, Opera)
- OS impersonation (Windows, Linux, macOS, Android, iOS)
- Sync and async clients
- Cookie management
- Proxy support (HTTP, SOCKS5)
- SSL certificate verification
- Response streaming
- File uploads (multipart/form-data)
- HTML to text/markdown conversion

## Quick Start

```python
import primp

# Sync client
client = primp.Client(impersonate="chrome_145")
resp = client.get("https://httpbin.org/get")
print(resp.text)

# Async client
import asyncio

async def main():
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        resp = await client.get("https://httpbin.org/get")
        print(await resp.text)

asyncio.run(main())
```

## Browser Impersonation

| Browser | Profiles |
|---------|----------|
| Chrome | `chrome_144`, `chrome_145` |
| Safari | `safari_18.5`, `safari_26` |
| Edge | `edge_144`, `edge_145` |
| Firefox | `firefox_140`, `firefox_146` |
| Opera | `opera_126`, `opera_127` |
| Random | `random` |

## OS Impersonation

| OS | Value |
|----|-------|
| Android | `android` |
| iOS | `ios` |
| Linux | `linux` |
| macOS | `macos` |
| Windows | `windows` |
| Random | `random` |

## Documentation

- [Client](client.md) - Synchronous HTTP client
- [AsyncClient](async_client.md) - Asynchronous HTTP client
- [Response](response.md) - Response object
- [Exceptions](exceptions.md) - Exception types
