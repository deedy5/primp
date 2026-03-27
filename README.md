# 🪞 PRIMP 🦀🐍

> HTTP client that can impersonate web browsers

## Quick Start

### 🦀 Rust → [/crates/primp](./crates/primp)

```toml
[dependencies]
primp = { git = "https://github.com/deedy5/primp" }
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
