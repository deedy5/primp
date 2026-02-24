"""
Local testing server that mimics httpbin.org functionality for testing the primp library.

Usage:
    # Run standalone server
    .venv/bin/python tests/server.py
    
    # Use as pytest fixture
    # The server will automatically start/stop for each test
"""

import json
import re
import threading
import time
from contextlib import contextmanager
from http.server import BaseHTTPRequestHandler, HTTPServer
from socketserver import ThreadingMixIn
from typing import Any, Generator
from urllib.parse import parse_qs, urlparse

import pytest


class HttpbinRequestHandler(BaseHTTPRequestHandler):
    """HTTP request handler that mimics httpbin.org functionality."""
    
    protocol_version = "HTTP/1.1"
    
    def log_message(self, format: str, *args: Any) -> None:
        """Suppress default logging."""
        pass
    
    def _build_response_dict(self, method: str, body: bytes = b"") -> dict[str, Any]:
        """Build a response dictionary with request details."""
        parsed_url = urlparse(self.path)
        
        # Get query parameters
        args = {}
        if parsed_url.query:
            query_params = parse_qs(parsed_url.query)
            # Convert lists to single values for consistency with httpbin
            args = {k: v[0] if len(v) == 1 else v for k, v in query_params.items()}
        
        # Get headers
        headers = dict(self.headers)
        
        # Parse body based on content type
        json_data = None
        form_data = None
        files_data = None
        body_str = body.decode("utf-8", errors="replace") if body else ""
        
        content_type = self.headers.get("Content-Type", "")
        
        if "application/json" in content_type and body:
            try:
                json_data = json.loads(body_str)
            except json.JSONDecodeError:
                json_data = None
        elif "multipart/form-data" in content_type and body:
            form_data, files_data = self._parse_multipart(body, content_type)
        elif "application/x-www-form-urlencoded" in content_type and body:
            parsed_form = parse_qs(body_str)
            form_data = {k: v[0] if len(v) == 1 else v for k, v in parsed_form.items()}
        
        response_dict = {
            "args": args,
            "headers": headers,
            "method": method,
            "url": f"http://{self.headers.get('Host', 'localhost')}{self.path}",
            "origin": self.client_address[0],
        }
        
        if body:
            response_dict["data"] = body_str
        
        if json_data is not None:
            response_dict["json"] = json_data
        
        if form_data is not None:
            response_dict["form"] = form_data
        
        if files_data is not None:
            response_dict["files"] = files_data
        
        return response_dict
    
    def _parse_multipart(self, body: bytes, content_type: str) -> tuple[dict, dict]:
        """Parse multipart form data."""
        form_data = {}
        files_data = {}
        
        # Extract boundary
        boundary_match = re.search(r"boundary=([^;\s]+)", content_type)
        if not boundary_match:
            return form_data, files_data
        
        boundary = boundary_match.group(1).strip().encode()
        
        # Split by boundary (with leading --)
        parts = body.split(b"--" + boundary)
        
        for part in parts:
            # Skip empty parts and terminators
            if not part or part.rstrip(b"\r\n") == b"--" or part == b"--":
                continue
            
            # Remove leading CRLF if present
            if part.startswith(b"\r\n"):
                part = part[2:]
            
            # Split headers from content
            if b"\r\n\r\n" in part:
                headers_section, content = part.split(b"\r\n\r\n", 1)
                headers_text = headers_section.decode("utf-8", errors="replace")
                
                # Parse content disposition - handle both quoted and unquoted names
                name_match = re.search(r'name="([^"]+)"', headers_text)
                if not name_match:
                    name_match = re.search(r'name=([^\s;]+)', headers_text)
                
                filename_match = re.search(r'filename="([^"]+)"', headers_text)
                if not filename_match:
                    filename_match = re.search(r'filename=([^\s;]+)', headers_text)
                
                content_type_match = re.search(r"Content-Type:\s*([^\r\n]+)", headers_text)
                
                if name_match:
                    name = name_match.group(1)
                    # Remove trailing \r\n and boundary markers
                    content = content.rstrip(b"\r\n")
                    
                    if filename_match:
                        files_data[name] = {
                            "filename": filename_match.group(1),
                            "content_type": content_type_match.group(1).strip() if content_type_match else "application/octet-stream",
                            "size": len(content),
                        }
                    else:
                        form_data[name] = content.decode("utf-8", errors="replace")
        
        return form_data, files_data
    
    def _send_json_response(self, data: dict, status: int = 200) -> None:
        """Send a JSON response."""
        response_body = json.dumps(data, indent=2).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response_body)))
        self.end_headers()
        self.wfile.write(response_body)
    
    def _send_html_response(self, html: str, status: int = 200) -> None:
        """Send an HTML response."""
        response_body = html.encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(response_body)))
        self.end_headers()
        self.wfile.write(response_body)
    
    def _read_body(self) -> bytes:
        """Read the request body."""
        content_length = self.headers.get("Content-Length")
        if content_length:
            return self.rfile.read(int(content_length))
        return b""
    
    # GET endpoints
    def do_GET(self) -> None:
        """Handle GET requests."""
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self._handle_anything("GET")
        elif path == "/get":
            self._handle_get()
        elif path == "/html":
            self._handle_html()
        elif path == "/headers":
            self._handle_headers()
        elif path == "/ip":
            self._handle_ip()
        elif path == "/user-agent":
            self._handle_user_agent()
        elif path == "/cookies":
            self._handle_cookies()
        elif path.startswith("/cookies/set"):
            self._handle_cookies_set()
        elif path.startswith("/stream/"):
            self._handle_stream(path)
        elif path.startswith("/status/"):
            self._handle_status(path)
        elif path.startswith("/delay/"):
            self._handle_delay(path)
        elif path.startswith("/redirect/"):
            self._handle_redirect(path)
        elif path == "/gzip":
            self._handle_gzip()
        elif path == "/invalid-gzip":
            self._handle_invalid_gzip()
        elif path == "/invalid-deflate":
            self._handle_invalid_deflate()
        elif path == "/broken-chunked":
            self._handle_broken_chunked()
        elif path == "/broken-body":
            self._handle_broken_body()
        elif path == "/xml":
            self._handle_xml()
        elif path == "/upgrade":
            self._handle_upgrade()
        else:
            self._send_json_response({"error": "Not Found"}, 404)
    
    def do_POST(self) -> None:
        """Handle POST requests."""
        body = self._read_body()
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self._handle_anything("POST", body)
        elif path == "/post":
            self._handle_post(body)
        elif path.startswith("/status/"):
            self._handle_status(path)
        else:
            self._send_json_response({"error": "Not Found"}, 404)
    
    def do_PUT(self) -> None:
        """Handle PUT requests."""
        body = self._read_body()
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self._handle_anything("PUT", body)
        elif path == "/put":
            self._handle_put(body)
        elif path.startswith("/status/"):
            self._handle_status(path)
        else:
            self._send_json_response({"error": "Not Found"}, 404)
    
    def do_PATCH(self) -> None:
        """Handle PATCH requests."""
        body = self._read_body()
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self._handle_anything("PATCH", body)
        elif path == "/patch":
            self._handle_patch(body)
        elif path.startswith("/status/"):
            self._handle_status(path)
        else:
            self._send_json_response({"error": "Not Found"}, 404)
    
    def do_DELETE(self) -> None:
        """Handle DELETE requests."""
        body = self._read_body()
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self._handle_anything("DELETE", body)
        elif path == "/delete":
            self._handle_delete(body)
        elif path.startswith("/status/"):
            self._handle_status(path)
        else:
            self._send_json_response({"error": "Not Found"}, 404)
    
    def do_HEAD(self) -> None:
        """Handle HEAD requests."""
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("X-Method", "HEAD")
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
    
    def do_OPTIONS(self) -> None:
        """Handle OPTIONS requests."""
        parsed_url = urlparse(self.path)
        path = parsed_url.path
        
        if path == "/anything":
            self.send_response(200)
            self.send_header("Allow", "GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS")
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            self.send_response(404)
            self.send_header("Content-Length", "0")
            self.end_headers()
    
    # Endpoint handlers
    def _handle_anything(self, method: str, body: bytes = b"") -> None:
        """Handle /anything endpoint."""
        response_dict = self._build_response_dict(method, body)
        self._send_json_response(response_dict)
    
    def _handle_get(self) -> None:
        """Handle /get endpoint."""
        response_dict = self._build_response_dict("GET")
        self._send_json_response(response_dict)
    
    def _handle_post(self, body: bytes) -> None:
        """Handle /post endpoint."""
        response_dict = self._build_response_dict("POST", body)
        self._send_json_response(response_dict)
    
    def _handle_put(self, body: bytes) -> None:
        """Handle /put endpoint."""
        response_dict = self._build_response_dict("PUT", body)
        self._send_json_response(response_dict)
    
    def _handle_patch(self, body: bytes) -> None:
        """Handle /patch endpoint."""
        response_dict = self._build_response_dict("PATCH", body)
        self._send_json_response(response_dict)
    
    def _handle_delete(self, body: bytes) -> None:
        """Handle /delete endpoint."""
        response_dict = self._build_response_dict("DELETE", body)
        self._send_json_response(response_dict)
    
    def _handle_html(self) -> None:
        """Handle /html endpoint."""
        html_content = """<!DOCTYPE html>
<html>
<head>
    <title>Test Page</title>
</head>
<body>
    <h1>Welcome to Test Server</h1>
    <p>This is a test paragraph for text extraction.</p>
    <ul>
        <li>Item 1</li>
        <li>Item 2</li>
        <li>Item 3</li>
    </ul>
    <div class="content">
        <p>Another paragraph inside a div.</p>
    </div>
</body>
</html>"""
        self._send_html_response(html_content)
    
    def _handle_stream(self, path: str) -> None:
        """Handle /stream/<n> endpoint."""
        match = re.match(r"/stream/(\d+)", path)
        if match:
            n = int(match.group(1))
            self.send_response(200)
            self.send_header("Content-Type", "application/x-ndjson")
            self.send_header("Transfer-Encoding", "chunked")
            self.end_headers()
            
            try:
                for i in range(n):
                    line_data = {
                        "id": i,
                        "message": f"Stream line {i}",
                    }
                    chunk = json.dumps(line_data) + "\n"
                    chunk_bytes = chunk.encode("utf-8")
                    # Send chunk size in hex followed by CRLF, then chunk data, then CRLF
                    self.wfile.write(f"{len(chunk_bytes):x}\r\n".encode())
                    self.wfile.write(chunk_bytes)
                    self.wfile.write(b"\r\n")
                
                # Send final empty chunk
                self.wfile.write(b"0\r\n\r\n")
            except (BrokenPipeError, ConnectionResetError, OSError):
                # Client closed connection early - this is expected in streaming tests
                pass
        else:
            self._send_json_response({"error": "Invalid stream path"}, 400)
    
    def _handle_status(self, path: str) -> None:
        """Handle /status/<code> endpoint."""
        match = re.match(r"/status/(\d+)", path)
        if match:
            code = int(match.group(1))
            response_body = {
                "status": code,
                "message": f"Returning status code {code}",
            }
            self._send_json_response(response_body, code)
        else:
            self._send_json_response({"error": "Invalid status path"}, 400)
    
    def _handle_delay(self, path: str) -> None:
        """Handle /delay/<seconds> endpoint."""
        match = re.match(r"/delay/(\d+)", path)
        if match:
            seconds = int(match.group(1))
            time.sleep(seconds)
            response_body = {
                "delay": seconds,
                "message": f"Delayed for {seconds} seconds",
            }
            self._send_json_response(response_body)
        else:
            self._send_json_response({"error": "Invalid delay path"}, 400)
    
    def _handle_headers(self) -> None:
        """Handle /headers endpoint."""
        headers = dict(self.headers)
        self._send_json_response({"headers": headers})
    
    def _handle_ip(self) -> None:
        """Handle /ip endpoint."""
        self._send_json_response({"origin": self.client_address[0]})
    
    def _handle_user_agent(self) -> None:
        """Handle /user-agent endpoint."""
        user_agent = self.headers.get("User-Agent", "")
        self._send_json_response({"user-agent": user_agent})
    
    def _handle_cookies(self) -> None:
        """Handle /cookies endpoint."""
        cookies = {}
        cookie_header = self.headers.get("Cookie", "")
        if cookie_header:
            for cookie in cookie_header.split(";"):
                cookie = cookie.strip()
                if "=" in cookie:
                    name, value = cookie.split("=", 1)
                    cookies[name.strip()] = value.strip()
        self._send_json_response({"cookies": cookies})
    
    def _handle_cookies_set(self) -> None:
        """Handle /cookies/set endpoint."""
        parsed_url = urlparse(self.path)
        query_params = parse_qs(parsed_url.query)
        
        response_body = json.dumps({"message": "Cookies set"}).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(response_body)))
        
        for name, values in query_params.items():
            self.send_header("Set-Cookie", f"{name}={values[0]}")
        
        self.end_headers()
        self.wfile.write(response_body)
    
    def _handle_redirect(self, path: str) -> None:
        """Handle /redirect/<n> endpoint."""
        match = re.match(r"/redirect/(\d+)", path)
        if match:
            n = int(match.group(1))
            if n <= 1:
                self._send_json_response({"message": "Redirect complete"})
            else:
                self.send_response(302)
                self.send_header("Location", f"/redirect/{n - 1}")
                self.send_header("Content-Length", "0")
                self.end_headers()
        else:
            self._send_json_response({"error": "Invalid redirect path"}, 400)
    
    def _handle_gzip(self) -> None:
        """Handle /gzip endpoint."""
        import gzip as gzip_module

        data = {"gzipped": True, "message": "This response is gzipped"}
        json_data = json.dumps(data).encode("utf-8")
        gzipped_data = gzip_module.compress(json_data)

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(gzipped_data)))
        self.end_headers()
        self.wfile.write(gzipped_data)

    def _handle_invalid_gzip(self) -> None:
        """Handle /invalid-gzip endpoint - returns invalid gzip data."""
        # Send invalid gzip data that will cause DecodeError when client tries to decompress
        invalid_gzip_data = b"This is not valid gzip data at all!"

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(invalid_gzip_data)))
        self.end_headers()
        self.wfile.write(invalid_gzip_data)

    def _handle_invalid_deflate(self) -> None:
        """Handle /invalid-deflate endpoint - returns invalid deflate data."""
        # Send invalid deflate data that will cause DecodeError when client tries to decompress
        invalid_deflate_data = b"This is not valid deflate data at all!"

        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Encoding", "deflate")
        self.send_header("Content-Length", str(len(invalid_deflate_data)))
        self.end_headers()
        self.wfile.write(invalid_deflate_data)

    def _handle_broken_chunked(self) -> None:
        """Handle /broken-chunked endpoint - sends incomplete chunked response."""
        # Start a chunked response but close connection mid-stream
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        # Send a chunk with incorrect size (claim 100 bytes in hex, send only 10)
        self.wfile.write(b"64\r\n")  # Hex 64 = 100 bytes claimed
        self.wfile.write(b"short data")  # Only 10 bytes actually sent
        self.wfile.flush()
        # Force close the connection immediately to trigger body error
        # This simulates a premature connection close
        self.close_connection = True

    def _handle_broken_body(self) -> None:
        """Handle /broken-body endpoint - closes connection immediately without reading request body."""
        # Read the request line and headers but not the body
        # Then close the connection immediately to trigger a body error on the client side
        # This simulates a server that closes the connection while the client is sending the body
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"OK")
        self.wfile.flush()
        # Close immediately - if client was still sending body, it would get an error
        self.close_connection = True
    
    def _handle_xml(self) -> None:
        """Handle /xml endpoint."""
        xml_content = """<?xml version="1.0" encoding="UTF-8"?>
<root>
    <element>Test XML content</element>
    <nested>
        <item>Value 1</item>
        <item>Value 2</item>
    </nested>
</root>"""
        response_body = xml_content.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/xml")
        self.send_header("Content-Length", str(len(response_body)))
        self.end_headers()
        self.wfile.write(response_body)

    def _handle_upgrade(self) -> None:
        """Handle /upgrade endpoint - sends 101 Switching Protocols but fails the upgrade.
        
        This endpoint simulates a failed protocol upgrade by sending a 101 response
        but then immediately closing the connection without completing the upgrade handshake.
        This should trigger an UpgradeError when the client tries to use the upgraded connection.
        """
        # Check if client requested an upgrade
        upgrade_header = self.headers.get("Upgrade", "")
        connection_header = self.headers.get("Connection", "")
        
        if "upgrade" in connection_header.lower() and upgrade_header:
            # Send 101 Switching Protocols response
            self.send_response(101)
            self.send_header("Connection", "upgrade")
            self.send_header("Upgrade", upgrade_header)
            self.send_header("Content-Length", "0")
            self.end_headers()
            self.wfile.flush()
            # Close connection immediately without completing upgrade
            # This simulates a failed upgrade and should trigger UpgradeError
            self.close_connection = True
        else:
            # No upgrade requested, return normal response
            self._send_json_response({"error": "No Upgrade header provided"}, 400)


