"""
Tests for Client and AsyncClient initialization and properties.

This module tests:
- Client/AsyncClient initialization with all parameters
- Property getters (headers, proxy, impersonate, impersonate_os)
- Property setters (headers, proxy, impersonate, impersonate_os)
- headers_update() method
- get_cookies() and set_cookies() methods
- Context manager support
"""

import asyncio
import os
import subprocess
import sys

import pytest

import primp


class TestClientInit:
    """Tests for synchronous Client initialization."""
    
    def test_client_init_basic(self) -> None:
        """Test basic client initialization with no parameters."""
        client = primp.Client()
        assert client is not None
    
    def test_client_init_auth(self, test_server: str) -> None:
        """Test client initialization with auth parameter."""
        base_url = test_server
        
        client = primp.Client(auth=("user", "pass"))
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Check that Authorization header is set (lowercase in response)
        assert "authorization" in data["headers"]
    
    def test_client_init_auth_bearer(self, test_server: str) -> None:
        """Test client initialization with auth_bearer parameter."""
        base_url = test_server
        
        client = primp.Client(auth_bearer="test-token-123")
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Check that Authorization header is set with Bearer (lowercase in response)
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    def test_client_init_params(self, test_server: str) -> None:
        """Test client initialization with default params."""
        base_url = test_server
        
        client = primp.Client(params={"default_param": "value"})
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["default_param"] == "value"
    
    def test_client_init_headers(self, test_server: str) -> None:
        """Test client initialization with default headers."""
        base_url = test_server
        
        client = primp.Client(headers={"X-Custom": "value"})
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "value"
    
    def test_client_init_cookie_store(self) -> None:
        """Test client initialization with cookie_store parameter."""
        # Test with cookie_store=True
        client = primp.Client(cookie_store=True)
        assert client is not None
        
        # Test with cookie_store=False
        client = primp.Client(cookie_store=False)
        assert client is not None
    
    def test_client_init_referer(self, test_server: str) -> None:
        """Test client initialization with referer parameter (bool to auto-set referer)."""
        base_url = test_server
        
        # referer is a bool parameter that controls auto-referer
        client = primp.Client(referer=True)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_client_init_timeout(self, test_server: str) -> None:
        """Test client initialization with timeout parameter."""
        base_url = test_server
        
        client = primp.Client(timeout=30)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_client_init_connect_timeout(self) -> None:
        """Test client initialization with connect_timeout parameter."""
        client = primp.Client(connect_timeout=5)
        assert client is not None
    
    def test_client_init_read_timeout(self, test_server: str) -> None:
        """Test client initialization with read_timeout parameter."""
        base_url = test_server
        
        client = primp.Client(read_timeout=10)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_client_init_follow_redirects(self) -> None:
        """Test client initialization with follow_redirects parameter."""
        client = primp.Client(follow_redirects=True)
        assert client is not None
        
        client = primp.Client(follow_redirects=False)
        assert client is not None
    
    def test_client_init_max_redirects(self) -> None:
        """Test client initialization with max_redirects parameter."""
        client = primp.Client(max_redirects=5)
        assert client is not None
    
    def test_client_init_verify(self) -> None:
        """Test client initialization with verify parameter."""
        client = primp.Client(verify=True)
        assert client is not None
        
        client = primp.Client(verify=False)
        assert client is not None
    
    def test_client_init_https_only(self) -> None:
        """Test client initialization with https_only parameter."""
        client = primp.Client(https_only=True)
        assert client is not None
    
    def test_client_init_http2_only(self) -> None:
        """Test client initialization with http2_only parameter."""
        client = primp.Client(http2_only=True)
        assert client is not None
    
    def test_client_init_base_url(self) -> None:
        """Test client initialization with base_url parameter."""
        client = primp.Client(base_url="https://httpbin.org")
        assert client is not None
        assert client.base_url == "https://httpbin.org"
    
    def test_client_init_cookies(self) -> None:
        """Test client initialization with cookies parameter."""
        client = primp.Client(cookies={"session": "abc123"})
        assert client is not None


