"""Error handling examples for primp."""

import json

import primp

client = primp.Client(impersonate="chrome_145", timeout=10)

# Handle HTTP status errors (4xx/5xx)
try:
    resp = client.get("https://httpbin.org/status/404")
except primp.StatusError as e:
    print(f"HTTP status error: {e}")

# Use raise_for_status
resp = client.get("https://httpbin.org/status/200")
try:
    resp.raise_for_status()
    print("Request successful")
except primp.StatusError as e:
    print(f"Status error: {e}")

# Handle timeout
try:
    resp = client.get("https://httpbin.org/delay/15", timeout=2)
except primp.TimeoutError as e:
    print(f"Timeout: {e}")

# Handle connection errors (includes proxy, SSL, DNS errors)
try:
    resp = client.get("https://nonexistent-domain-12345.com")
except primp.ConnectError as e:
    print(f"Connection error: {e}")

# Handle JSON decode errors (standard Python exception, not primp)
resp = client.get("https://httpbin.org/html")
try:
    data = resp.json()
except json.JSONDecodeError as e:
    print(f"JSON decode error: {e}")

# Handle proxy errors (these are ConnectError)
try:
    client = primp.Client(proxy="http://invalid-proxy:9999")
    resp = client.get("https://httpbin.org/get")
except primp.ConnectError as e:
    print(f"Proxy/Connection error: {e}")

# Catch-all primp exception
try:
    resp = client.get("https://httpbin.org/status/500")
except primp.PrimpError as e:
    print(f"Request failed: {e}")

# SSL errors (these are ConnectError)
try:
    client = primp.Client(verify=True)
    resp = client.get("https://expired.badssl.com/")
except primp.ConnectError as e:
    print(f"SSL/Connection error: {e}")
