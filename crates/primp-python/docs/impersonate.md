# Browser Impersonation

primp can impersonate real browsers by matching their TLS fingerprints, HTTP/2 settings, and headers.

## Usage

```python
import primp

# Specific browser version
client = primp.Client(impersonate="chrome_146")

# Browser + OS
client = primp.Client(impersonate="chrome_146", impersonate_os="windows")

# Random version from a browser family
client = primp.Client(impersonate="chrome")

# Completely random
client = primp.Client(impersonate="random")
```

## Browser Profiles

| Browser | Profiles |
|:--------|:---------|
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera` |
| Random | `random` |

Specific versions (e.g., `chrome_146`) pin a single browser version. Family selectors (e.g., `chrome`) pick a random version from that browser family. `random` picks any browser randomly.

## OS Profiles

`impersonate_os` controls the OS-specific TLS and header values:

| Value | Description |
|:------|:------------|
| `android` | Android |
| `ios` | iOS |
| `linux` | Linux |
| `macos` | macOS |
| `windows` | Windows |
| `random` | Random OS |

If `impersonate_os` is not set, `random` is used automatically.

## What Gets Impersonated

- **TLS fingerprint**: cipher suites, signature algorithms, named groups, extension order
- **HTTP/2 fingerprint**: SETTINGS order, pseudo-header order, header priority, header order, initial window sizes
- **Headers**: User-Agent, sec-ch-ua, Accept, Accept-Language, Accept-Encoding, sec-fetch-*, etc.
- **Compression**: gzip, brotli, zstd support per browser

## Async

```python
import asyncio
import primp

async def main():
    async with primp.AsyncClient(impersonate="chrome_146") as client:
        resp = await client.get("https://tls.peet.ws/api/all")
        print(resp.json())

asyncio.run(main())
```
