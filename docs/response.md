# Response

HTTP response object returned by Client and AsyncClient.

## Properties

### Sync Client

```python
resp = client.get(url)

# Body
resp.content          # bytes: Raw response body
resp.text             # str: Decoded text
resp.json()           # Any: Parsed JSON

# HTML conversion
resp.text_markdown    # str: HTML to Markdown
resp.text_plain       # str: HTML to plain text
resp.text_rich        # str: HTML to rich text

# Metadata
resp.status_code      # int: HTTP status code
resp.url              # str: Final URL
resp.headers          # dict: Response headers
resp.cookies          # dict: Response cookies
resp.encoding         # str: Character encoding
```

### Async Client

```python
resp = await client.get(url)

# Body (awaitable)
await resp.content    # bytes
await resp.text       # str
await resp.json       # Any

# HTML conversion (awaitable)
await resp.text_markdown
await resp.text_plain
await resp.text_rich

# Metadata
resp.status_code      # int (sync)
resp.url              # str (sync)
await resp.headers    # dict
await resp.cookies    # dict
await resp.encoding   # str
```

## Methods

### Streaming

```python
# Sync streaming
for chunk in resp.stream():
    print(chunk)

# Chunked iteration
chunk = resp.iter_content(chunk_size=1024)

# Line iteration
lines = resp.iter_lines()
```

### Status Check

```python
resp.raise_for_status()  # Raises exception if status >= 400
```

### Read

```python
content = resp.read()  # Alias for content property
```

## Encoding

Default encoding is UTF-8. Override with:

```python
resp.encoding = "iso-8859-1"
text = resp.text
```

## Status Code Reference

| Code | Meaning |
|------|---------|
| 200 | OK |
| 201 | Created |
| 204 | No Content |
| 301 | Moved Permanently |
| 302 | Found |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found |
| 500 | Internal Server Error |
| 502 | Bad Gateway |
| 503 | Service Unavailable |
