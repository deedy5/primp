PRIMP Fork based on wreq-util and wreq
HTTP client that can impersonate web browsers, mimicking their headers and TLS/JA3/JA4/HTTP2 fingerprints.

## Features

- **Browser Impersonation:** Mimic Chrome, Safari, Firefox, Edge, and OkHttp clients  
- **TLS Fingerprinting:** Match JA3/JA4 fingerprints of real browsers  
- **HTTP/2 Support:** Full HTTP/2 implementation with proper fingerprinting  
- **Cookie Management:** Automatic cookie handling and persistence  
- **Proxy Support:** HTTP/HTTPS/SOCKS5 proxy support  
- **Async/Await:** Full async support with thread pool execution  
- **Cross-Platform:** Works on Windows, macOS, and Linux  
- **Fast:** Built in Rust for maximum performance  


## Quick Start

### Synchronous Usage
```python
import primp

# Simple GET request
response = primp.get('https://httpbin.org/get')
print(response.text)

# POST request with JSON data
response = primp.post(
    'https://httpbin.org/post',
    json={'key': 'value'}
)
print(response.json())

# Using a client for multiple requests
with primp.Client(impersonate='chrome_131') as client:
    response = client.get('https://httpbin.org/get')
    print(response.status_code)
```

### Asynchronous Usage
```python
import asyncio
import primp

async def main():
    # Simple async GET request
    async with primp.AsyncClient() as client:
        response = await client.get('https://httpbin.org/get')
        print(response.text)
        
    # POST request with impersonation
    async with primp.AsyncClient(impersonate='chrome_131') as client:
        response = await client.post(
            'https://httpbin.org/post',
            json={'key': 'value'}
        )
        print(response.json())

asyncio.run(main())
```

## Browser Impersonation

### Supported Browsers

**Chrome:**
chrome_100, chrome_101, chrome_104, chrome_105, chrome_106  
chrome_107, chrome_108, chrome_109, chrome_114, chrome_116  
chrome_117, chrome_118, chrome_119, chrome_120, chrome_123  
chrome_124, chrome_126, chrome_127, chrome_128, chrome_129  
chrome_130, chrome_131, chrome_133, chrome_137  

**Safari:**  
safari_15.3, safari_15.5, safari_15.6.1, safari_16, safari_16.5  
safari_17.0, safari_17.2.1, safari_17.4.1, safari_17.5  
safari_18, safari_18.2  

**Safari iOS:**  
safari_ios_16.5, safari_ios_17.2, safari_ios_17.4.1, safari_ios_18.1.1  
safari_ipad_18  

**Firefox:**  
firefox_109, firefox_117, firefox_128, firefox_133, firefox_135, firefox_136  

**Edge:**  
edge_101, edge_122, edge_127, edge_131  

**OkHttp:**  
okhttp_3.13, okhttp_3.14, okhttp_4.9, okhttp_4.10, okhttp_5  

**Random:**  
random - Randomly selects from available browsers  

### Operating System Impersonation

- android - Android devices  
- ios - iOS devices  
- linux - Linux systems  
- macos - macOS systems  
- windows - Windows systems  
- random - Randomly selects OS  

## Advanced Usage

### Client Configuration
```python
import primp

# Configure client with all options
client = primp.Client(
    impersonate='chrome_131',
    impersonate_os='windows',
    headers={'User-Agent': 'Custom-Agent'},
    cookies={'session': 'abc123'},
    proxy='http://proxy.example.com:8080',
    timeout=30.0,
    verify=True,
    follow_redirects=True,
    max_redirects=10
)

with client:
    response = client.get('https://example.com')
```

### Cookie Management
```python
import primp

with primp.Client() as client:
    # Set cookies for a domain
    client.set_cookies('https://example.com', {
        'session_id': 'abc123',
        'user_pref': 'dark_mode'
    })
    
    # Get cookies for a domain
    cookies = client.get_cookies('https://example.com')
    print(cookies)
    
    # Cookies are automatically handled in requests
    response = client.get('https://example.com/profile')
```

### Headers Management
```python
import primp

with primp.Client() as client:
    # Set default headers
    client.headers = {
        'Authorization': 'Bearer token123',
        'Accept': 'application/json'
    }
    
    # Update headers (merges with existing)
    client.headers_update({
        'X-Custom-Header': 'value'
    })
    
    # Request with additional headers
    response = client.get(
        'https://api.example.com/data',
        headers={'X-Request-ID': 'req-456'}
    )
```

