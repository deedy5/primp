"""Error handling examples for primp."""

import primp

client = primp.Client(impersonate="chrome_145", timeout=10)

# Handle HTTP errors
try:
    resp = client.get("https://httpbin.org/status/404")
except primp.HTTPError as e:
    print(f"HTTP error: {e}")

# Use raise_for_status
resp = client.get("https://httpbin.org/status/200")
try:
    resp.raise_for_status()
    print("Request successful")
except primp.RequestException as e:
    print(f"Error: {e}")

# Handle timeout
try:
    resp = client.get("https://httpbin.org/delay/15", timeout=2)
except primp.Timeout as e:
    print(f"Timeout: {e}")
except primp.ConnectTimeout as e:
    print(f"Connect timeout: {e}")
except primp.ReadTimeout as e:
    print(f"Read timeout: {e}")

# Handle connection errors
try:
    resp = client.get("https://nonexistent-domain-12345.com")
except primp.ConnectionError as e:
    print(f"Connection error: {e}")

# Handle JSON decode errors
resp = client.get("https://httpbin.org/html")
try:
    data = resp.json()
except primp.JSONDecodeError as e:
    print(f"JSON decode error: {e}")

# Handle proxy errors
try:
    client = primp.Client(proxy="http://invalid-proxy:9999")
    resp = client.get("https://httpbin.org/get")
except primp.ProxyError as e:
    print(f"Proxy error: {e}")

# Catch-all request exception
try:
    resp = client.get("https://httpbin.org/status/500")
except primp.RequestException as e:
    print(f"Request failed: {e}")

# SSL errors
try:
    client = primp.Client(verify=True)
    resp = client.get("https://expired.badssl.com/")
except primp.SSLError as e:
    print(f"SSL error: {e}")
