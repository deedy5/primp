# 🪞 PRIMP 🦀

> HTTP client that can impersonate web browsers.

## 📦 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
primp = { git = "https://github.com/deedy5/primp", branch = "main" }
```

## 🔧 Building from Source

```bash
git clone https://github.com/deedy5/primp.git && cd primp

# Build primp
cargo build -r -p primp

# Test primp
cargo test -r -p primp
```

## 🚀 Quick Start

### Async API

```rust
use primp::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with browser impersonation
    let client = Client::builder()
        .impersonate("chrome_145")
        .build()?;

    // Make a request
    let resp = client.get("https://tls.peet.ws/api/all").send().await?;
    
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text().await?);
    
    Ok(())
}
```

### Blocking API

```rust
use primp::blocking::Client;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::builder()
        .impersonate("chrome_145")
        .build()?;

    let resp = client.get("https://httpbin.org/get").send()?;
    
    println!("Status: {}", resp.status());
    println!("Body: {}", resp.text()?);
    
    Ok(())
}
```

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

- 🔥 **Fast** - Built on hyper and tokio
- 🎭 **Browser Impersonation** - Mimic Chrome, Safari, Firefox, Edge, Opera
- 🌍 **OS Impersonation** - Windows, Linux, macOS, Android, iOS
- 🔄 **Sync & Async** - Both APIs available
- 🍪 **Cookie Management** - Persistent cookie store
- 🌐 **Proxy Support** - HTTP, HTTPS, SOCKS5
- 📤 **Multipart** - Form data and file uploads
- 🔐 **TLS** - rustls backend
- 📦 **Compression** - gzip, brotli, deflate, zstd

## 📖 Examples

See the [examples](examples/) directory for more usage examples.

___
## Disclaimer

This tool is for educational purposes only. Use it at your own risk.
