## Benchmark

Benchmark between `primp` and other python http clients:

- curl_cffi
- httpx
- primp
- pycurl
- python-tls-client
- requests

Server response is gzipped.

#### Run benchmark:
    
- run server: `uvicorn server:app`
- run benchmark: `python benchmark.py`