class TestClientProperties:
    """Tests for Client property getters."""
    
    def test_client_headers_getter(self, test_server: str) -> None:
        """Test headers property getter."""
        base_url = test_server
        
        client = primp.Client(headers={"X-Test": "value"})
        headers = client.headers
        
        assert headers is not None
        assert isinstance(headers, dict)
    
    def test_client_proxy_getter(self) -> None:
        """Test proxy property getter."""
        client = primp.Client()
        proxy = client.proxy
        
        # Proxy should be None if not set
        assert proxy is None
    
    def test_client_impersonate_getter(self) -> None:
        """Test impersonate property getter."""
        client = primp.Client()
        impersonate = client.impersonate
        
        # Should be None if not set
        assert impersonate is None
    
    def test_client_impersonate_getter_set(self) -> None:
        """Test impersonate property getter when set."""
        client = primp.Client(impersonate="chrome_144")
        assert client.impersonate == "chrome_144"
    
    def test_client_impersonate_os_getter(self) -> None:
        """Test impersonate_os property getter."""
        client = primp.Client()
        impersonate_os = client.impersonate_os
        
        # Should be None if not set
        assert impersonate_os is None
    
    def test_client_impersonate_os_getter_set(self) -> None:
        """Test impersonate_os property getter when set."""
        client = primp.Client(impersonate_os="windows")
        assert client.impersonate_os == "windows"
    
    def test_client_ca_cert_file(self) -> None:
        """Test client initialization with ca_cert_file parameter."""
        client = primp.Client(ca_cert_file=None)
        assert client is not None


