"""HTML conversion examples for primp."""

import primp

client = primp.Client(impersonate="chrome_145")

# Get HTML content
resp = client.get("https://httpbin.org/html")
print(f"HTML length: {len(resp.text)}")

# Convert HTML to Markdown
resp = client.get("https://httpbin.org/html")
markdown = resp.text_markdown
print(f"Markdown:\n{markdown[:200]}...")

# Convert HTML to plain text
resp = client.get("https://httpbin.org/html")
plain = resp.text_plain
print(f"Plain text:\n{plain[:200]}...")

# Convert HTML to rich text
resp = client.get("https://httpbin.org/html")
rich = resp.text_rich
print(f"Rich text:\n{rich[:200]}...")

# Get encoding
resp = client.get("https://httpbin.org/html")
print(f"Encoding: {resp.encoding}")

# Override encoding
resp = client.get("https://httpbin.org/html")
resp.encoding = "iso-8859-1"
text = resp.text
print(f"Text with custom encoding: {text[:100]}...")
