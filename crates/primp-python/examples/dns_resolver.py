"""DNS resolver examples."""

import primp


def system_resolver():
    """Default: system resolver."""
    client = primp.Client()
    resp = client.get("https://httpbin.org/get")
    print(resp.status_code)


def doh_resolver():
    """DNS-over-HTTPS via Cloudflare."""
    client = primp.Client(dns_resolver="doh://cloudflare-dns.com/dns-query")
    resp = client.get("https://httpbin.org/get")
    print(resp.status_code)


def dot_resolver():
    """DNS-over-TLS."""
    client = primp.Client(dns_resolver="dot://1.1.1.1")
    resp = client.get("https://httpbin.org/get")
    print(resp.status_code)


def plain_dns():
    """Plain DNS on port 53 (shorthand and explicit)."""
    client = primp.Client(dns_resolver="1.1.1.1")
    resp = client.get("https://httpbin.org/get")
    print(resp.status_code)


def fallback_chain():
    """Try DoH first, fall back through system to plain DNS."""
    client = primp.Client(
        dns_resolver=[
            "doh://cloudflare-dns.com/dns-query",
            "system",
            "1.1.1.1",
        ]
    )
    resp = client.get("https://httpbin.org/get")
    print(resp.status_code)


def async_example():
    """Async with DNS resolver."""
    import asyncio

    async def main():
        client = primp.AsyncClient(dns_resolver="dot://dns.google")
        resp = await client.get("https://httpbin.org/get")
        print(resp.status_code)

    asyncio.run(main())


if __name__ == "__main__":
    system_resolver()
    doh_resolver()
    dot_resolver()
    plain_dns()
    fallback_chain()
    async_example()
