"""Basic usage examples for primp Client."""

import primp

# Simple GET request
client = primp.Client()
resp = client.get("https://httpbin.org/get")
print(f"Status: {resp.status_code}")
print(f"Body: {resp.text[:100]}...")

# Browser impersonation
client = primp.Client(impersonate="chrome_145", impersonate_os="windows")
resp = client.get("https://tls.peet.ws/api/all")
print(f"TLS fingerprint: {resp.json()['tls']['ja3']}")

# Query parameters
resp = client.get("https://httpbin.org/get", params={"key": "value", "foo": "bar"})
print(f"Params: {resp.json()['args']}")

# Custom headers
resp = client.get(
    "https://httpbin.org/headers",
    headers={"X-Custom-Header": "custom-value"}
)
print(f"Headers: {resp.json()['headers']}")

# Cookies
resp = client.get(
    "https://httpbin.org/cookies",
    cookies={"session": "abc123"}
)
print(f"Cookies: {resp.json()['cookies']}")

# Timeout
resp = client.get("https://httpbin.org/delay/1", timeout=5)
print(f"Status: {resp.status_code}")

# Convenience functions (one-off requests with impersonation)
resp = primp.Client(impersonate="chrome_145").get("https://httpbin.org/get")
print(f"Status: {resp.status_code}")
