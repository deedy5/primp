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

## Fingerprint Comparison

Target: `https://tls.browserleaks.com/json` | Reference: Chrome 146 / Linux

```json
{
  "user_agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
  "ja4": "t13d1516h2_8daaf6152771_d8a2da3f94cd",
  "ja4_r": "t13d1516h2_002f,0035,009c,009d,1301,1302,1303,c013,c014,c02b,c02c,c02f,c030,cca8,cca9_0005,000a,000b,000d,0012,0017,001b,0023,002b,002d,0033,44cd,fe0d,ff01_0403,0804,0401,0503,0805,0501,0806,0601",
  "ja4_o": "t13d1516h2_acb858a92679_a4c42e1680d9",
  "ja4_ro": "t13d1516h2_1301,1302,1303,c02b,c02f,c02c,c030,cca9,cca8,c013,c014,009c,009d,002f,0035_fe0d,002b,0012,44cd,000a,ff01,0033,0005,0023,0000,000d,0010,000b,001b,0017,002d_0403,0804,0401,0503,0805,0501,0806,0601",
  "akamai_hash": "52d84b11737d980aef856699f885ca86",
  "akamai_text": "1:65536;2:0;4:6291456;6:262144|15663105|0|m,a,s,p"
}
```

| Fingerprint | Primp | curl-cffi (Chrome 142)* |
|:---|:---:|:---:|
| **user_agent** | ✅ | ❌ |
| **ja4** | ✅ | ✅ |
| **ja4_r** | ✅ | ✅ |
| **ja4_o** | ✅ | ❌ |
| **ja4_ro** | ✅ | ❌ |
| **akamai_hash** | ✅ | ✅ |
| **akamai_text** | ✅ | ✅ |
| **Total** | **7/7** | **4/7** |

> \* curl-cffi maxes at `chrome142` (macOS). No OS parameter — OS is baked into each profile. Primp supports independent OS selection via `impersonate_os`.

____
### Disclaimer

This tool is for educational purposes only. Use it at your own risk.
