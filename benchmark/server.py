import base64
import os
import gzip
from starlette.applications import Starlette
from starlette.responses import Response
from starlette.routing import Route

random_5k = base64.b64encode(os.urandom(5 * 1024)).decode('utf-8')
random_5k = gzip.compress(random_5k.encode('utf-8'))

random_50k = base64.b64encode(os.urandom(50 * 1024)).decode('utf-8')
random_50k = gzip.compress(random_50k.encode('utf-8'))


def gzip_response(gzipped_content):
    headers = {
        'Content-Encoding': 'gzip',
        'Content-Length': str(len(gzipped_content)),
    }
    return Response(gzipped_content, headers=headers)

app = Starlette(
    routes=[
        Route("/5k", lambda r: gzip_response(random_5k)),
        Route("/50k", lambda r: gzip_response(random_50k)),
    ],
)

# Run server: uvicorn server:app
