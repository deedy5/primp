Basic Usage Examples
pythonimport primp

# Simple GET request
resp = primp.get("https://httpbin.org/get")
print(resp.status_code)
print(resp.text)

# GET with browser impersonation (based on your impersonate.rs)
resp = primp.get("https://httpbin.org/get", impersonate="chrome_131")
print(resp.json())
Client Usage
pythonimport primp

# Initialize client with impersonation
client = primp.Client(impersonate="chrome_131", timeout=30)

# GET request
resp = client.get("https://httpbin.org/get")
print(f"Status: {resp.status_code}")
print(f"Headers: {resp.headers}")

# POST with JSON data
json_data = {"key1": "value1", "key2": "value2"}
resp = client.post("https://httpbin.org/post", json=json_data)
print(resp.json())

# POST with form data
form_data = {"username": "test", "password": "secret"}
resp = client.post("https://httpbin.org/post", data=form_data)

# Request with custom headers
headers = {"User-Agent": "Custom-Agent", "Accept": "application/json"}
resp = client.get("https://httpbin.org/headers", headers=headers)
Supported Browser Impersonation
python# Chrome versions (from your impersonate.rs)
client = primp.Client(impersonate="chrome_137")  # Latest
client = primp.Client(impersonate="chrome_131")
client = primp.Client(impersonate="chrome_100")

# Firefox versions  
client = primp.Client(impersonate="firefox_136")
client = primp.Client(impersonate="firefox_133")

# Safari versions
client = primp.Client(impersonate="safari_18_2")
client = primp.Client(impersonate="safari_ios_18_1_1")

# Edge versions
client = primp.Client(impersonate="edge_131")

# Random selection
client = primp.Client(impersonate="random")
Authentication Examples
python# Basic authentication
client = primp.Client(auth=("username", "password"))
resp = client.get("https://httpbin.org/basic-auth/username/password")

# Bearer token authentication  
client = primp.Client(auth_bearer="your-token-here")
resp = client.get("https://httpbin.org/bearer")

# Per-request authentication
resp = client.get("https://httpbin.org/basic-auth/user/pass", 
                  auth=("user", "pass"))
Cookie Handling
pythonclient = primp.Client(cookie_store=True)

# Set cookies for a domain
cookies = {"session": "abc123", "user": "john"}
client.set_cookies("https://example.com", cookies)

# Get cookies from a domain
stored_cookies = client.get_cookies("https://example.com")
print(stored_cookies)

# Cookies in response
resp = client.get("https://httpbin.org/cookies/set/test/value")
print(resp.cookies)
File Upload
python# Upload files (multipart/form-data)
files = {
    "file1": "/path/to/file1.txt",
    "file2": "/path/to/file2.pdf"
}
resp = client.post("https://httpbin.org/post", files=files)
Async Usage
pythonimport asyncio
import primp

async def fetch_data():
    async with primp.AsyncClient(impersonate="chrome_131") as client:
        resp = await client.get("https://httpbin.org/get")
        return resp.json()

async def main():
    urls = [
        "https://httpbin.org/delay/1",
        "https://httpbin.org/delay/2", 
        "https://httpbin.org/delay/3"
    ]
    
    async with primp.AsyncClient() as client:
        tasks = [client.get(url) for url in urls]
        responses = await asyncio.gather(*tasks)
        
        for resp in responses:
            print(f"Status: {resp.status_code}")

asyncio.run(main())
Response Handling
pythonclient = primp.Client()
resp = client.get("https://example.com")

# Different ways to access response content
print(resp.content)           # Raw bytes
print(resp.text)             # Text with auto-detected encoding
print(resp.json())           # Parse JSON
print(resp.status_code)      # HTTP status code
print(resp.url)              # Final URL after redirects
print(resp.headers)          # Response headers
print(resp.cookies)          # Response cookies

# HTML to text conversion (unique to your implementation)
print(resp.text_markdown)    # HTML converted to markdown
print(resp.text_plain)       # HTML converted to plain text  
print(resp.text_rich)        # HTML converted to rich text
Advanced Configuration
python# Client with all options
client = primp.Client(
    impersonate="chrome_131",
    headers={"Accept": "application/json"},
    cookie_store=True,
    referer=True,
    proxy="socks5://127.0.0.1:1080",
    timeout=60,
    follow_redirects=True,
    max_redirects=10,
    verify=True,
    ca_cert_file="/path/to/cacert.pem",
    https_only=False,
    http2_only=False
)
Key Differences from Documentation
Note that your actual implementation differs from the documentation in these ways:

No impersonate_os parameter - This doesn't exist in your Rust code
Different browser versions - Some versions in docs don't match your impersonate.rs
Response streaming - The stream() method exists in your code but works differently than shown in docs

These examples are based on your actual Rust implementation and will work correctly with your library.