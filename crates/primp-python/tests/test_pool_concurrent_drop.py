"""Pool `Drop` race — needs many threads to catch.

Many concurrent temp clients must not `broken pipe`; body timeout is
`TimeoutError`, not `DecodeError` (pool detaches while streams active)."""

import threading
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from http.server import BaseHTTPRequestHandler, HTTPServer
from socketserver import ThreadingMixIn

import pytest
import primp


def test_many_threads_early_drop_does_not_cause_broken_pipe():
    """Many threads with temp clients early-dropped must not `broken pipe`.

    100 concurrent `Client().get().text` (Client dropped after headers)
    reproduces the race — pool must stay alive while body streams.
    H2 path covered in Rust (`pool_concurrent_drop.rs`); here via H1.
    """
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def do_GET(self):
            body = b"hello world" * 100
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            mid = len(body) // 2
            try:
                self.wfile.write(body[:mid])
                self.wfile.flush()
                time.sleep(0.02)
                self.wfile.write(body[mid:])
            except BrokenPipeError:
                pass

    class Server(ThreadingMixIn, HTTPServer):
        daemon_threads = True
        allow_reuse_address = True

    srv = Server(("127.0.0.1", 0), Handler)
    port = srv.server_address[1]
    th = threading.Thread(target=srv.serve_forever, daemon=True)
    th.start()
    time.sleep(0.1)

    def get_temp(_):
        return primp.Client().get(f"http://127.0.0.1:{port}/", timeout=5).text

    n = 100
    decode_errors = []
    with ThreadPoolExecutor(max_workers=n) as ex:
        futs = [ex.submit(get_temp, i) for i in range(n)]
        for f in as_completed(futs):
            try:
                assert f.result() == "hello world" * 100
            except primp.DecodeError as e:
                if "broken pipe" in str(e).lower():
                    decode_errors.append(e)
                else:
                    raise
            except Exception:
                pass
    srv.shutdown()
    assert not decode_errors, f"broken-pipe DecodeError: {decode_errors[:3]}"


def test_mid_body_timeout_is_timeout_not_decode(test_server):
    """Mid-body stall must be `TimeoutError`, not `DecodeError`."""
    try:
        primp.Client().get(f"{test_server}/delay/2", timeout=0.2)
        pytest.fail("should have timed out")
    except primp.TimeoutError:
        pass
    except primp.DecodeError as e:
        pytest.fail(f"should be TimeoutError, got DecodeError: {e}")
