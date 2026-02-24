"""POST request examples for primp."""

import primp

client = primp.Client(impersonate="chrome_145")

# POST form data
resp = client.post(
    "https://httpbin.org/post",
    data={"username": "john", "password": "secret"}
)
print(f"Form data: {resp.json()['form']}")

# POST JSON
resp = client.post(
    "https://httpbin.org/post",
    json={"name": "John", "age": 30, "active": True}
)
print(f"JSON data: {resp.json()['json']}")

# POST raw bytes
content = b"Raw binary data"
resp = client.post("https://httpbin.org/post", content=content)
print(f"Content: {resp.json()['data']}")

# POST with files (multipart/form-data)
# Note: files dict maps field name to file path
files = {"file": "/path/to/file.txt"}
resp = client.post("https://httpbin.org/post", files=files)
print(f"Files: {resp.json()['files']}")

# PUT request
resp = client.put(
    "https://httpbin.org/put",
    json={"id": 1, "name": "updated"}
)
print(f"PUT: {resp.json()['json']}")

# PATCH request
resp = client.patch(
    "https://httpbin.org/patch",
    json={"name": "patched"}
)
print(f"PATCH: {resp.json()['json']}")

# DELETE request
resp = client.delete("https://httpbin.org/delete")
print(f"DELETE: {resp.status_code}")
