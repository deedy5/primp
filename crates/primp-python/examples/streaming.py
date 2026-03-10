"""Streaming examples for primp."""

import primp

# Create client with browser impersonation
client = primp.Client(impersonate="chrome_145")

# ============================================================================
# Sync Streaming Examples
# ============================================================================

# Method 1: Using context manager (recommended)
print("=== Context Manager Streaming ===")
with client.get("https://httpbin.org/stream/5", stream=True) as resp:
    for chunk in resp.iter_bytes():
        print(f"Chunk: {len(chunk)} bytes")

# Method 2: Manual resource management
print("\n=== Manual Streaming ===")
resp = client.get("https://httpbin.org/bytes/1024", stream=True)
try:
    for chunk in resp.iter_bytes(chunk_size=256):
        print(f"Chunk: {len(chunk)} bytes")
finally:
    resp.close()

# Iterating lines
print("\n=== Line Streaming ===")
with client.get("https://httpbin.org/stream/3", stream=True) as resp:
    for line in resp.iter_lines():
        print(f"Line: {line}")

# Large file download with progress
print("\n=== Large File Download ===")
with client.get("https://httpbin.org/bytes/65536", stream=True) as resp:
    total = 0
    for chunk in resp.iter_bytes(chunk_size=8192):
        total += len(chunk)
        print(f"Downloaded: {total} bytes")

# Text streaming
print("\n=== Text Streaming ===")
with client.get("https://httpbin.org/stream/3", stream=True) as resp:
    for text_chunk in resp.iter_text():
        print(f"Text chunk: {text_chunk[:50]}...")

# ============================================================================
# Async Streaming Examples
# ============================================================================

import asyncio


async def async_streaming():
    """Async streaming examples."""
    client = primp.AsyncClient(impersonate="chrome_145")

    # Async context manager streaming
    print("\n=== Async Context Manager Streaming ===")
    async with await client.get("https://httpbin.org/stream/5", stream=True) as resp:
        async for chunk in resp.aiter_bytes():
            print(f"Async chunk: {len(chunk)} bytes")

    # Async line streaming
    print("\n=== Async Line Streaming ===")
    async with await client.get("https://httpbin.org/stream/3", stream=True) as resp:
        async for line in resp.aiter_lines():
            print(f"Async line: {line}")

    # Manual async streaming
    print("\n=== Manual Async Streaming ===")
    resp = await client.get("https://httpbin.org/bytes/1024", stream=True)
    try:
        async for chunk in resp.aiter_bytes(chunk_size=256):
            print(f"Async chunk: {len(chunk)} bytes")
    finally:
        await resp.aclose()


if __name__ == "__main__":
    asyncio.run(async_streaming())
