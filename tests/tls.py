#!/usr/bin/env python3
"""
Test script for primp with Firefox128 emulation, Windows OS, HTTP/1 only, and disabled cert verification.
Based on the Rust example using EmulationOption::builder().
"""

import asyncio
import primp


async def test_async_client():
    """Test with AsyncClient (recommended)"""
    print("Testing AsyncClient...")
    
    # Create client with Firefox128 emulation, Windows OS, skip HTTP/2, disable cert verification
    client = primp.AsyncClient(
        impersonate="firefox_128",
        impersonate_os="windows", 
        skip_http2=True,
        verify=False,
        timeout=30.0
    )
    
    try:
        # Make the request
        response = await client.get("https://tls.peet.ws/api/all")
        text = response.text  # Remove 'await' - this is a property getter
        print("Response status:", response.status_code)
        print("Response text:")
        print(text)
        return text
    except Exception as e:
        print(f"AsyncClient request failed: {e}")
        return None


def test_sync_client():
    """Test with sync Client"""
    print("\nTesting sync Client...")
    
    # Create client with Firefox128 emulation, Windows OS, skip HTTP/2, disable cert verification
    client = primp.Client(
        impersonate="firefox_128",
        impersonate_os="windows",
        skip_http2=True, 
        verify=False,
        timeout=30.0
    )
    
    try:
        # Make the request
        response = client.request("GET", "https://tls.peet.ws/api/all")
        text = response.text
        print("Response status:", response.status_code)
        print("Response text:")
        print(text)
        return text
    except Exception as e:
        print(f"Sync Client request failed: {e}")
        return None


async def test_with_different_browsers():
    """Test the skip_http2 parameter with different browsers"""
    browsers = [
        "firefox_128", 
        "chrome_131", 
        "safari_18",
        "edge_131"
    ]
    
    print("\nTesting skip_http2=True with different browsers...")
    
    for browser in browsers:
        print(f"\n--- Testing {browser} ---")
        
        client = primp.AsyncClient(
            impersonate=browser,
            impersonate_os="windows",
            skip_http2=True,
            verify=False,
            timeout=10.0
        )
        
        try:
            response = await client.get("https://tls.peet.ws/api/all")
            data = response.json()  # Remove 'await' - this is a synchronous method
            
            # Extract relevant TLS info
            tls_version = data.get("tls", {}).get("version", "unknown")
            http_version = data.get("http_version", "unknown") 
            ja3_hash = data.get("ja3_hash", "unknown")[:16] + "..." if data.get("ja3_hash") else "unknown"
            
            print(f"  Status: {response.status_code}")
            print(f"  TLS Version: {tls_version}")
            print(f"  HTTP Version: {http_version}")
            print(f"  JA3 Hash: {ja3_hash}")
            
        except Exception as e:
            print(f"  Failed: {e}")


async def test_skip_headers():
    """Test the skip_headers parameter"""
    print("\n--- Testing skip_headers parameter ---")
    
    # Test with skip_headers=False (default)
    print("With skip_headers=False (default headers):")
    client1 = primp.AsyncClient(
        impersonate="firefox_128",
        impersonate_os="windows",
        skip_http2=True,
        skip_headers=False,
        verify=False,
        timeout=10.0
    )
    
    try:
        response1 = await client1.get("https://httpbin.org/headers")
        data1 = response1.json()  # Remove 'await' - this is a synchronous method
        headers1 = data1.get("headers", {})
        print(f"  User-Agent: {headers1.get('User-Agent', 'Not set')}")
        print(f"  Accept: {headers1.get('Accept', 'Not set')}")
        print(f"  Total headers: {len(headers1)}")
    except Exception as e:
        print(f"  Failed: {e}")
    
    # Test with skip_headers=True (minimal headers)
    print("\nWith skip_headers=True (minimal headers):")
    client2 = primp.AsyncClient(
        impersonate="firefox_128",
        impersonate_os="windows", 
        skip_http2=True,
        skip_headers=True,
        verify=False,
        timeout=10.0
    )
    
    try:
        response2 = await client2.get("https://httpbin.org/headers")
        data2 = response2.json()  # Remove 'await' - this is a synchronous method
        headers2 = data2.get("headers", {})
        print(f"  User-Agent: {headers2.get('User-Agent', 'Not set')}")
        print(f"  Accept: {headers2.get('Accept', 'Not set')}")
        print(f"  Total headers: {len(headers2)}")
    except Exception as e:
        print(f"  Failed: {e}")


async def main():
    """Main test function"""
    print("Primp Library Test - Firefox128 + Windows + HTTP/1 + No Cert Verification")
    print("=" * 70)
    
    # Test basic functionality (equivalent to the Rust example)
    await test_async_client()
    test_sync_client()
    
    # Test with different browsers to verify skip_http2 works
    await test_with_different_browsers()
    
    # Test skip_headers parameter
    await test_skip_headers()
    
    print("\nTest completed!")


if __name__ == "__main__":
    asyncio.run(main())