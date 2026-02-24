![Python >= 3.10](https://img.shields.io/badge/python->=3.10-red.svg) [![](https://badgen.net/github/release/deedy5/primp)](https://github.com/deedy5/primp/releases) [![](https://badge.fury.io/py/primp.svg)](https://pypi.org/project/primp) [![Downloads](https://static.pepy.tech/badge/primp/week)](https://pepy.tech/project/primp) [![CI](https://github.com/deedy5/primp/actions/workflows/CI.yml/badge.svg?branch=main)](https://github.com/deedy5/primp/actions/workflows/CI.yml)

# 🪞 PRIMP 🐍

> HTTP client that can impersonate web browsers.

## 📦 Installation

```bash
pip install -U primp
```

## 🔧 Building from Source

```bash
git clone https://github.com/deedy5/primp.git && cd primp

# Build python library using cargo
cargo build -r -p primp-python

# Build python library using maturin
cd crates/primp-python
python -m venv .venv && source .venv/bin/activate  # Linux/macOS
pip install maturin && maturin develop -r
```

## 🚀 Quick Start

### Sync API

```python
import primp

# Create client with browser impersonation
client = primp.Client(impersonate="chrome_145")

# Make requests
resp = client.get("https://tls.peet.ws/api/all")
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
        resp = await client.get("https://tls.peet.ws/api/all")
        print(resp.text)

asyncio.run(main())
```

## 📊 Benchmark

![](./benchmark/benchmark.jpg)

## 🎭 Browser Impersonation

<table>
<tr>
<td valign="top">

| Browser | Profiles |
|:--------|:---------|
| 🌐 **Chrome** | `chrome_144`, `chrome_145` |
| 🧭 **Safari** | `safari_18.5`, `safari_26` |
| 🔷 **Edge** | `edge_144`, `edge_145` |
| 🦊 **Firefox** | `firefox_140`, `firefox_146` |
| ⭕ **Opera** | `opera_126`, `opera_127` |
| 🎲 **Random** | `random` |

</td>
<td valign="top">

| OS | Value |
|:---|:------|
| 🤖 Android | `android` |
| 🍎 iOS | `ios` |
| 🐧 Linux | `linux` |
| 🍏 macOS | `macos` |
| 🪟 Windows | `windows` |
| 🎲 Random | `random` |

</td>
</tr>
</table>

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

### Standard Response

All properties work identically for both sync and async responses:

| Property | Type | Description |
|:---------|:-----|:------------|
| `.url` | `str` | Final response URL (after redirects) |
| `.status_code` | `int` | HTTP status code |
| `.content` | `bytes` | Raw response body as bytes |
| `.encoding` | `str` | Character encoding (read/write) |
| `.text` | `str` | Decoded text content |
| `.headers` | `dict` | Response headers dictionary |
| `.cookies` | `dict` | Response cookies |
| `.text_markdown` | `str` | HTML converted to Markdown |
| `.text_plain` | `str` | HTML converted to plain text |
| `.text_rich` | `str` | HTML converted to rich text |

| Method | Description |
|:-------|:------------|
| `.json()` | Parse response body as JSON. Raises `json.JSONDecodeError` on invalid JSON. |
| `.raise_for_status()` | Raise `StatusError` for 4xx/5xx status codes |

### Streaming Response

Use `stream=True` to get a `StreamResponse` for efficient handling of large data:

```python
# Sync streaming
with primp.get("https://example.com/large-file.zip", stream=True) as response:
    for chunk in response.iter_bytes():
        process(chunk)

# Async streaming
async with await client.get("https://example.com/large-file.zip", stream=True) as response:
    async for chunk in response.aiter_bytes():
        process(chunk)
```

| Sync Method | Async Method | Description |
|:------------|:-------------|:------------|
| `.read()` | `.aread()` | Read remaining content into memory |
| `.iter_bytes(chunk_size)` | `.aiter_bytes(chunk_size)` | Iterate over byte chunks |
| `.iter_text(chunk_size)` | `.aiter_text(chunk_size)` | Iterate over decoded text chunks |
| `.iter_lines()` | `.aiter_lines()` | Iterate over lines |
| `.next()` | `.anext()` | Get next chunk explicitly |
| `.close()` | `.aclose()` | Close response and release resources |

## 📚 Examples

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
