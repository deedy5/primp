"""Async usage examples for primp AsyncClient."""

import asyncio

import primp


async def basic_request():
    """Basic async GET request."""
    async with primp.AsyncClient(impersonate="chrome_146") as client:
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

    async with primp.AsyncClient(impersonate="chrome_146") as client:
        tasks = [client.get(url) for url in urls]
        responses = await asyncio.gather(*tasks)

        for resp in responses:
            print(f"{resp.url}: {resp.status_code}")


async def main():
    print("=== Basic Request ===")
    await basic_request()

    print("\n=== Concurrent Requests ===")
    await concurrent_requests()


if __name__ == "__main__":
    asyncio.run(main())
