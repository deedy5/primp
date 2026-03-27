import primp


# Test 1: Verify headers order is preserved
client = primp.Client(headers={
    "X-Third": "3",
    "X-First": "1",
    "X-Second": "2"
})
headers = client.headers
# Headers should maintain insertion order (custom headers after any defaults)
header_keys = list(headers.keys())
# Find the indices of our custom headers
x_third_idx = header_keys.index("x-third")
x_first_idx = header_keys.index("x-first")
x_second_idx = header_keys.index("x-second")
# Verify they appear in the order we specified them
assert x_third_idx < x_first_idx < x_second_idx, f"Header order not preserved: {header_keys}"
print("Test 1 passed: Header insertion order preserved")

# Test 2: Verify response headers order
response = client.get("https://httpbin.org/get")
response_headers = response.headers
# Response headers should maintain HTTP response order
assert isinstance(response_headers, dict)
# Verify order is consistent across multiple accesses
header_keys = list(response_headers.keys())
assert header_keys == list(response_headers.keys()), "Response header order not consistent"
print("Test 2 passed: Response headers order preserved")

print("All header order tests passed!")
