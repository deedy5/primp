"""Cookie management examples for primp."""

import primp

# Persistent cookie store (default)
client = primp.Client(cookie_store=True, impersonate="chrome_145")

# Set cookies for a specific domain
client.set_cookies(
    url="https://httpbin.org",
    cookies={"session_id": "abc123", "user": "john"}
)

# Get cookies for a domain
cookies = client.get_cookies(url="https://httpbin.org")
print(f"Stored cookies: {cookies}")

# Cookies are automatically sent with requests
resp = client.get("https://httpbin.org/cookies")
print(f"Request cookies: {resp.json()['cookies']}")

# Set cookies in request
resp = client.get(
    "https://httpbin.org/cookies",
    cookies={"temp_cookie": "temp_value"}
)
print(f"Request cookies: {resp.json()['cookies']}")

# Get cookies from response
resp = client.get("https://httpbin.org/cookies/set?test_cookie=test_value")
print(f"Response cookies: {resp.cookies}")

# Disable cookie store
client = primp.Client(cookie_store=False)
resp = client.get("https://httpbin.org/cookies/set?name=value")
# Cookies won't persist between requests
