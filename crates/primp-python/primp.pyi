"""
Type stubs for primp - HTTP client that can impersonate web browsers.

This module provides both synchronous and asynchronous HTTP clients with
browser impersonation capabilities.
"""

from collections.abc import Mapping
from typing import (
    Any,
)

# =============================================================================
# Type Aliases
# =============================================================================

HeadersType = Mapping[str, str] | None
ParamsType = Mapping[str, str] | None
CookiesType = Mapping[str, str] | None
FilesType = Mapping[str, str] | None
AuthType = tuple[str, str | None] | None

# =============================================================================
# Exception Classes
# =============================================================================

class PrimpError(Exception):
    """Base exception for all primp errors."""

    ...

class BuilderError(PrimpError):
    """Error during client builder configuration."""

    ...

class RequestError(PrimpError):
    """Error during request execution."""

    ...

class ConnectError(RequestError):
    """Error connecting to the server."""

    ...

class TimeoutError(RequestError):
    """Request timed out."""

    ...

class StatusError(PrimpError):
    """HTTP status error (4xx/5xx)."""

    ...

class RedirectError(PrimpError):
    """Error during redirect handling."""

    ...

class BodyError(PrimpError):
    """Error with request/response body."""

    ...

class DecodeError(PrimpError):
    """Error decoding response content."""

    ...

class UpgradeError(PrimpError):
    """Error during protocol upgrade."""

    ...

# =============================================================================
# Response Classes (Synchronous)
# =============================================================================

class Response:
    """
    HTTP response object.

    Provides access to response status, headers, cookies, and body content.
    """

    # Properties (read-only)
    url: str
    status_code: int

    @property
    def content(self) -> bytes: ...
    @property
    def encoding(self) -> str: ...
    @encoding.setter
    def encoding(self, value: str | None) -> None: ...
    @property
    def headers(self) -> dict[str, str]: ...
    @property
    def cookies(self) -> dict[str, str]: ...
    @property
    def text(self) -> str: ...
    @property
    def text_markdown(self) -> str: ...
    @property
    def text_plain(self) -> str: ...
    @property
    def text_rich(self) -> str: ...

    # Methods
    def json(self) -> Any: ...
    def raise_for_status(self) -> None: ...

    # Streaming methods
    def read(self) -> bytes: ...
    def iter_bytes(self, chunk_size: int | None = None) -> BytesIterator: ...
    def iter_text(self, chunk_size: int | None = None) -> TextIterator: ...
    def iter_lines(self) -> LinesIterator: ...
    def next(self) -> bytes | None: ...
    def close(self) -> None: ...

    # Context manager
    def __enter__(self) -> Response: ...
    def __exit__(
        self,
        exc_type: Any | None,
        exc_value: Any | None,
        traceback: Any | None,
    ) -> bool: ...

class BytesIterator:
    """Iterator over byte chunks from a streaming response."""

    def __iter__(self) -> BytesIterator: ...
    def __next__(self) -> bytes: ...

class TextIterator:
    """Iterator over text chunks from a streaming response."""

    def __iter__(self) -> TextIterator: ...
    def __next__(self) -> str: ...

class LinesIterator:
    """Iterator over lines from a streaming response."""

    def __iter__(self) -> LinesIterator: ...
    def __next__(self) -> str: ...

# =============================================================================
# Response Classes (Asynchronous)
# =============================================================================

class AsyncResponse:
    """
    Asynchronous HTTP response object.

    Provides access to response status, headers, cookies, and body content.
    All property access is synchronous but may block.
    """

    # Properties (read-only)
    url: str
    status_code: int

    @property
    def content(self) -> bytes: ...
    @property
    def encoding(self) -> str: ...
    @encoding.setter
    def encoding(self, value: str | None) -> None: ...
    @property
    def headers(self) -> dict[str, str]: ...
    @property
    def cookies(self) -> dict[str, str]: ...
    @property
    def text(self) -> str: ...
    @property
    def text_markdown(self) -> str: ...
    @property
    def text_plain(self) -> str: ...
    @property
    def text_rich(self) -> str: ...

    # Methods
    def json(self) -> Any: ...
    def raise_for_status(self) -> None: ...

    # Async streaming methods
    async def aread(self) -> bytes: ...
    def aiter_bytes(self, chunk_size: int | None = None) -> AsyncBytesIterator: ...
    def aiter_text(self, chunk_size: int | None = None) -> AsyncTextIterator: ...
    def aiter_lines(self) -> AsyncLinesIterator: ...
    async def anext(self) -> bytes | None: ...
    async def aclose(self) -> None: ...

    # Async context manager
    async def __aenter__(self) -> AsyncResponse: ...
    async def __aexit__(
        self,
        exc_type: Any | None,
        exc_value: Any | None,
        traceback: Any | None,
    ) -> bool: ...

class AsyncBytesIterator:
    """Async iterator over byte chunks from a streaming response."""

    def __aiter__(self) -> AsyncBytesIterator: ...
    async def __anext__(self) -> bytes: ...

class AsyncTextIterator:
    """Async iterator over text chunks from a streaming response."""

    def __aiter__(self) -> AsyncTextIterator: ...
    async def __anext__(self) -> str: ...

class AsyncLinesIterator:
    """Async iterator over lines from a streaming response."""

    def __aiter__(self) -> AsyncLinesIterator: ...
    async def __anext__(self) -> str: ...

# =============================================================================
# Client Classes
# =============================================================================

