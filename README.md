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

# Simple GET request - note: module-level functions not implemented, use Client
client = primp.Client()
response = client.request('GET', 'https://httpbin.org/get')
print(response.text)

# POST request with JSON data
response = client.request(
    'POST',
    'https://httpbin.org/post',
    json={'key': 'value'}
)
print(response.json())

# Using a client for multiple requests
client = primp.Client(impersonate='chrome_137')
response = client.request('GET', 'https://httpbin.org/get')
print(response.status_code)
```

### Asynchronous Usage
```python
import asyncio
import primp

async def main():
    # Simple async GET request
    client = primp.AsyncClient()
    response = await client.get('https://httpbin.org/get')
    print(response.text)
        
    # POST request with impersonation
    client = primp.AsyncClient(impersonate='chrome_137')
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
chrome_100, chrome_101, chrome_104, chrome_105, chrome_106, chrome_107, chrome_108, chrome_109, chrome_110, chrome_114, chrome_116, chrome_117, chrome_118, chrome_119, chrome_120, chrome_123, chrome_124, chrome_126, chrome_127, chrome_128, chrome_129, chrome_130, chrome_131, chrome_132, chrome_133, chrome_134, chrome_135, chrome_136, chrome_137

**Safari:**  
safari_15.3, safari_15.5, safari_15.6.1, safari_16, safari_16.5, safari_17.0, safari_17.2.1, safari_17.4.1, safari_17.5, safari_18, safari_18.2, safari_18.3, safari_18.3.1, safari_18.5  

**Safari iOS:**  
safari_ios_16.5, safari_ios_17.2, safari_ios_17.4.1, safari_ios_18.1.1  

**Safari iPad:**  
safari_ipad_18  

**Firefox:**  
firefox_109, firefox_117, firefox_128, firefox_133, firefox_135, firefox_private_135, firefox_android_135, firefox_136, firefox_private_136, firefox_139  

**Edge:**  
edge_101, edge_122, edge_127, edge_131, edge_134  

**Opera:**  
opera_116, opera_117, opera_118, opera_119  

**OkHttp:**  
okhttp_3.9, okhttp_3.11, okhttp_3.13, okhttp_3.14, okhttp_4.9, okhttp_4.10, okhttp_4.12, okhttp_5  

**Random:**  
random - Randomly selects from available browsers  

### Operating System Impersonation

- android - Android devices  
- ios - iOS devices  
- linux - Linux systems (default)  
- macos - macOS systems  
- windows - Windows systems  
- random - Randomly selects OS  

## Advanced Usage

### Client Configuration
```python
import primp

# Configure client with all options
client = primp.Client(
    impersonate='chrome_137',
    impersonate_os='windows',
    headers={'User-Agent': 'Custom-Agent'},
    proxy='http://proxy.example.com:8080',
    timeout=30.0,
    verify=True,
    follow_redirects=True,
    max_redirects=10,
    skip_http2=False,
    skip_headers=False
)

response = client.request('GET', 'https://example.com')
```

### Cookie Management
```python
import primp

client = primp.Client()

# Set cookies for a domain
client.set_cookies('https://example.com', {
    'session_id': 'abc123',
    'user_pref': 'dark_mode'
})

# Get cookies for a domain
cookies = client.get_cookies('https://example.com')
print(cookies)

# Cookies are automatically handled in requests
response = client.request('GET', 'https://example.com/profile')
```

### Headers Management
```python
import primp

client = primp.Client()

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
response = client.request(
    'GET',
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
client = primp.Client()
client.proxy = 'http://proxy1.example.com:8080'
response1 = client.request('GET', 'https://example.com')

client.proxy = 'http://proxy2.example.com:8080'
response2 = client.request('GET', 'https://example.com')
```

### File Uploads
```python
import primp

# Upload files
client = primp.Client()
response = client.request(
    'POST',
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
client = primp.Client(auth=('username', 'password'))
response = client.request('GET', 'https://httpbin.org/basic-auth/username/password')

# Bearer token authentication
client = primp.Client(auth_bearer='your-token-here')
response = client.request('GET', 'https://api.example.com/protected')

# Per-request authentication
client = primp.Client()
response = client.request(
    'GET',
    'https://api.example.com/data',
    auth_bearer='request-specific-token'
)
```

### Async Client Convenience Methods
The AsyncClient provides convenient HTTP method shortcuts:
```python
import asyncio
import primp

async def main():
    client = primp.AsyncClient(impersonate='chrome_137')
    
    # GET request
    response = await client.get('https://httpbin.org/get')
    
    # POST request
    response = await client.post('https://httpbin.org/post', json={'key': 'value'})
    
    # PUT request
    response = await client.put('https://httpbin.org/put', json={'key': 'value'})
    
    # PATCH request
    response = await client.patch('https://httpbin.org/patch', json={'key': 'value'})
    
    # DELETE request
    response = await client.delete('https://httpbin.org/delete')
    
    # HEAD request
    response = await client.head('https://httpbin.org/get')
    
    # OPTIONS request
    response = await client.options('https://httpbin.org/get')

asyncio.run(main())
```

