"""POST request examples for primp."""

import primp

client = primp.Client(impersonate="chrome_146")

# POST form data
resp = client.post(
    "https://httpbin.org/post",
    data={"username": "john", "password": "secret"}
)
print(f"Form data: {resp.json()['form']}")

# POST JSON
resp = client.post(
    "https://httpbin.org/post",
    json={"name": "John", "age": 30}
)
print(f"JSON data: {resp.json()['json']}")

# POST raw bytes
resp = client.post("https://httpbin.org/post", content=b"Raw binary data")
print(f"Content: {resp.json()['data']}")

# PUT request
resp = client.put("https://httpbin.org/put", json={"id": 1, "name": "updated"})
print(f"PUT: {resp.json()['json']}")

# DELETE request
resp = client.delete("https://httpbin.org/delete")
print(f"DELETE: {resp.status_code}")