### Proxy Configuration
```python
import primp

# HTTP proxy
client = primp.Client(proxy='http://proxy.example.com:8080')

# SOCKS5 proxy with authentication
client = primp.Client(proxy='socks5://user:pass@proxy.example.com:1080')

# Use different proxies per request
with primp.Client() as client:
    client.proxy = 'http://proxy1.example.com:8080'
    response1 = client.get('https://example.com')
    
    client.proxy = 'http://proxy2.example.com:8080'
    response2 = client.get('https://example.com')
```

### File Uploads
```python
import primp

# Upload files
with primp.Client() as client:
    response = client.post(
        'https://httpbin.org/post',
        files={
            'file1': '/path/to/file1.txt',
            'file2': '/path/to/file2.pdf'
        }
    )
    print(response.json())
```

### Authentication
```python
import primp

# Basic authentication
with primp.Client(auth=('username', 'password')) as client:
    response = client.get('https://httpbin.org/basic-auth/username/password')

# Bearer token authentication
with primp.Client(auth_bearer='your-token-here') as client:
    response = client.get('https://api.example.com/protected')

# Per-request authentication
with primp.Client() as client:
    response = client.get(
        'https://api.example.com/data',
        auth_bearer='request-specific-token'
    )
```

## Response Handling

### Response Properties
```python
import primp

response = primp.get('https://httpbin.org/json')

# Basic properties
print(response.status_code)  # 200
print(response.url)         # Final URL after redirects
print(response.headers)     # Response headers dict
print(response.cookies)     # Response cookies dict

# Content access
print(response.content)     # Raw bytes
print(response.text)        # Decoded text
print(response.json())      # Parsed JSON

# Text formatting options
print(response.text_plain)     # Plain text (HTML stripped)
print(response.text_markdown)  # Markdown formatted
print(response.text_rich)      # Rich text formatted
```

### Streaming Responses
```python
import primp

with primp.Client() as client:
    response = client.get('https://example.com/large-file')
    
    # Stream response content
    for chunk in response.stream():
        process_chunk(chunk)
```

### Error Handling
```python
import primp

try:
    response = primp.get('https://httpbin.org/status/404')
    response.raise_for_status()  # Raises exception for 4xx/5xx status
except primp.HTTPError as e:
    print(f"HTTP error: {e}")
except primp.RequestException as e:
    print(f"Request error: {e}")
```

## Performance Tips

- **Reuse Clients:** Create a client once and reuse it for multiple requests  
- **Connection Pooling:** Clients automatically pool connections  
- **Async for I/O:** Use AsyncClient for I/O-bound workloads  
- **Disable Verification:** Set verify=False only if needed (security risk)  

```python
import primp

# Good: Reuse client
with primp.Client(impersonate='chrome_131') as client:
    for url in urls:
        response = client.get(url)
        process_response(response)

# Less efficient: New client per request
for url in urls:
    response = primp.get(url, impersonate='chrome_131')
    process_response(response)
```

## Environment Variables

- **PRIMP_CA_BUNDLE:** Path to custom CA certificate bundle  
- **PRIMP_PROXY:** Default proxy URL  

## API Reference

### Client Parameters

- impersonate: Browser to impersonate  
- impersonate_os: Operating system to impersonate  
- headers: Default headers dict  
- cookies: Default cookies dict  
- auth: Basic auth tuple (username, password)  
- auth_bearer: Bearer token string  
- proxy: Proxy URL  
- timeout: Request timeout in seconds  
- verify: Verify SSL certificates (default: True)  
- ca_cert_file: Path to CA certificate file  
- follow_redirects: Follow redirects (default: True)  
- max_redirects: Maximum redirects (default: 20)  
- cookie_store: Enable cookie persistence (default: True)  
- referer: Auto-set referer header (default: True)  
- https_only: Force HTTPS connections (default: False)  
- http2_only: Force HTTP/2 only (default: False)  

### Request Parameters

- params: URL query parameters dict  
- headers: Request headers dict  
- cookies: Request cookies dict  
- json: JSON data to send  
- data: Form data to send  
- content: Raw bytes to send  
- files: Files to upload dict  
- auth: Request-specific auth tuple  
- auth_bearer: Request-specific bearer token  
- timeout: Request-specific timeout  

##