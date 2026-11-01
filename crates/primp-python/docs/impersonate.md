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
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome_147`, `chrome_148`, `chrome_150`, `chrome_150_0_7871_187`, `chrome_156`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge_147`, `edge_148`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera_130`, `opera_131`, `opera` |
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

## Custom fingerprints via `impersonate_overrides`

If a browser is newer than the built-in profiles, you can layer overrides on
top of the closest hard-coded profile without waiting for a library release.
The overrides are applied after the base profile is resolved, so start from
whatever is closest and only patch what changed.

```python
import primp

client = primp.Client(
    impersonate="chrome_150",  # closest built-in base
    impersonate_overrides={
        # HTTP-level (peet.ws does not carry these — supply them yourself)
        "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) "
                       "AppleWebKit/537.36 (KHTML, like Gecko) "
                       "Chrome/151.0.0.0 Safari/537.36",
        "sec_ch_ua": '"Google Chrome";v="151", "Chromium";v="151", "Not?A_Brand";v="24"',
        "sec_ch_ua_platform": '"Windows"',
        "sec_ch_ua_mobile": "?0",
        "headers": {"accept-language": "ru,en-US;q=0.9,en;q=0.8"},
        # HTTP/2 header order (Chrome 148+ moved sec-ch-ua after sec-fetch-*)
        "headers_order": [
            "sec-ch-ua", "sec-ch-ua-mobile", "sec-ch-ua-platform",
            "upgrade-insecure-requests", "user-agent", "accept",
            "sec-fetch-site", "sec-fetch-mode", "sec-fetch-user", "sec-fetch-dest",
            "accept-encoding", "accept-language", "priority",
        ],
        # TLS: numeric IDs (decimal or "0x…" hex strings)
        "signature_algorithms": [
            0x0904, 0x0905, 0x0906, 0x0403, 0x0804, 0x0401,
            0x0503, 0x0805, 0x0501, 0x0806, 0x0601,
        ],
        # HTTP/2 SETTINGS (id, value), in the exact wire order
        "http2_settings": [(1, 65536), (2, 0), (4, 6291456), (6, 262144)],
        "http2_pseudo_order": "masp",
        "http2_headers_priority": (255, 0, True),
        "http2_initial_connection_window_size": 15663105,
    },
)
```

### Bootstrapping from `tls.peet.ws`

`https://tls.peet.ws` has two endpoints — pass either response dict directly and
the format is auto-detected.

**`/api/all` (recommended)** — carries the full request, including
`User-Agent`, `sec-ch-ua*`, header wire order, and the HEADERS-frame priority
block. All of that is applied automatically:

```python
import primp
import requests

peet = requests.get("https://tls.peet.ws/api/all",
                    headers={"User-Agent": "…what you want to mimic…"}).json()

client = primp.Client(
    impersonate="chrome_150",
    impersonate_overrides={"peet_api_response": peet},
    # Auto-filled from the payload: user_agent, sec-ch-ua*, headers_order,
    # http2_headers_priority, TLS ciphers/sig-algs/groups, HTTP/2 SETTINGS.
    # Anything you set explicitly still wins.
)
```

**`/api/clean`** — compact `peetprint` + `akamai` only (no HTTP-level info).
Supply headers yourself:

```python
peet = requests.get("https://tls.peet.ws/api/clean").json()

client = primp.Client(
    impersonate="chrome_150",
    impersonate_overrides={
        "peet_api_response": peet,   # TLS/HTTP2 fingerprint only
        "user_agent": "Mozilla/5.0 … Chrome/151.0.0.0 …",
        "sec_ch_ua": '"Chromium";v="151", "Google Chrome";v="151", "Not?A_Brand";v="24"',
        "headers": {"accept-language": "en-US,en;q=0.9"},
    },
)
```

Or pass the individual strings directly, without the wrapper dict:

```python
"peetprint": peet["peetprint"],   # from /api/clean or /api/all["tls"]
"akamai":    peet["akamai"],       # from /api/clean or /api/all["http2"]["akamai_fingerprint"]
```

### Supported override keys

| Key | Type | Purpose |
|:----|:-----|:--------|
| `user_agent` | `str` | Replace the `User-Agent` header |
| `sec_ch_ua` | `str` | Replace `sec-ch-ua` |
| `sec_ch_ua_platform` | `str` | Replace `sec-ch-ua-platform` |
| `sec_ch_ua_mobile` | `str` | Replace `sec-ch-ua-mobile` (`"?0"` / `"?1"`) |
| `headers` | `dict[str, str]` | Extra headers, inserted/overwritten verbatim |
| `headers_order` | `list[str]` | Explicit HTTP/2 header order |
| `cipher_suites` | `list[int \| str]` | TLS cipher suites in wire order; ints or `"0x1301"` strings |
| `signature_algorithms` | `list[int \| str]` | TLS signature algorithms in wire order |
| `named_groups` | `list[int \| str]` | TLS named groups in wire order |
| `extension_order_seed` | `int` | Seed for the rustls extension-order permutation |
| `http2_settings` | `list[(int, int)]` | HTTP/2 SETTINGS pairs `(id, value)` in wire order |
| `http2_pseudo_order` | `str \| list[str]` | Pseudo-header order, e.g. `"masp"` or `["m","a","s","p"]` |
| `http2_headers_priority` | `tuple[int, int, bool] \| None` | `(weight, dep, exclusive)` or `None` to drop the PRIORITY flag |
| `http2_initial_stream_window_size` | `int` | Overrides SETTINGS_INITIAL_WINDOW_SIZE |
| `http2_initial_connection_window_size` | `int` | WINDOW_UPDATE increment sent after SETTINGS |
| `http2_header_table_size` | `int` | Overrides SETTINGS_HEADER_TABLE_SIZE |
| `http2_max_header_list_size` | `int` | Overrides SETTINGS_MAX_HEADER_LIST_SIZE |
| `peet_api_response` | `dict` | Full JSON from `tls.peet.ws/api/clean`; convenience for the four fields above |
| `peetprint`, `akamai` | `str` | The individual strings from `peet_api_response` |

### Known limits

* The `/api/clean` payload does **not** include HTTP headers, `User-Agent`,
  `sec-ch-ua*`, `Accept*`, cookies, or `headers_order`. Supply those yourself
  based on the browser you are impersonating.
* TLS extension **order** is not derived from peetprint — the underlying
  rustls emulator picks it from `extension_order_seed`. When the base
  profile is a Chrome variant this yields Chrome's order; other layouts need
  a manually chosen seed.
* GREASE markers in peetprint are stripped; rustls injects GREASE where
  required.
* Overrides cannot **add** TLS extensions. If a new browser build starts
  sending an extension the base profile lacks (e.g. Chrome 156's
  `trust_anchors`, `51764` / `0xCA34`), JA3/JA4/peetprint will differ until a
  built-in profile for that build exists — use `chrome_156` for that one.
