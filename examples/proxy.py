"""Proxy examples for primp."""

import primp

# HTTP proxy
client = primp.Client(proxy="http://127.0.0.1:8080")
resp = client.get("https://httpbin.org/ip")
print(f"HTTP proxy: {resp.json()}")

# SOCKS5 proxy
client = primp.Client(proxy="socks5://127.0.0.1:1080")
resp = client.get("https://httpbin.org/ip")
print(f"SOCKS5 proxy: {resp.json()}")

# Proxy with authentication
client = primp.Client(proxy="http://user:pass@proxy.example.com:8080")
resp = client.get("https://httpbin.org/ip")

# Environment variable proxy
export PRIMP_PROXY="http://127.0.0.1:8080"
client = primp.Client()
resp = client.get("https://httpbin.org/ip")

# Update proxy dynamically
client = primp.Client()
client.proxy = "http://127.0.0.1:8080"
print(f"Current proxy: {client.proxy}")

# Disable SSL verification (useful for proxy debugging)
client = primp.Client(proxy="http://127.0.0.1:8080", verify=False)
resp = client.get("https://httpbin.org/ip")
print(f"No SSL verify: {resp.json()}")
