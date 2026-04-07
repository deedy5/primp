[![GitHub stars](https://img.shields.io/github/stars/deedy5/primp?style=social)](https://github.com/deedy5/primp/stargazers)
[![Forks](https://img.shields.io/github/forks/deedy5/primp?style=social)](https://github.com/deedy5/primp/forks)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.70%2B-orange.svg?logo=rust)](https://rust-lang.org)
[![Python](https://img.shields.io/badge/Python-3.8%2B-blue.svg?logo=python)](https://python.org)

# 🪞 PRIMP 🦀🐍

> HTTP client that can impersonate web browsers

## Quick Start

### 🦀 Rust → [/crates/primp](./crates/primp)

```toml
[dependencies]
primp = "1.2.2"
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
| Chrome | `chrome_144`, `chrome_145`, `chrome_146`, `chrome` |
| Safari | `safari_18.5`, `safari_26`, `safari_26.3`, `safari` |
| Edge | `edge_144`, `edge_145`, `edge_146`, `edge` |
| Firefox | `firefox_140`, `firefox_146`, `firefox_147`, `firefox_148`, `firefox` |
| Opera | `opera_126`, `opera_127`, `opera_128`, `opera_129`, `opera` |
| Random | `random` |

**OS:** `android`, `ios`, `linux`, `macos`, `windows`, `random`
____
### Disclaimer

This tool is for educational purposes only. Use it at your own risk.
