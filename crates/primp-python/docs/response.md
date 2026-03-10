# 📖 Response Object

## Overview

primp provides two types of response objects:

- **`Response`** - Standard response for regular requests (eager loading)
- **`StreamResponse`** - Streaming response for efficient handling of large data (lazy loading)

## Response Object

The standard `Response` object is returned by default from all request methods. All properties are synchronous and work identically for both sync and async clients.

### Properties

| Property | Type | Description |
|:---------|:-----|:------------|
| `.url` | `str` | Final response URL (after redirects) |
| `.status_code` | `int` | HTTP status code |
| `.content` | `bytes` | Raw response body as bytes |
| `.encoding` | `str` | Character encoding (read/write) |
| `.text` | `str` | Decoded text content |
| `.headers` | `dict` | Response headers dictionary |
| `.cookies` | `dict` | Response cookies |
| `.text_markdown` | `str` | HTML converted to Markdown |
| `.text_plain` | `str` | HTML converted to plain text |
| `.text_rich` | `str` | HTML converted to rich text |

### Methods

| Method | Description |
|:-------|:------------|
| `.json()` | Parse response body as JSON (sync for both). Raises `json.JSONDecodeError` on invalid JSON. |
| `.raise_for_status()` | Raise `StatusError` for 4xx/5xx status codes (sync for both) |

### Example

```python
import primp

# Sync usage
response = primp.get("https://httpbin.org/json")
print(response.status_code)  # 200
print(response.json())       # Parsed JSON data

# Async usage (all properties work the same)
async def fetch_data():
    client = primp.AsyncClient()
    response = await client.get("https://httpbin.org/json")
    print(response.status_code)  # 200
    print(response.json())       # Parsed JSON data
```

---

## StreamResponse Object

The `StreamResponse` object is returned when using `stream=True` parameter. It provides streaming access to the response body and supports context manager protocol for automatic resource cleanup.

### Getting a StreamResponse

```python
import primp

# Method 1: Using stream=True parameter
response = primp.get("https://example.com/large-file.zip", stream=True)

# Method 2: Using context manager (recommended)
with primp.get("https://example.com/large-file.zip", stream=True) as response:
    for chunk in response.iter_bytes():
        process_chunk(chunk)
```

### Properties

| Property | Type | Description |
|:---------|:-----|:------------|
| `.url` | `str` | Final response URL (after redirects) |
| `.status_code` | `int` | HTTP status code |
| `.headers` | `dict` | Response headers dictionary |
| `.cookies` | `dict` | Response cookies |
| `.encoding` | `str` | Character encoding (read/write) |
| `.content` | `bytes` | Raw response body as bytes (reads all remaining content) |
| `.text` | `str` | Decoded text content (reads all remaining content) |

### Methods

| Method | Description |
|:-------|:------------|
| `.read()` | Read remaining content into memory and return as bytes |
| `.iter_bytes(chunk_size=8192)` | Return an iterator over byte chunks |
| `.iter_text(chunk_size=8192)` | Return an iterator over decoded text chunks |
| `.iter_lines()` | Return an iterator over lines |
| `.next()` | Get next chunk explicitly (returns bytes or None) |
| `.close()` | Close response and release resources |
| `.raise_for_status()` | Raise `HTTPError` for 4xx/5xx status codes |

### Context Manager Support

`StreamResponse` supports the context manager protocol for automatic resource cleanup:

```python
import primp

# Sync context manager
with primp.get("https://example.com/large-file.zip", stream=True) as response:
    for chunk in response.iter_bytes():
        process_chunk(chunk)
# Response is automatically closed when exiting the context
```

### Streaming Examples

#### Byte Streaming

```python
import primp

# Stream bytes with default chunk size (8KB)
with primp.get("https://example.com/large-file.zip", stream=True) as response:
    with open("large-file.zip", "wb") as f:
        for chunk in response.iter_bytes():
            f.write(chunk)

# Custom chunk size (64KB)
with primp.get("https://example.com/large-file.zip", stream=True) as response:
    for chunk in response.iter_bytes(65536):
        process_chunk(chunk)
```

#### Text Streaming

```python
import primp

# Stream text with automatic decoding
with primp.get("https://example.com/large-text.txt", stream=True) as response:
    for text_chunk in response.iter_text():
        print(text_chunk)
```

#### Line Streaming

```python
import primp

# Stream line by line
with primp.get("https://example.com/log-file.txt", stream=True) as response:
    for line in response.iter_lines():
        print(f"Line: {line}")
```

---

## AsyncStreamResponse Object

The async version of `StreamResponse` is returned when using `stream=True` with the async client. It provides the same functionality but with async methods.

### Async Streaming Methods

| Sync Method | Async Method | Description |
|:------------|:-------------|:------------|
| `.read()` | `.aread()` | Read remaining content into memory |
| `.iter_bytes()` | `.aiter_bytes()` | Return an async iterator over byte chunks |
| `.iter_text()` | `.aiter_text()` | Return an async iterator over text chunks |
| `.iter_lines()` | `.aiter_lines()` | Return an async iterator over lines |
| `.next()` | `.anext()` | Get next chunk explicitly |
| `.close()` | `.aclose()` | Close response and release resources |

### Async Context Manager Support

```python
import primp

async def download_large_file():
    client = primp.AsyncClient()
    
    # Async context manager
    async with await client.get("https://example.com/large-file.zip", stream=True) as response:
        with open("large-file.zip", "wb") as f:
            async for chunk in response.aiter_bytes():
                f.write(chunk)
```

### Async Streaming Examples

#### Byte Streaming

```python
import primp

async def stream_bytes():
    client = primp.AsyncClient()
    response = await client.get("https://example.com/large-file.zip", stream=True)
    
    try:
        async for chunk in response.aiter_bytes():
            process_chunk(chunk)
    finally:
        await response.aclose()
```

#### Text Streaming

```python
import primp

async def stream_text():
    client = primp.AsyncClient()
    
    async with await client.get("https://example.com/data.txt", stream=True) as response:
        async for text_chunk in response.aiter_text():
            print(text_chunk)
```

#### Line Streaming

```python
import primp

async def stream_lines():
    client = primp.AsyncClient()
    
    async with await client.get("https://example.com/log.txt", stream=True) as response:
        async for line in response.aiter_lines():
            print(f"Line: {line}")
```

---

## Comparison: Response vs StreamResponse

| Feature | Response | StreamResponse |
|:--------|:---------|:---------------|
| Body loading | Eager (loads entire body) | Lazy (streams on demand) |
| Memory usage | Higher (entire body in memory) | Lower (chunk by chunk) |
| Use case | Small responses, JSON APIs | Large files, streaming data |
| Iteration | Not supported | Supported via iterators |
| Context manager | Not needed | Recommended for cleanup |

## Best Practices

1. **Use `Response` for small data**: When working with APIs that return JSON or small responses, use the default `Response` object.

2. **Use `StreamResponse` for large data**: When downloading files or processing large responses, use `stream=True` to avoid loading everything into memory.

3. **Always use context managers with streams**: This ensures proper resource cleanup even if an error occurs.

4. **Choose appropriate chunk sizes**: Larger chunks reduce overhead but use more memory. The default 8KB is a good balance for most use cases.

```python
# Good: Context manager for streaming
with primp.get(url, stream=True) as response:
    for chunk in response.iter_bytes(65536):  # 64KB chunks
        process(chunk)

# Avoid: Forgetting to close stream
response = primp.get(url, stream=True)
for chunk in response.iter_bytes():
    process(chunk)
# response.close()  # Don't forget!
