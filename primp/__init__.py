from __future__ import annotations

import asyncio
import sys
from functools import partial
from typing import Any, Dict, Optional, Tuple

if sys.version_info <= (3, 11):
    from typing_extensions import Unpack
else:
    from typing import Unpack

# Import the Rust Client directly
from .primp import Client as RustClient

# Type aliases
HttpMethod = str
RequestParams = Dict[str, Any]


class Client(RustClient):
    """HTTP client that can impersonate web browsers."""

    def __enter__(self) -> 'Client':
        return self

    def __exit__(self, *args) -> None:
        pass

    def get(self, url: str, **kwargs) -> Any:
        """Send GET request."""
        return self.request("GET", url, **kwargs)

    def post(self, url: str, **kwargs) -> Any:
        """Send POST request."""
        return self.request("POST", url, **kwargs)

    def put(self, url: str, **kwargs) -> Any:
        """Send PUT request."""
        return self.request("PUT", url, **kwargs)

    def patch(self, url: str, **kwargs) -> Any:
        """Send PATCH request."""
        return self.request("PATCH", url, **kwargs)

    def delete(self, url: str, **kwargs) -> Any:
        """Send DELETE request."""
        return self.request("DELETE", url, **kwargs)

    def head(self, url: str, **kwargs) -> Any:
        """Send HEAD request."""
        return self.request("HEAD", url, **kwargs)

    def options(self, url: str, **kwargs) -> Any:
        """Send OPTIONS request."""
        return self.request("OPTIONS", url, **kwargs)


class AsyncClient:
    """Async HTTP client that can impersonate web browsers."""

    def __init__(
        self,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        params: Optional[Dict[str, Any]] = None,
        headers: Optional[Dict[str, str]] = None,
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
    ):
        """Initialize AsyncClient with the same parameters as the Rust Client."""
        self._client = RustClient(
            auth=auth,
            auth_bearer=auth_bearer,
            params=params,
            headers=headers,
            cookie_store=cookie_store,
            referer=referer,
            proxy=proxy,
            timeout=timeout,
            impersonate=impersonate,
            impersonate_os=impersonate_os,
            follow_redirects=follow_redirects,
            max_redirects=max_redirects,
            verify=verify,
            ca_cert_file=ca_cert_file,
            https_only=https_only,
            http2_only=http2_only,
        )

    async def __aenter__(self) -> 'AsyncClient':
        return self

    async def __aexit__(self, *args) -> None:
        pass

    def __getattr__(self, name):
        """Delegate attribute access to the underlying client."""
        return getattr(self._client, name)

    async def _run_sync_asyncio(self, fn, *args, **kwargs):
        """Run synchronous function in thread pool."""
        loop = asyncio.get_running_loop()
        return await loop.run_in_executor(None, partial(fn, *args, **kwargs))

    async def request(
        self,
        method: str,
        url: str,
        params: Optional[Dict[str, Any]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Any] = None,
        json: Optional[Any] = None,
        files: Optional[Dict[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
        **kwargs
    ):
        """Send async HTTP request."""
        return await self._run_sync_asyncio(
            self._client.request,
            method,
            url,
            params=params,
            headers=headers,
            cookies=cookies,
            content=content,
            data=data,
            json=json,
            files=files,
            auth=auth,
            auth_bearer=auth_bearer,
            timeout=timeout,
            **kwargs
        )

    async def get(self, url: str, **kwargs):
        """Send async GET request."""
        return await self.request("GET", url, **kwargs)

    async def post(self, url: str, **kwargs):
        """Send async POST request."""
        return await self.request("POST", url, **kwargs)

    async def put(self, url: str, **kwargs):
        """Send async PUT request."""
        return await self.request("PUT", url, **kwargs)

    async def patch(self, url: str, **kwargs):
        """Send async PATCH request."""
        return await self.request("PATCH", url, **kwargs)

    async def delete(self, url: str, **kwargs):
        """Send async DELETE request."""
        return await self.request("DELETE", url, **kwargs)

    async def head(self, url: str, **kwargs):
        """Send async HEAD request."""
        return await self.request("HEAD", url, **kwargs)

    async def options(self, url: str, **kwargs):
        """Send async OPTIONS request."""
        return await self.request("OPTIONS", url, **kwargs)


def request(
    method: str,
    url: str,
    impersonate: Optional[str] = None,
    verify: bool = True,
    ca_cert_file: Optional[str] = None,
    **kwargs,
):
    """
    Send HTTP request with optional browser impersonation.
    
    Args:
        method: HTTP method (GET, POST, etc.).
        url: Target URL.
        impersonate: Browser to impersonate.
        verify: Whether to verify SSL certificates.
        ca_cert_file: Path to CA certificate file.
        **kwargs: Additional request parameters.
    
    Returns:
        Response object.
    """
    with Client(
        impersonate=impersonate,
        verify=verify,
        ca_cert_file=ca_cert_file,
    ) as client:
        return client.request(method, url, **kwargs)


def get(url: str, **kwargs):
    """Send GET request."""
    return request("GET", url, **kwargs)


def post(url: str, **kwargs):
    """Send POST request."""
    return request("POST", url, **kwargs)


def put(url: str, **kwargs):
    """Send PUT request."""
    return request("PUT", url, **kwargs)


def patch(url: str, **kwargs):
    """Send PATCH request."""
    return request("PATCH", url, **kwargs)


def delete(url: str, **kwargs):
    """Send DELETE request."""
    return request("DELETE", url, **kwargs)


def head(url: str, **kwargs):
    """Send HEAD request."""
    return request("HEAD", url, **kwargs)


def options(url: str, **kwargs):
    """Send OPTIONS request."""
    return request("OPTIONS", url, **kwargs)


__version__ = "0.15.1"
__all__ = [
    "Client",
    "AsyncClient", 
    "request",
    "get",
    "post",
    "put",
    "patch", 
    "delete",
    "head",
    "options",
]