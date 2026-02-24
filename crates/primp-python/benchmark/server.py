import base64
import gzip
import os

# Pre-compress responses at startup
random_5k = base64.b64encode(os.urandom(5 * 1024)).decode("utf-8")
random_5k_gz = gzip.compress(random_5k.encode("utf-8"))

random_50k = base64.b64encode(os.urandom(50 * 1024)).decode("utf-8")
random_50k_gz = gzip.compress(random_50k.encode("utf-8"))

random_200k = base64.b64encode(os.urandom(200 * 1024)).decode("utf-8")
random_200k_gz = gzip.compress(random_200k.encode("utf-8"))


async def app(scope, receive, send):
    """ASGI app for benchmarking."""
    path = scope["path"]
    
    if path == "/5k":
        body = random_5k_gz
    elif path == "/50k":
        body = random_50k_gz
    elif path == "/200k":
        body = random_200k_gz
    else:
        await send({
            "type": "http.response.start",
            "status": 404,
            "headers": [[b"content-type", b"text/plain"]],
        })
        await send({"type": "http.response.body", "body": b"Not Found"})
        return
    
    await send({
        "type": "http.response.start",
        "status": 200,
        "headers": [
            [b"content-encoding", b"gzip"],
            [b"content-length", str(len(body)).encode()],
        ],
    })
    await send({"type": "http.response.body", "body": body})
