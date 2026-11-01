# 🪞 PRIMP 🦀🐍

> HTTP client that can impersonate web browsers

## Quick Start

### 🦀 Rust → [/crates/primp](./crates/primp)

```toml
[dependencies]
primp = "1"
```

```rust
use primp::{Client, Impersonate};

#[tokio::main]
async fn main() -> Result<(), primp::Error> {
    let client = Client::builder()
        .impersonate(Impersonate::ChromeV146)
        .build()?;
    let resp = client.get("https://tls.peet.ws/api/all").send().await?;
    println!("Body: {}", resp.text().await?);
    Ok(())
}
```

### 🐍 Python → [/crates/primp-python](./crates/primp-python)

```bash
pip install primp
```

```python
import primp

client = primp.Client(impersonate="chrome_146")
resp = client.get("https://tls.peet.ws/api/all")
print(resp.text)
```

## Browser profiles

| Browser | Profiles |
|:--------|:---------|
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome_147`, `chrome_148`, `chrome_150`, `chrome_150_0_7871_187`, `chrome_156`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge_147`, `edge_148`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera_130`, `opera_131`, `opera` |
| Random | `random` |

**OS:** `android`, `ios`, `linux`, `macos`, `windows`, `random`

## Custom fingerprints without a library update

If a new browser build changes the fingerprint, layer overrides on top of the
closest built-in profile — no library release required.

**Python** — full docs: [`crates/primp-python/docs/impersonate.md`](./crates/primp-python/docs/impersonate.md):

```python
import primp, requests
peet = requests.get("https://tls.peet.ws/api/all",
                    headers={"User-Agent": "…the UA you want to mimic…"}).json()

client = primp.Client(
    impersonate="chrome_150",           # closest built-in base
    impersonate_overrides={"peet_api_response": peet},
    # /api/all also fills user_agent, sec-ch-ua*, headers_order,
    # http2_headers_priority — you rarely need to set them yourself.
)
```

**Rust:**

```rust
use primp::{Client, Impersonate, ProfileOverrides};

let overrides = ProfileOverrides::from_peet_api(peetprint, akamai)?;
let client = Client::builder()
    .impersonate(Impersonate::ChromeV150)
    .impersonate_overrides(overrides)
    .build()?;
```

For a fully hand-built profile, use `.impersonate_settings(BrowserSettings)`.

## Releasing

Releases are automatic. CI (`.github/workflows/ci.yml`) builds the Windows
x64 wheel (`primp-<version>-cp310-abi3-win_amd64.whl`, Python 3.10+) on every
pull request to `main`. When a PR is merged, CI checks the `version` in
`Cargo.toml` (it must match `crates/primp` and `crates/primp-python`); if there
is no `v<version>` tag yet, it creates the tag and a GitHub Release with that
wheel attached.

To release: bump the version in those three `Cargo.toml` files in your PR and
merge it. A PR that doesn't bump the version builds and tests but releases
nothing. If a `PYPI_API_TOKEN` secret is set, the wheel is also uploaded to
PyPI.

____
### Disclaimer

This tool is for educational purposes only. Use it at your own risk.
