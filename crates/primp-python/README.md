![Python >= 3.10](https://img.shields.io/badge/python->=3.10-red.svg) [![](https://badgen.net/github/release/deedy5/primp)](https://github.com/deedy5/primp/releases) [![](https://badge.fury.io/py/primp.svg)](https://pypi.org/project/primp) [![Downloads](https://static.pepy.tech/badge/primp/week)](https://pepy.tech/project/primp) [![CI](https://github.com/deedy5/primp/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/deedy5/primp/actions/workflows/ci.yml)

# 🪞 PRIMP 🐍

> HTTP client that can impersonate web browsers

## 📦 Installation

```bash
pip install -U primp
```

## 🔧 Building from Source

```bash
git clone https://github.com/deedy5/primp.git && cd primp/crates/primp-python
python -m venv .venv && source .venv/bin/activate
pip install maturin && maturin develop -r
```

## 🚀 Quick Start

### Sync API

```python
import primp

client = primp.Client(impersonate="chrome_146")
resp = client.get("https://tls.peet.ws/api/all")
print(resp.text)
```

### Async API

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_146") as client:
        resp = await client.get("https://tls.peet.ws/api/all")
        print(resp.text)

asyncio.run(main())
```
### More examples

 > See the [examples](examples/) directory.

## 📊 Benchmark

![](./benchmark/benchmark.jpg)

## 🎭 Browser profiles

| Browser | Profiles |
|:--------|:---------|
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera` |
| Random | `random` |

**OS:** `android`, `ios`, `linux`, `macos`, `windows`, `random`

## 📖 Documentation

- [Browser Impersonation](docs/impersonate.md) — profiles, OS, TLS/HTTP2 fingerprinting
- [Client](docs/client.md) — constructor, methods, authentication
- [AsyncClient](docs/async_client.md) — async API, concurrency
- [Response](docs/response.md) — properties, streaming
- [Exceptions](docs/exceptions.md) — error handling

____
### Disclaimer

This tool is for educational purposes only. Use it at your own risk.
