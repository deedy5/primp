"""Async usage examples for primp AsyncClient."""

import asyncio
import primp


async def basic_request():
    """Basic async GET request."""
    async with primp.AsyncClient(impersonate="chrome_145") as client:
        resp = await client.get("https://httpbin.org/get")
        print(f"Status: {resp.status_code}")
        print(f"Body: {await resp.text}")


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
        data = await resp.json
        print(f"Posted JSON: {data['json']}")


async def streaming():
    """Stream response body."""
    async with primp.AsyncClient() as client:
        resp = await client.get("https://httpbin.org/stream/5")
        async for chunk in await resp.stream():
            print(chunk.decode()[:50])


async def main():
    await basic_request()
    print("---")
    await concurrent_requests()
    print("---")
    await post_json()
    print("---")
    await streaming()


if __name__ == "__main__":
    asyncio.run(main())
