"""Authentication examples for primp."""

import primp

# Basic authentication
client = primp.Client(auth=("username", "password"))
resp = client.get("https://httpbin.org/basic-auth/username/password")
print(f"Basic auth: {resp.json()}")

# Bearer token authentication
client = primp.Client(auth_bearer="token123")
resp = client.get("https://httpbin.org/bearer")
print(f"Bearer auth: {resp.json()}")

# Per-request authentication
client = primp.Client()
resp = client.get(
    "https://httpbin.org/basic-auth/user/pass",
    auth=("user", "pass")
)
print(f"Per-request auth: {resp.json()}")

# Bearer token per request
resp = client.get(
    "https://httpbin.org/bearer",
    auth_bearer="my-token"
)
print(f"Per-request bearer: {resp.json()}")