class ThreadedHTTPServer(ThreadingMixIn, HTTPServer):
    """Threaded HTTP server that handles each request in a new thread."""
    daemon_threads = True
    allow_reuse_address = True


class ServerThread(threading.Thread):
    """Thread that runs the HTTP server."""
    
    def __init__(self, port: int = 8080, host: str = "127.0.0.1"):
        super().__init__(daemon=True)
        self.port = port
        self.host = host
        self.server: ThreadedHTTPServer | None = None
    
    def run(self) -> None:
        """Run the HTTP server."""
        self.server = ThreadedHTTPServer((self.host, self.port), HttpbinRequestHandler)
        self.server.serve_forever()
    
    def shutdown(self) -> None:
        """Shutdown the server."""
        if self.server:
            self.server.shutdown()


@contextmanager
def run_server(port: int = 8080, host: str = "127.0.0.1") -> Generator[str, None, None]:
    """Context manager to run the test server.
    
    Args:
        port: Port to run the server on (default: 8080)
        host: Host to bind to (default: 127.0.0.1)
    
    Yields:
        The base URL of the server (e.g., "http://127.0.0.1:8080")
    
    Example:
        with run_server(port=8080) as base_url:
            response = requests.get(f"{base_url}/get")
    """
    server_thread = ServerThread(port, host)
    server_thread.start()
    
    # Wait for server to be ready
    import socket
    max_retries = 50
    for _ in range(max_retries):
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.settimeout(0.1)
            result = sock.connect_ex((host, port))
            sock.close()
            if result == 0:
                break
        except Exception:
            pass
        time.sleep(0.1)
    
    base_url = f"http://{host}:{port}"
    
    try:
        yield base_url
    finally:
        server_thread.shutdown()
        server_thread.join(timeout=2)


