"""Basic usage examples for primp Client."""

import primp

# Simple GET request
client = primp.Client()
resp = client.get("https://httpbin.org/get")
print(f"Status: {resp.status_code}")
print(f"Body: {resp.text[:100]}...")

# Browser impersonation
client = primp.Client(impersonate="chrome_146", impersonate_os="windows")
resp = client.get("https://tls.peet.ws/api/all")
print(f"TLS fingerprint: {resp.json()['tls']['ja3']}")

# Query parameters
resp = client.get("https://httpbin.org/get", params={"key": "value", "foo": "bar"})
print(f"Params: {resp.json()['args']}")

# Custom headers
resp = client.get("https://httpbin.org/headers", headers={"X-Custom-Header": "custom-value"})
print(f"Headers: {resp.json()['headers']}")

# Timeout configuration
client = primp.Client(
    timeout=30,
    connect_timeout=10,
    read_timeout=15,
)
resp = client.get("https://httpbin.org/delay/5")
print(f"Status: {resp.status_code}")

# Base URL for relative paths
client = primp.Client(base_url="https://httpbin.org")
resp = client.get("/get")
print(f"Status: {resp.status_code}")

# Per-request follow_redirects
client = primp.Client(follow_redirects=True)
resp = client.get("https://httpbin.org/redirect/1", follow_redirects=False)
print(f"Status: {resp.status_code}")