class TestClientSetters:
    """Tests for Client property setters."""
    
    def test_client_headers_setter(self, test_server: str) -> None:
        """Test headers property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.headers = {"X-New-Header": "new_value"}
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-new-header"] == "new_value"
    
    def test_client_proxy_setter(self) -> None:
        """Test proxy property setter."""
        client = primp.Client()
        client.proxy = "http://localhost:9999"
        assert client.proxy == "http://localhost:9999"
    
    def test_client_auth_setter(self, test_server: str) -> None:
        """Test auth property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.auth = ("user", "pass")
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert "authorization" in data["headers"]
    
    def test_client_auth_bearer_setter(self, test_server: str) -> None:
        """Test auth_bearer property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.auth_bearer = "my-token"
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    def test_client_params_setter(self, test_server: str) -> None:
        """Test params property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.params = {"set_param": "value"}
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["set_param"] == "value"
    
    def test_client_timeout_setter(self, test_server: str) -> None:
        """Test timeout property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.timeout = 30
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
    
    def test_client_connect_timeout_setter_rejected(self) -> None:
        """connect_timeout is construction-only; post-construction mutation
        must raise AttributeError, not silently no-op (finding F9)."""
        client = primp.Client()
        with pytest.raises(AttributeError):
            client.connect_timeout = 5
        assert client.connect_timeout is None
        client2 = primp.Client(connect_timeout=5)
        assert client2.connect_timeout == 5

    def test_client_dns_timeout_setter_rejected(self) -> None:
        """dns_timeout is construction-only; post-construction
        mutation must raise AttributeError."""
        client = primp.Client()
        with pytest.raises(AttributeError):
            client.dns_timeout = 5
        assert client.dns_timeout is None
        client2 = primp.Client(dns_timeout=5)
        assert client2.dns_timeout == 5
    
    def test_client_read_timeout_setter(self, test_server: str) -> None:
        """Test read_timeout property setter."""
        base_url = test_server
        
        client = primp.Client()
        client.read_timeout = 10
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
    
    def test_client_base_url_setter(self) -> None:
        """Test base_url property setter."""
        client = primp.Client()
        client.base_url = "https://httpbin.org"
        assert client.base_url == "https://httpbin.org"


class TestClientInvalidTimeouts:
    """NaN/negative/infinite timeouts must raise ValueError, never abort.

    Timeouts are converted with Duration::from_secs_f64, which panics on
    NaN/negative/infinite values; the release profile aborts the whole
    process (panic = "abort"), so the Python bindings must validate at the
    boundary and raise ValueError instead.
    """

    INVALID = [float("nan"), float("inf"), -1.0, -0.0001]

    @pytest.mark.parametrize("value", INVALID)
    def test_client_init_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.Client(timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    def test_client_init_connect_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.Client(connect_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    def test_client_init_read_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.Client(read_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    def test_client_init_dns_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.Client(dns_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_asyncclient_init_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.AsyncClient(timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_asyncclient_init_connect_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.AsyncClient(connect_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_asyncclient_init_read_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.AsyncClient(read_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_asyncclient_init_dns_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.AsyncClient(dns_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_async_per_request_timeout_invalid(self, test_server: str, value: float) -> None:
        client = primp.AsyncClient()
        with pytest.raises(primp.PrimpError):
            await client.get(f"{test_server}/get", timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    @pytest.mark.asyncio
    async def test_async_per_request_read_timeout_invalid(self, test_server: str, value: float) -> None:
        client = primp.AsyncClient()
        with pytest.raises(primp.PrimpError):
            await client.get(f"{test_server}/get", read_timeout=value)

    @pytest.mark.parametrize("value", INVALID)
    def test_module_timeout_invalid(self, value: float) -> None:
        with pytest.raises(primp.PrimpError):
            primp.get("http://127.0.0.1:1/", timeout=value)


class TestClientHeadersUpdate:
    """Tests for Client headers_update() method."""
    
    def test_client_headers_update(self, test_server: str) -> None:
        """Test headers_update() method adds headers to existing."""
        base_url = test_server
        
        client = primp.Client(headers={"X-Original": "original"})
        client.headers_update({"X-Added": "added"})
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-original"] == "original"
        assert data["headers"]["x-added"] == "added"


class TestClientCookies:
    """Tests for Client cookie methods."""
    
    def test_client_set_and_get_cookies(self, test_server: str) -> None:
        """Test set_cookies() and get_cookies() methods."""
        base_url = test_server
        
        client = primp.Client()
        # set_cookies requires URL and cookies dict
        client.set_cookies(base_url, {"test_cookie": "test_value"})
        
        # Verify cookies were set
        cookies = client.get_cookies(base_url)
        assert "test_cookie" in cookies
        assert cookies["test_cookie"] == "test_value"

    def test_client_get_cookies_empty_jar_returns_empty_dict(
        self, test_server: str
    ) -> None:
        """get_cookies() on an empty jar returns {} instead of raising."""
        base_url = test_server

        client = primp.Client()
        cookies = client.get_cookies(base_url)
        assert cookies == {}

    def test_asyncclient_get_cookies_empty_jar_returns_empty_dict(
        self, test_server: str
    ) -> None:
        """AsyncClient get_cookies() on an empty jar returns {} instead of raising."""
        base_url = test_server

        client = primp.AsyncClient()
        cookies = client.get_cookies(base_url)
        assert cookies == {}

    def test_client_cookie_store_enabled(self, test_server: str) -> None:
        """Test that cookie_store parameter works."""
        base_url = test_server
        
        # Test with cookie_store enabled
        client = primp.Client(cookie_store=True)
        response = client.get(f"{base_url}/cookies/set?session_id=abc123")
        
        assert response.status_code == 200


class TestClientContextManager:
    """Tests for Client context manager support."""
    
    def test_client_context_manager(self, test_server: str) -> None:
        """Test using Client as a context manager."""
        base_url = test_server
        
        with primp.Client() as client:
            response = client.get(f"{base_url}/get")
            assert response.status_code == 200


class TestAsyncClientInit:
    """Tests for AsyncClient initialization."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_basic(self) -> None:
        """Test basic async client initialization with no parameters."""
        client = primp.AsyncClient()
        assert client is not None
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_auth(self, test_server: str) -> None:
        """Test async client initialization with auth parameter."""
        base_url = test_server
        
        client = primp.AsyncClient(auth=("user", "pass"))
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_auth_bearer(self, test_server: str) -> None:
        """Test async client initialization with auth_bearer parameter."""
        base_url = test_server
        
        client = primp.AsyncClient(auth_bearer="test-token-123")
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_params(self, test_server: str) -> None:
        """Test async client initialization with default params."""
        base_url = test_server
        
        client = primp.AsyncClient(params={"default_param": "value"})
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["default_param"] == "value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_headers(self, test_server: str) -> None:
        """Test async client initialization with default headers."""
        base_url = test_server
        
        client = primp.AsyncClient(headers={"X-Custom": "value"})
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_timeout(self, test_server: str) -> None:
        """Test async client initialization with timeout parameter."""
        base_url = test_server
        
        client = primp.AsyncClient(timeout=30)
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_asyncclient_init_connect_timeout(self) -> None:
        """Test async client initialization with connect_timeout parameter."""
        client = primp.AsyncClient(connect_timeout=5)
        assert client is not None
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_read_timeout(self, test_server: str) -> None:
        """Test async client initialization with read_timeout parameter."""
        base_url = test_server
        
        client = primp.AsyncClient(read_timeout=10)
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_asyncclient_init_proxy(self) -> None:
        """Test async client initialization with proxy parameter."""
        # proxy is accepted without error even if not reachable
        client = primp.AsyncClient(proxy="http://localhost:9999")
        assert client is not None
        assert client.proxy == "http://localhost:9999"
    
    def test_asyncclient_init_verify(self) -> None:
        """Test async client initialization with verify parameter."""
        client = primp.AsyncClient(verify=True)
        assert client is not None
        
        client = primp.AsyncClient(verify=False)
        assert client is not None
    
    def test_asyncclient_init_https_only(self) -> None:
        """Test async client initialization with https_only parameter."""
        client = primp.AsyncClient(https_only=True)
        assert client is not None
    
    def test_asyncclient_init_http2_only(self) -> None:
        """Test async client initialization with http2_only parameter."""
        client = primp.AsyncClient(http2_only=True)
        assert client is not None

    def test_asyncclient_init_base_url(self) -> None:
        """Test async client initialization with base_url parameter."""
        client = primp.AsyncClient(base_url="https://httpbin.org")
        assert client is not None
        assert client.base_url == "https://httpbin.org"
    
    def test_asyncclient_init_cookies(self) -> None:
        """Test async client initialization with cookies parameter."""
        client = primp.AsyncClient(cookies={"session": "abc123"})
        assert client is not None

    def test_asyncclient_init_cookie_store(self) -> None:
        """Test async client initialization with cookie_store parameter."""
        client = primp.AsyncClient(cookie_store=True)
        assert client is not None

        client = primp.AsyncClient(cookie_store=False)
        assert client is not None

    def test_asyncclient_init_referer(self, test_server: str) -> None:
        """Test async client initialization with referer parameter."""
        client = primp.AsyncClient(referer=True)
        assert client is not None

        client = primp.AsyncClient(referer=False)
        assert client is not None

    def test_asyncclient_init_ca_cert_file(self) -> None:
        """Test async client initialization with ca_cert_file parameter."""
        client = primp.AsyncClient(ca_cert_file=None)
        assert client is not None


