"""
Type stubs for primp - HTTP client that can impersonate web browsers.

This module provides both synchronous and asynchronous HTTP clients with
browser impersonation capabilities.
"""

from typing import (
    Any,
    Dict,
    Iterator,
    Mapping,
    Optional,
    Tuple,
    Union,
    AsyncIterator,
)

# =============================================================================
# Type Aliases
# =============================================================================

HeadersType = Optional[Mapping[str, str]]
ParamsType = Optional[Mapping[str, str]]
CookiesType = Optional[Mapping[str, str]]
FilesType = Optional[Mapping[str, str]]
AuthType = Optional[Tuple[str, Optional[str]]]

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
    def encoding(self, value: Optional[str]) -> None: ...
    @property
    def headers(self) -> Dict[str, str]: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
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


class StreamResponse:
    """
    Streaming HTTP response object.
    
    Supports iteration over response chunks and context manager protocol.
    """
    
    # Properties (read-only)
    url: str
    status_code: int
    
    @property
    def headers(self) -> Dict[str, str]: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
    @property
    def encoding(self) -> str: ...
    @encoding.setter
    def encoding(self, value: str) -> None: ...
    @property
    def content(self) -> bytes: ...
    @property
    def text(self) -> str: ...
    
    # Methods
    def read(self) -> bytes: ...
    def iter_bytes(self, chunk_size: Optional[int] = None) -> BytesIterator: ...
    def iter_text(self, chunk_size: Optional[int] = None) -> TextIterator: ...
    def iter_lines(self) -> LinesIterator: ...
    def next(self) -> Optional[bytes]: ...
    def close(self) -> None: ...
    def raise_for_status(self) -> None: ...
    
    # Context manager
    def __enter__(self) -> StreamResponse: ...
    def __exit__(
        self,
        exc_type: Optional[Any],
        exc_value: Optional[Any],
        traceback: Optional[Any],
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
    def encoding(self, value: Optional[str]) -> None: ...
    @property
    def headers(self) -> Dict[str, str]: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
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


class AsyncStreamResponse:
    """
    Asynchronous streaming HTTP response object.
    
    Supports async iteration over response chunks and async context manager protocol.
    """
    
    # Properties (read-only)
    url: str
    status_code: int
    
    @property
    def headers(self) -> Dict[str, str]: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
    @property
    def encoding(self) -> str: ...
    @encoding.setter
    def encoding(self, value: str) -> None: ...
    @property
    def content(self) -> bytes: ...
    @property
    def text(self) -> str: ...
    
    # Async methods
    async def aread(self) -> bytes: ...
    def aiter_bytes(self, chunk_size: Optional[int] = None) -> AsyncBytesIterator: ...
    def aiter_text(self, chunk_size: Optional[int] = None) -> AsyncTextIterator: ...
    def aiter_lines(self) -> AsyncLinesIterator: ...
    async def anext(self) -> Optional[bytes]: ...
    async def aclose(self) -> None: ...
    def raise_for_status(self) -> None: ...
    
    # Async context manager
    async def __aenter__(self) -> AsyncStreamResponse: ...
    async def __aexit__(
        self,
        exc_type: Optional[Any],
        exc_value: Optional[Any],
        traceback: Optional[Any],
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
        >>> client = Client(impersonate="chrome_144")
        >>> response = client.get("https://example.com")
        >>> print(response.text)
    """
    
    # Instance properties
    auth: Optional[Tuple[str, Optional[str]]]
    auth_bearer: Optional[str]
    params: Optional[Mapping[str, str]]
    proxy: Optional[str]
    timeout: Optional[float]
    
    @property
    def headers(self) -> Dict[str, str]: ...
    @headers.setter
    def headers(self, value: Optional[Mapping[str, str]]) -> None: ...
    @property
    def impersonate(self) -> Optional[str]: ...
    @property
    def impersonate_os(self) -> Optional[str]: ...
    
    def __init__(
        self,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookie_store: bool = True,
        referer: bool = True,
        proxy: Optional[str] = None,
        timeout: Optional[float] = None,
        impersonate: Optional[str] = None,
        impersonate_os: Optional[str] = None,
        follow_redirects: bool = True,
        max_redirects: int = 20,
        verify: bool = True,
        ca_cert_file: Optional[str] = None,
        https_only: bool = False,
        http2_only: bool = False,
    ) -> None: ...
    
    def headers_update(self, new_headers: Optional[Mapping[str, str]]) -> None: ...
    def get_cookies(self, url: str) -> Dict[str, str]: ...
    def set_cookies(self, url: str, cookies: Optional[Mapping[str, str]]) -> None: ...
    
    def request(
        self,
        method: str,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def get(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def head(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def options(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def delete(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def post(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def put(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    def patch(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[Response, StreamResponse]: ...
    
    # Context manager
    def __enter__(self) -> Client: ...
    def __exit__(
        self,
        exc_type: Optional[Any],
        exc_value: Optional[Any],
        traceback: Optional[Any],
    ) -> None: ...


class AsyncClient:
    """
    Asynchronous HTTP client that can impersonate web browsers.
    
    This client supports browser impersonation, proxy configuration,
    cookie management, and various authentication methods.
    
    Example:
        >>> async with AsyncClient(impersonate="chrome_144") as client:
        ...     response = await client.get("https://example.com")
        ...     print(response.text)
    """
    
    # Instance properties
    auth: Optional[Tuple[str, Optional[str]]]
    auth_bearer: Optional[str]
    params: Optional[Mapping[str, str]]
    proxy: Optional[str]
    timeout: Optional[float]
    
    @property
    def headers(self) -> Dict[str, str]: ...
    @headers.setter
    def headers(self, value: Optional[Mapping[str, str]]) -> None: ...
    @property
    def impersonate(self) -> Optional[str]: ...
    @property
    def impersonate_os(self) -> Optional[str]: ...
    
    def __init__(
        self,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookie_store: bool = True,
        referer: bool = True,
        proxy: Optional[str] = None,
        timeout: Optional[float] = None,
        impersonate: Optional[str] = None,
        impersonate_os: Optional[str] = None,
        follow_redirects: bool = True,
        max_redirects: int = 20,
        verify: bool = True,
        ca_cert_file: Optional[str] = None,
        https_only: bool = False,
        http2_only: bool = False,
    ) -> None: ...
    
    def headers_update(self, new_headers: Optional[Mapping[str, str]]) -> None: ...
    def get_cookies(self, url: str) -> Dict[str, str]: ...
    def set_cookies(self, url: str, cookies: Optional[Mapping[str, str]]) -> None: ...
    
    async def request(
        self,
        method: str,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def get(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def head(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def options(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def delete(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def post(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def put(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    async def patch(
        self,
        url: str,
        params: Optional[Mapping[str, str]] = None,
        headers: Optional[Mapping[str, str]] = None,
        cookies: Optional[Mapping[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Mapping[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        stream: bool = False,
    ) -> Union[AsyncResponse, AsyncStreamResponse]: ...
    
    # Async context manager
    async def __aenter__(self) -> AsyncClient: ...
    async def __aexit__(
        self,
        exc_type: Optional[Any],
        exc_value: Optional[Any],
        traceback: Optional[Any],
    ) -> None: ...


# =============================================================================
# Module-level Functions
# =============================================================================

def get(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a GET request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def head(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a HEAD request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def options(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send an OPTIONS request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def delete(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a DELETE request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def post(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Any] = None,
    json: Optional[Any] = None,
    files: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a POST request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        content: Raw bytes to send in the request body.
        data: Form data to send in the request body.
        json: JSON-serializable object to send in the request body.
        files: Mapping of field names to file paths for multipart upload.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def put(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Any] = None,
    json: Optional[Any] = None,
    files: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a PUT request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        content: Raw bytes to send in the request body.
        data: Form data to send in the request body.
        json: JSON-serializable object to send in the request body.
        files: Mapping of field names to file paths for multipart upload.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def patch(
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Any] = None,
    json: Optional[Any] = None,
    files: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a PATCH request with a temporary client.
    
    Args:
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        content: Raw bytes to send in the request body.
        data: Form data to send in the request body.
        json: JSON-serializable object to send in the request body.
        files: Mapping of field names to file paths for multipart upload.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...


def request(
    method: str,
    url: str,
    params: Optional[Mapping[str, str]] = None,
    headers: Optional[Mapping[str, str]] = None,
    cookies: Optional[Mapping[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Any] = None,
    json: Optional[Any] = None,
    files: Optional[Mapping[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    impersonate_os: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    stream: bool = False,
) -> Union[Response, StreamResponse]:
    """
    Send a request with a custom HTTP method using a temporary client.
    
    Args:
        method: The HTTP method to use (e.g., "GET", "POST", "CUSTOM").
        url: The URL to request.
        params: Query parameters to append to the URL.
        headers: HTTP headers to send with the request.
        cookies: Cookies to send with the request.
        content: Raw bytes to send in the request body.
        data: Form data to send in the request body.
        json: JSON-serializable object to send in the request body.
        files: Mapping of field names to file paths for multipart upload.
        auth: Tuple of (username, password) for basic authentication.
        auth_bearer: Bearer token for authentication.
        timeout: Request timeout in seconds.
        impersonate: Browser to impersonate (e.g., "chrome_144").
        impersonate_os: OS to impersonate (e.g., "windows", "macos", "linux").
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        stream: If True, returns a StreamResponse for streaming.
    
    Returns:
        Response or StreamResponse object.
    """
    ...
