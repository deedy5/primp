"""HTML conversion examples for primp."""

import primp

client = primp.Client(impersonate="chrome_146")
resp = client.get("https://httpbin.org/html")

# Convert HTML to Markdown
print(f"Markdown:\n{resp.text_markdown[:200]}...")

# Convert HTML to plain text
print(f"Plain text:\n{resp.text_plain[:200]}...")

# Convert HTML to rich text
print(f"Rich text:\n{resp.text_rich[:200]}...")

# Override encoding
resp.encoding = "iso-8859-1"
print(f"Custom encoding: {resp.text[:100]}...")