class TestAsyncClientProperties:
    """Tests for AsyncClient property getters."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_getter(self, test_server: str) -> None:
        """Test headers property getter."""
        base_url = test_server
        
        client = primp.AsyncClient(headers={"X-Test": "value"})
        headers = client.headers
        
        assert headers is not None
        assert isinstance(headers, dict)
    
    @pytest.mark.asyncio
    async def test_asyncclient_proxy_getter(self) -> None:
        """Test proxy property getter."""
        client = primp.AsyncClient()
        proxy = client.proxy
        
        assert proxy is None
    
    @pytest.mark.asyncio
    async def test_asyncclient_impersonate_getter(self) -> None:
        """Test impersonate property getter."""
        client = primp.AsyncClient()
        impersonate = client.impersonate
        
        assert impersonate is None

    def test_asyncclient_impersonate_getter_set(self) -> None:
        """Test impersonate property getter when set."""
        client = primp.AsyncClient(impersonate="chrome_144")
        assert client.impersonate == "chrome_144"

    def test_asyncclient_impersonate_os_getter(self) -> None:
        """Test impersonate_os property getter."""
        client = primp.AsyncClient()
        assert client.impersonate_os is None

    def test_asyncclient_impersonate_os_getter_set(self) -> None:
        """Test impersonate_os property getter when set."""
        client = primp.AsyncClient(impersonate_os="windows")
        assert client.impersonate_os == "windows"


class TestAsyncClientSetters:
    """Tests for AsyncClient property setters."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_setter(self, test_server: str) -> None:
        """Test headers property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.headers = {"X-New-Header": "new_value"}
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-new-header"] == "new_value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_auth_setter(self, test_server: str) -> None:
        """Test auth property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.auth = ("user", "pass")
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert "authorization" in data["headers"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_auth_bearer_setter(self, test_server: str) -> None:
        """Test auth_bearer property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.auth_bearer = "my-token"
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_params_setter(self, test_server: str) -> None:
        """Test params property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.params = {"set_param": "value"}
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["set_param"] == "value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_timeout_setter(self, test_server: str) -> None:
        """Test timeout property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.timeout = 30
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
    
    def test_asyncclient_connect_timeout_setter_rejected(self) -> None:
        """connect_timeout is construction-only; post-construction mutation
        must raise AttributeError, not silently no-op (finding F9)."""
        client = primp.AsyncClient()
        with pytest.raises(AttributeError):
            client.connect_timeout = 5
        assert client.connect_timeout is None
        client2 = primp.AsyncClient(connect_timeout=5)
        assert client2.connect_timeout == 5

    @pytest.mark.asyncio
    async def test_asyncclient_dns_timeout_setter_rejected(self) -> None:
        """dns_timeout is construction-only; post-construction
        mutation must raise AttributeError."""
        client = primp.AsyncClient()
        with pytest.raises(AttributeError):
            client.dns_timeout = 5
        assert client.dns_timeout is None
        client2 = primp.AsyncClient(dns_timeout=5)
        assert client2.dns_timeout == 5
    
    @pytest.mark.asyncio
    async def test_asyncclient_read_timeout_setter(self, test_server: str) -> None:
        """Test read_timeout property setter."""
        base_url = test_server
        
        client = primp.AsyncClient()
        client.read_timeout = 10
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
    
    def test_asyncclient_proxy_setter(self) -> None:
        """Test proxy property setter."""
        client = primp.AsyncClient()
        client.proxy = "http://localhost:9999"
        assert client.proxy == "http://localhost:9999"
    
    def test_asyncclient_base_url_setter(self) -> None:
        """Test base_url property setter."""
        client = primp.AsyncClient()
        client.base_url = "https://httpbin.org"
        assert client.base_url == "https://httpbin.org"


class TestAsyncClientHeadersUpdate:
    """Tests for AsyncClient headers_update() method."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_update(self, test_server: str) -> None:
        """Test headers_update() method adds headers to existing."""
        base_url = test_server
        
        client = primp.AsyncClient(headers={"X-Original": "original"})
        client.headers_update({"X-Added": "added"})
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-original"] == "original"
        assert data["headers"]["x-added"] == "added"


class TestAsyncClientCookies:
    """Tests for AsyncClient cookie methods."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_set_and_get_cookies(self, test_server: str) -> None:
        """Test set_cookies() and get_cookies() methods."""
        base_url = test_server
        
        client = primp.AsyncClient()
        # set_cookies requires URL and cookies dict
        client.set_cookies(base_url, {"test_cookie": "test_value"})
        
        # Verify cookies were set
        cookies = client.get_cookies(base_url)
        assert "test_cookie" in cookies
        assert cookies["test_cookie"] == "test_value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_cookie_store_enabled(self, test_server: str) -> None:
        """Test that cookie_store parameter works."""
        base_url = test_server
        
        # Test with cookie_store enabled
        client = primp.AsyncClient(cookie_store=True)
        response = await client.get(f"{base_url}/cookies/set?session_id=abc123")
        
        assert response.status_code == 200


class TestAsyncClientContextManager:
    """Tests for AsyncClient context manager support."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_context_manager(self, test_server: str) -> None:
        """Test using AsyncClient as a context manager."""
        base_url = test_server
        
        async with primp.AsyncClient() as client:
            response = await client.get(f"{base_url}/get")
            assert response.status_code == 200


class TestAsyncBridgeCoroutines:
    """Async methods return coroutines, not bare asyncio.Futures (a bare
    Future fails `asyncio.create_task`: "a coroutine was expected")."""

    @pytest.mark.asyncio
    async def test_asyncclient_verb_returns_coroutine(self, test_server: str) -> None:
        client = primp.AsyncClient()
        ret = client.get(f"{test_server}/get")
        assert asyncio.iscoroutine(ret), type(ret)
        task = asyncio.create_task(ret)
        response = await task
        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_asyncclient_request_method_returns_coroutine(self, test_server: str) -> None:
        client = primp.AsyncClient()
        ret = client.request("GET", f"{test_server}/get")
        assert asyncio.iscoroutine(ret), type(ret)
        response = await asyncio.create_task(ret)
        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_asyncclient_verb_wrappers_return_coroutine(self, test_server: str) -> None:
        client = primp.AsyncClient()
        for fn in (
            client.delete,
            client.head,
            client.options,
            client.patch,
            client.post,
            client.put,
        ):
            ret = fn(f"{test_server}/anything")
            assert asyncio.iscoroutine(ret), f"{fn.__name__} -> {type(ret)}"
            response = await asyncio.create_task(ret)
            assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_asyncclient_aenter_aexit_return_coroutines(self, test_server: str) -> None:
        client = primp.AsyncClient()
        aenter = client.__aenter__()
        assert asyncio.iscoroutine(aenter), type(aenter)
        entered = await asyncio.create_task(aenter)
        assert entered is client
        aexit = client.__aexit__(None, None, None)
        assert asyncio.iscoroutine(aexit), type(aexit)
        await asyncio.create_task(aexit)

    @pytest.mark.asyncio
    async def test_asyncclient_streaming_response_aread_returns_coroutine(
        self, test_server: str
    ) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get", stream=True)
        ret = response.aread()
        assert asyncio.iscoroutine(ret), type(ret)
        body = await asyncio.create_task(ret)
        assert isinstance(body, bytes)

    @pytest.mark.asyncio
    async def test_asyncclient_buffered_aread_returns_coroutine(self, test_server: str) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get")
        assert response.content
        ret = response.aread()
        assert asyncio.iscoroutine(ret), type(ret)
        body = await asyncio.create_task(ret)
        assert isinstance(body, bytes)

    @pytest.mark.asyncio
    async def test_asyncclient_aread_then_content_text_json(self, test_server: str) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get")
        body = await asyncio.create_task(response.aread())
        assert isinstance(body, bytes) and body
        assert response.content == body
        assert response.text == body.decode("utf-8")
        assert response.json() is not None

    @pytest.mark.asyncio
    async def test_asyncclient_aread_then_content_twice(self, test_server: str) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get")
        await asyncio.create_task(response.aread())
        first = response.content
        second = response.content
        assert first == second and first

    @pytest.mark.asyncio
    async def test_asyncclient_aclose_returns_coroutine(self, test_server: str) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get", stream=True)
        ret = response.aclose()
        assert asyncio.iscoroutine(ret), type(ret)
        await asyncio.create_task(ret)

    @pytest.mark.asyncio
    async def test_asyncclient_response_ctx_methods_return_coroutines(
        self, test_server: str
    ) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/stream/3", stream=True)
        aenter = response.__aenter__()
        assert asyncio.iscoroutine(aenter), type(aenter)
        assert await asyncio.create_task(aenter) is response
        aexit = response.__aexit__(None, None, None)
        assert asyncio.iscoroutine(aexit), type(aexit)
        assert await asyncio.create_task(aexit) is False

    def test_pending_future_at_interpreter_exit_does_not_panic(self) -> None:
        """A pending primp future at interpreter exit must not panic (regression:
        post-finalize Python::attach SIGABRT'd release builds)."""
        script = (
            "import asyncio, socket, threading, primp\n"
            "lsock = socket.socket()\n"
            "lsock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)\n"
            "lsock.bind(('127.0.0.1', 0))\n"
            "lsock.listen(8)\n"
            "port = lsock.getsockname()[1]\n"
            "def accept_loop():\n"
            "    while True:\n"
            "        try:\n"
            "            conn, _ = lsock.accept()\n"
            "        except OSError:\n"
            "            return\n"
            "        threading.Thread(\n"
            "            target=lambda c: (c.recv(65536), c.recv(65536)),\n"
            "            args=(conn,), daemon=True,\n"
            "        ).start()\n"
            "threading.Thread(target=accept_loop, name='acceptor', daemon=True).start()\n"
            "async def main():\n"
            "    client = primp.AsyncClient()\n"
            "    coro = client.get(f'http://127.0.0.1:{port}/hang', timeout=120)\n"
            "    asyncio.create_task(coro)\n"
            "    await asyncio.sleep(0.2)\n"
            "asyncio.run(main())\n"
        )
        env = os.environ.copy()
        for key in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"):
            env.pop(key, None)
        proc = subprocess.run(
            [sys.executable, "-c", script],
            capture_output=True,
            text=True,
            env=env,
            timeout=60,
        )
        assert "panicked" not in proc.stderr, proc.stderr
        assert "SIGABRT" not in proc.stderr, proc.stderr
        assert proc.returncode == 0, proc.stderr
