"""Streaming examples for primp."""

import primp

# Stream response body
client = primp.Client(impersonate="chrome_145")

# Using stream() iterator
resp = client.get("https://httpbin.org/stream/5")
for chunk in resp.stream():
    print(f"Chunk: {len(chunk)} bytes")

# Iterating content with chunk size
resp = client.get("https://httpbin.org/bytes/1024")
while True:
    chunk = resp.iter_content(chunk_size=256)
    if chunk is None:
        break
    print(f"Chunk: {len(chunk)} bytes")

# Iterating lines
resp = client.get("https://httpbin.org/stream/3")
while True:
    lines = resp.iter_lines()
    if lines is None:
        break
    for line in lines:
        print(f"Line: {line}")

# Large file download
resp = client.get("https://httpbin.org/bytes/65536")
total = 0
for chunk in resp.stream():
    total += len(chunk)
    print(f"Downloaded: {total} bytes")