class Client:
    """
    Synchronous HTTP client that can impersonate web browsers.

    This client supports browser impersonation, proxy configuration,
    cookie management, and various authentication methods.

    Example:
        >>> client = Client(impersonate="chrome_146")
        >>> response = client.get("https://example.com")
        >>> print(response.text)
    """

    # Instance properties
    auth: tuple[str, str | None] | None
    auth_bearer: str | None
    params: Mapping[str, str] | None
    proxy: str | None
    timeout: float | None
    connect_timeout: float | None
    read_timeout: float | None
    base_url: str | None

    @property
    def headers(self) -> dict[str, str]: ...
    @headers.setter
    def headers(self, value: Mapping[str, str] | None) -> None: ...
    @property
    def impersonate(self) -> str | None: ...
    @property
    def impersonate_os(self) -> str | None: ...
    def __init__(
        self,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookie_store: bool = True,
        referer: bool = True,
        proxy: str | None = None,
        timeout: float | None = None,
        connect_timeout: float | None = None,
        read_timeout: float | None = None,
        impersonate: str | None = None,
        impersonate_os: str | None = None,
        follow_redirects: bool = True,
        max_redirects: int = 20,
        verify: bool = True,
        ca_cert_file: str | None = None,
        https_only: bool = False,
        http2_only: bool = False,
        base_url: str | None = None,
        cookies: Mapping[str, str] | None = None,
    ) -> None: ...
    def headers_update(self, new_headers: Mapping[str, str] | None) -> None: ...
    def get_cookies(self, url: str) -> dict[str, str]: ...
    def set_cookies(self, url: str, cookies: Mapping[str, str] | None) -> None: ...
    def request(
        self,
        method: str,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def get(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def head(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def options(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def delete(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def post(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def put(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...
    def patch(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> Response: ...

    # Context manager
    def __enter__(self) -> Client: ...
    def __exit__(
        self,
        exc_type: Any | None,
        exc_value: Any | None,
        traceback: Any | None,
    ) -> None: ...

class AsyncClient:
    """
    Asynchronous HTTP client that can impersonate web browsers.

    This client supports browser impersonation, proxy configuration,
    cookie management, and various authentication methods.

    Example:
        >>> async with AsyncClient(impersonate="chrome_146") as client:
        ...     response = await client.get("https://example.com")
        ...     print(response.text)
    """

    # Instance properties
    auth: tuple[str, str | None] | None
    auth_bearer: str | None
    params: Mapping[str, str] | None
    proxy: str | None
    timeout: float | None
    connect_timeout: float | None
    read_timeout: float | None
    base_url: str | None

    @property
    def headers(self) -> dict[str, str]: ...
    @headers.setter
    def headers(self, value: Mapping[str, str] | None) -> None: ...
    @property
    def impersonate(self) -> str | None: ...
    @property
    def impersonate_os(self) -> str | None: ...
    def __init__(
        self,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookie_store: bool = True,
        referer: bool = True,
        proxy: str | None = None,
        timeout: float | None = None,
        connect_timeout: float | None = None,
        read_timeout: float | None = None,
        impersonate: str | None = None,
        impersonate_os: str | None = None,
        follow_redirects: bool = True,
        max_redirects: int = 20,
        verify: bool = True,
        ca_cert_file: str | None = None,
        https_only: bool = False,
        http2_only: bool = False,
        base_url: str | None = None,
        cookies: Mapping[str, str] | None = None,
    ) -> None: ...
    def headers_update(self, new_headers: Mapping[str, str] | None) -> None: ...
    def get_cookies(self, url: str) -> dict[str, str]: ...
    def set_cookies(self, url: str, cookies: Mapping[str, str] | None) -> None: ...
    async def request(
        self,
        method: str,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def get(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def head(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def options(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def delete(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def post(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def put(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...
    async def patch(
        self,
        url: str,
        params: Mapping[str, str] | None = None,
        headers: Mapping[str, str] | None = None,
        cookies: Mapping[str, str] | None = None,
        content: bytes | None = None,
        data: Any | None = None,
        json: Any | None = None,
        files: Mapping[str, str] | None = None,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        timeout: float | None = None,
        read_timeout: float | None = None,
        follow_redirects: bool | None = None,
        stream: bool = False,
    ) -> AsyncResponse: ...

    # Async context manager
    async def __aenter__(self) -> AsyncClient: ...
    async def __aexit__(
        self,
        exc_type: Any | None,
        exc_value: Any | None,
        traceback: Any | None,
    ) -> None: ...

# =============================================================================
# Module-level Functions
# =============================================================================

def get(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a GET request with a temporary client."""
    ...

def head(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a HEAD request with a temporary client."""
    ...

def options(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send an OPTIONS request with a temporary client."""
    ...

def delete(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a DELETE request with a temporary client."""
    ...

def post(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a POST request with a temporary client."""
    ...

def put(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a PUT request with a temporary client."""
    ...

def patch(
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a PATCH request with a temporary client."""
    ...

def request(
    method: str,
    url: str,
    params: Mapping[str, str] | None = None,
    headers: Mapping[str, str] | None = None,
    cookies: Mapping[str, str] | None = None,
    content: bytes | None = None,
    data: Any | None = None,
    json: Any | None = None,
    files: Mapping[str, str] | None = None,
    auth: tuple[str, str | None] | None = None,
    auth_bearer: str | None = None,
    timeout: float | None = None,
    connect_timeout: float | None = None,
    read_timeout: float | None = None,
    impersonate: str | None = None,
    impersonate_os: str | None = None,
    verify: bool = True,
    ca_cert_file: str | None = None,
    follow_redirects: bool | None = None,
    stream: bool = False,
) -> Response:
    """Send a request with a custom HTTP method using a temporary client."""
    ...
