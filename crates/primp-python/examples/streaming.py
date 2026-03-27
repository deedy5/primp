"""Streaming examples for primp."""

import asyncio
import primp


client = primp.Client(impersonate="chrome_146")

# Sync: context manager streaming
with client.get("https://httpbin.org/stream/5", stream=True) as resp:
    for chunk in resp.iter_bytes():
        print(f"Chunk: {len(chunk)} bytes")

# Sync: line streaming
with client.get("https://httpbin.org/stream/3", stream=True) as resp:
    for line in resp.iter_lines():
        print(f"Line: {line}")

# Sync: text streaming
with client.get("https://httpbin.org/stream/3", stream=True) as resp:
    for text_chunk in resp.iter_text():
        print(f"Text chunk: {text_chunk[:50]}...")


async def async_streaming():
    async with primp.AsyncClient(impersonate="chrome_146") as client:
        async with await client.get("https://httpbin.org/stream/5", stream=True) as resp:
            async for chunk in resp.aiter_bytes():
                print(f"Async chunk: {len(chunk)} bytes")

        async with await client.get("https://httpbin.org/stream/3", stream=True) as resp:
            async for line in resp.aiter_lines():
                print(f"Async line: {line}")


if __name__ == "__main__":
    asyncio.run(async_streaming())
