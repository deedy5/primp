![Python >= 3.10](https://img.shields.io/badge/python->=3.10-red.svg) [![](https://badgen.net/github/release/deedy5/pyreqwest-impersonate)](https://github.com/deedy5/pyreqwest-impersonate/releases) [![](https://badge.fury.io/py/primp.svg)](https://pypi.org/project/primp) [![Downloads](https://static.pepy.tech/badge/primp/week)](https://pepy.tech/project/primp) [![CI](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/pyreqwest-impersonate/actions/workflows/CI.yml)

# 🪞 PRIMP

**P**ython **R**equests **IMP**ersonate | The fastest Python HTTP client that can impersonate web browsers.

## 📦 Installation

```bash
pip install -U primp
```

## 🚀 Quick Start

### Sync API

```python
import primp

# Create client with browser impersonation
client = primp.Client(impersonate="chrome_145")

# Make requests
resp = client.get("https://httpbin.org/get")
print(resp.status_code)  # 200
print(resp.text)         # Response body
print(resp.json())       # Parsed JSON
```

### Async API

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        resp = await client.get("https://httpbin.org/get")
        print(await resp.text)

asyncio.run(main())
```

## 📊 Benchmark

![](https://github.com/deedy5/primp/blob/main/benchmark.jpg?raw=true)

## 🎭 Browser Impersonation

| Browser | Profiles |
|:--------|:---------|
| 🌐 **Chrome** | `chrome_144`, `chrome_145` |
| 🧭 **Safari** | `safari_18.5`, `safari_26` |
| 🔷 **Edge** | `edge_144`, `edge_145` |
| 🦊 **Firefox** | `firefox_140`, `firefox_146` |
| ⭕ **Opera** | `opera_126`, `opera_127` |
| 🎲 **Random** | `random` |

## 💻 OS Impersonation

| OS | Value |
|:---|:------|
| 🤖 Android | `android` |
| 🍎 iOS | `ios` |
| 🐧 Linux | `linux` |
| 🍏 macOS | `macos` |
| 🪟 Windows | `windows` |
| 🎲 Random | `random` |

## ⚡ Features

- 🔥 **Fast** - Built with Rust
- 🎭 **Browser Impersonation** - Mimic Chrome, Safari, Firefox, Edge, Opera
- 🌍 **OS Impersonation** - Windows, Linux, macOS, Android, iOS
- 🔄 **Sync & Async** - Both APIs available
- 🍪 **Cookie Management** - Persistent cookie store
- 🌐 **Proxy Support** - HTTP, HTTPS, SOCKS5
- 📝 **HTML Conversion** - Convert to markdown, plain text, rich text
- 📤 **File Uploads** - Multipart/form-data support
- 🔐 **SSL Verification** - Custom CA certificates

## 📖 Response Object

```python
resp.status_code      # HTTP status code
resp.text             # Response body as text
resp.content          # Response body as bytes
resp.json()           # Parsed JSON
resp.headers          # Response headers
resp.cookies          # Response cookies
resp.url              # Final URL
resp.encoding         # Character encoding

# HTML conversion
resp.text_markdown    # HTML → Markdown
resp.text_plain       # HTML → Plain text
resp.text_rich        # HTML → Rich text

# Streaming
for chunk in resp.stream():
    print(chunk)
```

## 📚 More Examples

See the [`/examples`](examples/) folder for detailed usage:

- [`basic_usage.py`](examples/basic_usage.py) - GET, POST, params, headers
- [`async_usage.py`](examples/async_usage.py) - Async client, concurrent requests
- [`authentication.py`](examples/authentication.py) - Basic auth, bearer tokens
- [`proxy.py`](examples/proxy.py) - HTTP/SOCKS5 proxies
- [`cookies.py`](examples/cookies.py) - Cookie management
- [`streaming.py`](examples/streaming.py) - Response streaming
- [`post_requests.py`](examples/post_requests.py) - POST/PUT/PATCH/DELETE
- [`error_handling.py`](examples/error_handling.py) - Exception handling
- [`html_conversion.py`](examples/html_conversion.py) - HTML to text

___
## Disclaimer

This tool is for educational purposes only. Use it at your own risk.