## Response Handling

### Response Properties
```python
import primp

client = primp.Client()
response = client.request('GET', 'https://httpbin.org/json')

# Basic properties
print(response.status_code)  # 200
print(response.url)         # Final URL after redirects
print(response.headers)     # Response headers dict
print(response.cookies)     # Response cookies dict

# Content access
print(response.content)     # Raw bytes
print(response.text)        # Decoded text
print(response.json())      # Parsed JSON

# Text formatting options (HTML content converted to text)
print(response.text_plain)     # Plain text (HTML stripped)
print(response.text_markdown)  # Markdown formatted
print(response.text_rich)      # Rich text formatted

# Encoding
print(response.encoding)       # Detected encoding
response.encoding = 'utf-8'    # Set custom encoding
```

### Streaming Responses
```python
import primp

client = primp.Client()
response = client.request('GET', 'https://example.com/large-file')

# Stream response content
for chunk in response.stream():
    process_chunk(chunk)
```

## Performance Tips

- **Reuse Clients:** Create a client once and reuse it for multiple requests  
- **Connection Pooling:** Clients automatically pool connections  
- **Async for I/O:** Use AsyncClient for I/O-bound workloads  
- **Disable Verification:** Set verify=False only if needed (security risk)  

```python
import primp

# Good: Reuse client
client = primp.Client(impersonate='chrome_137')
for url in urls:
    response = client.request('GET', url)
    process_response(response)

# Less efficient: New client per request
for url in urls:
    client = primp.Client(impersonate='chrome_137')
    response = client.request('GET', url)
    process_response(response)
```

## Environment Variables

- **PRIMP_CA_BUNDLE:** Path to custom CA certificate bundle  
- **CA_CERT_FILE:** Alternative name for CA certificate bundle path  
- **PRIMP_PROXY:** Default proxy URL  

## API Reference

### Client Parameters

- impersonate: Browser to impersonate (string, default: None)  
- impersonate_os: Operating system to impersonate (string, default: "linux")  
- headers: Default headers dict (dict, default: None)  
- auth: Basic auth tuple (username, password) (tuple, default: None)  
- auth_bearer: Bearer token string (string, default: None)  
- params: Default query parameters dict (dict, default: None)  
- proxy: Proxy URL (string, default: None)  
- timeout: Request timeout in seconds (float, default: None)  
- verify: Verify SSL certificates (bool, default: True)  
- ca_cert_file: Path to CA certificate file (string, default: None)  
- follow_redirects: Follow redirects (bool, default: True)  
- max_redirects: Maximum redirects (int, default: 20)  
- cookie_store: Enable cookie persistence (bool, default: True)  
- referer: Auto-set referer header (bool, default: True)  
- https_only: Force HTTPS connections (bool, default: False)  
- http2_only: Force HTTP/2 only (bool, default: False)  
- skip_http2: Skip HTTP/2 support in browser emulation (bool, default: False)  
- skip_headers: Skip adding default headers in browser emulation (bool, default: False)  

### Request Parameters

- method: HTTP method (string, required)  
- url: Request URL (string, required)  
- params: URL query parameters dict (dict, default: None)  
- headers: Request headers dict (dict, default: None)  
- cookies: Request cookies dict (dict, default: None)  
- json: JSON data to send (any JSON-serializable object, default: None)  
- data: Form data to send (dict or other form data, default: None)  
- content: Raw bytes to send (bytes, default: None)  
- files: Files to upload dict (dict, default: None)  
- auth: Request-specific auth tuple (tuple, default: None)  
- auth_bearer: Request-specific bearer token (string, default: None)  
- timeout: Request-specific timeout (float, default: None)  

### Client Properties (Read/Write)

- auth: Basic authentication credentials
- auth_bearer: Bearer token
- params: Default query parameters
- proxy: Proxy URL
- timeout: Default timeout
- impersonate: Browser to impersonate (read-only, use setter)
- impersonate_os: OS to impersonate (read-only, use setter)
- headers: Default headers (use getter/setter or headers_update)

### Client Methods

- request(method, url, ...): Make HTTP request
- get_cookies(url): Get cookies for URL
- set_cookies(url, cookies): Set cookies for URL
- headers_update(headers): Update default headers
- set_impersonate(browser): Set browser to impersonate
- set_impersonate_os(os): Set OS to impersonate
- set_skip_http2(skip): Set HTTP/2 skip option
- set_skip_headers(skip): Set headers skip option

### AsyncClient Additional Methods

- get(url, ...): GET request
- post(url, ...): POST request
- put(url, ...): PUT request
- patch(url, ...): PATCH request
- delete(url, ...): DELETE request
- head(url, ...): HEAD request
- options(url, ...): OPTIONS request

## Installation
Install from PyPI (when available):
```bash
pip install primp
```
Or build from source:
```bash
# Requires Rust toolchain
maturin build --release
pip install target/wheels/primp-*.whl
```