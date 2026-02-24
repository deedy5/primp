"""Async usage examples for primp AsyncClient."""

import asyncio
import primp


async def basic_request():
    """Basic async GET request."""
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        resp = await client.get("https://httpbin.org/get")
        print(f"Status: {resp.status_code}")
        print(f"Body: {resp.text[:100]}...")


async def concurrent_requests():
    """Multiple concurrent requests."""
    urls = [
        "https://httpbin.org/get",
        "https://httpbin.org/ip",
        "https://httpbin.org/headers",
    ]

    async with primp.AsyncClient(impersonate="chrome_145") as client:
        tasks = [client.get(url) for url in urls]
        responses = await asyncio.gather(*tasks)
        
        for resp in responses:
            print(f"{resp.url}: {resp.status_code}")


async def post_json():
    """POST JSON data."""
    async with primp.AsyncClient() as client:
        resp = await client.post(
            "https://httpbin.org/post",
            json={"key": "value", "number": 42}
        )
        data = resp.json()
        print(f"Posted JSON: {data['json']}")


async def streaming():
    """Stream response body using stream=True."""
    async with primp.AsyncClient() as client:
        async with await client.get("https://httpbin.org/stream/5", stream=True) as resp:
            async for chunk in resp.aiter_bytes():
                print(f"Chunk: {len(chunk)} bytes")


async def streaming_lines():
    """Stream response line by line."""
    async with primp.AsyncClient() as client:
        async with await client.get("https://httpbin.org/stream/3", stream=True) as resp:
            async for line in resp.aiter_lines():
                print(f"Line: {line}")


async def manual_streaming():
    """Manual streaming with explicit resource cleanup."""
    client = primp.AsyncClient()
    resp = await client.get("https://httpbin.org/bytes/1024", stream=True)
    try:
        async for chunk in resp.aiter_bytes(chunk_size=256):
            print(f"Chunk: {len(chunk)} bytes")
    finally:
        await resp.aclose()


async def main():
    print("=== Basic Request ===")
    await basic_request()
    
    print("\n=== Concurrent Requests ===")
    await concurrent_requests()
    
    print("\n=== POST JSON ===")
    await post_json()
    
    print("\n=== Streaming ===")
    await streaming()
    
    print("\n=== Streaming Lines ===")
    await streaming_lines()
    
    print("\n=== Manual Streaming ===")
    await manual_streaming()


if __name__ == "__main__":
    asyncio.run(main())
