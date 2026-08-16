"""Error handling examples for primp."""

import json

import primp

client = primp.Client(impersonate="chrome_146", timeout=10)

# HTTP status errors (4xx/5xx)
try:
    resp = client.get("https://httpbin.org/status/404")
except primp.StatusError as e:
    print(f"HTTP status error: {e}")

# raise_for_status
resp = client.get("https://httpbin.org/status/200")
try:
    resp.raise_for_status()
except primp.StatusError as e:
    print(f"Status error: {e}")

# Timeout
try:
    client.get("https://httpbin.org/delay/15", timeout=2)
except primp.TimeoutError as e:
    print(f"Timeout: {e}")

# Read timeout (max gap between bytes)
try:
    client.get("https://httpbin.org/delay/10", read_timeout=2)
except primp.TimeoutError as e:
    print(f"Read timeout: {e}")

# Connection errors (DNS, proxy, SSL)
try:
    client.get("https://nonexistent-domain-12345.com")
except primp.ConnectError as e:
    print(f"Connection error: {e}")

# JSON decode errors (catchable as both PrimpError and json.JSONDecodeError)
try:
    resp = client.get("https://httpbin.org/html")
    data = resp.json()
except primp.PrimpError as e:
    print(f"JSON decode error: {e}")

# You can also catch as stdlib json.JSONDecodeError (preserves .doc/.pos)
try:
    resp = client.get("https://httpbin.org/html")
    data = resp.json()
except json.JSONDecodeError as e:
    print(f"JSON decode error at line {e.lineno}: {e}")

# Catch-all
try:
    client.get("https://httpbin.org/status/500")
except primp.PrimpError as e:
    print(f"Request failed: {e}")
