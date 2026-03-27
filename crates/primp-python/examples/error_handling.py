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

# Connection errors (DNS, proxy, SSL)
try:
    client.get("https://nonexistent-domain-12345.com")
except primp.ConnectError as e:
    print(f"Connection error: {e}")

# JSON decode errors (standard Python exception, not primp)
try:
    resp = client.get("https://httpbin.org/html")
    data = resp.json()
except json.JSONDecodeError as e:
    print(f"JSON decode error: {e}")

# Catch-all
try:
    client.get("https://httpbin.org/status/500")
except primp.PrimpError as e:
    print(f"Request failed: {e}")
