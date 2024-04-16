## Benchmark

Benchmark between `pyreqwests_impersonate` and other python http clients:

- curl_cffi
- httpx
- pyreqwests_impersonate
- python-tls-client
- requests

All the clients run with session/client enabled.
Server response is gzipped.

#### Run benchmark:
    
- run server: `uvicorn server:app`
- run benchmark: `python benchmark.py`
