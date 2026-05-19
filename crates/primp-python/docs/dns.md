# DNS Resolution

Control how DNS lookups are performed by passing `dns_resolver` to `Client()` or `AsyncClient()`.

## Quick Reference

| Value | Behavior |
|-------|----------|
| `None` (default) | System resolver |
| `"system"` | System resolver |
| `"doh://.../dns-query"` | DNS-over-HTTPS |
| `"dot://1.1.1.1"` | DNS-over-TLS |
| `"dns://1.1.1.1"` | Plain DNS on port 53 |
| `"1.1.1.1"` | Plain DNS on port 53 (shorthand) |
| `["doh://...", "dot://..."]` | Fallback chain (first success wins) |

## Examples

**System resolver (default):**
```python
client = primp.Client()
```

**DNS-over-HTTPS via Cloudflare:**
```python
client = primp.Client(dns_resolver="doh://cloudflare-dns.com/dns-query")
```

**DNS-over-TLS:**
```python
client = primp.Client(dns_resolver="dot://1.1.1.1")
```

**Plain DNS:**
```python
client = primp.Client(dns_resolver="1.1.1.1")
# or explicitly:
client = primp.Client(dns_resolver="dns://1.1.1.1")
```

**Fallback chain: try DoH first, fall back to plain DNS:**
```python
client = primp.Client(dns_resolver=["doh://cloudflare-dns.com/dns-query", "1.1.1.1"])
```

**Fallback chain including system resolver:**
```python
client = primp.Client(dns_resolver=["doh://cloudflare-dns.com/dns-query", "system", "1.1.1.1"])
```

**Async:**
```python
client = primp.AsyncClient(dns_resolver="dot://dns.google")
```

## Order Matters

Resolvers in a list are tried in order. The first one that succeeds wins.

```python
# Try DoH first, fall back to system
client = primp.Client(dns_resolver=["doh://cloudflare-dns.com/dns-query", "system"])
```

## Scheme Reference

| Scheme | Protocol | Default Port | Example |
|--------|----------|-------------|---------|
| `doh://` | HTTPS (DoH) | 443 | `doh://cloudflare-dns.com/dns-query` |
| `dot://` | TLS (DoT) | 853 | `dot://1.1.1.1` |
| `dns://` | UDP/TCP | 53 | `dns://8.8.8.8` |
| *(none)* | UDP/TCP (plain) | 53 | `1.1.1.1` |
| `system` | OS `getaddrinfo` | — | `system` |