@pytest.fixture
def test_server_dynamic_port(unused_tcp_port: int) -> Generator[str, None, None]:
    """Pytest fixture that starts the server on a dynamically allocated port.
    
    Yields:
        The base URL of the server with a dynamic port.
    """
    with run_server(port=unused_tcp_port) as base_url:
        yield base_url


def main():
    """Run the server standalone."""
    import argparse
    
    parser = argparse.ArgumentParser(description="Run the test server")
    parser.add_argument("--port", type=int, default=8080, help="Port to run on")
    parser.add_argument("--host", default="127.0.0.1", help="Host to bind to")
    args = parser.parse_args()
    
    print(f"Starting test server on http://{args.host}:{args.port}")
    print("Available endpoints:")
    print("  GET  /anything          - Returns request details")
    print("  GET  /get               - Returns request details")
    print("  POST /post              - Returns request details with posted data")
    print("  PUT  /put               - Returns request details")
    print("  PATCH /patch            - Returns request details")
    print("  DELETE /delete          - Returns request details")
    print("  HEAD /anything          - Returns only headers")
    print("  OPTIONS /anything       - Returns allowed methods")
    print("  GET  /html              - Returns HTML content")
    print("  GET  /stream/<n>        - Returns n JSON lines streamed")
    print("  GET  /status/<code>     - Returns specified status code")
    print("  GET  /delay/<seconds>   - Returns after delay")
    print("  GET  /headers           - Returns request headers")
    print("  GET  /ip                - Returns client IP")
    print("  GET  /user-agent        - Returns User-Agent")
    print("  GET  /cookies           - Returns cookies")
    print("  GET  /cookies/set       - Sets cookies from query params")
    print("  GET  /redirect/<n>      - Redirects n times")
    print("  GET  /gzip              - Returns gzipped response")
    print("  GET  /invalid-gzip      - Returns invalid gzip (for DecodeError testing)")
    print("  GET  /invalid-deflate   - Returns invalid deflate (for DecodeError testing)")
    print("  GET  /broken-chunked    - Returns broken chunked response (for BodyError testing)")
    print("  GET  /xml               - Returns XML content")
    print("\nPress Ctrl+C to stop")
    
    try:
        server = ThreadedHTTPServer((args.host, args.port), HttpbinRequestHandler)
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nServer stopped.")


if __name__ == "__main__":
    main()
