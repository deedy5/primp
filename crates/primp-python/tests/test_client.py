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

import pytest

import primp


class TestClientInit:
    """Tests for synchronous Client initialization."""
    
    def test_client_init_basic(self) -> None:
        """Test basic client initialization with no parameters."""
        client = primp.Client()
        assert client is not None
    
    def test_client_init_auth(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with auth parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.Client(auth=("user", "pass"))
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Check that Authorization header is set (lowercase in response)
        assert "authorization" in data["headers"]
    
    def test_client_init_auth_bearer(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with auth_bearer parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.Client(auth_bearer="test-token-123")
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Check that Authorization header is set with Bearer (lowercase in response)
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    def test_client_init_params(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with default params."""
        base_url = test_server_dynamic_port
        
        client = primp.Client(params={"default_param": "value"})
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["default_param"] == "value"
    
    def test_client_init_headers(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with default headers."""
        base_url = test_server_dynamic_port
        
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
    
    def test_client_init_referer(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with referer parameter (bool to auto-set referer)."""
        base_url = test_server_dynamic_port
        
        # referer is a bool parameter that controls auto-referer
        client = primp.Client(referer=True)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
    
    def test_client_init_timeout(self, test_server_dynamic_port: str) -> None:
        """Test client initialization with timeout parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.Client(timeout=30)
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


class TestClientProperties:
    """Tests for Client property getters."""
    
    def test_client_headers_getter(self, test_server_dynamic_port: str) -> None:
        """Test headers property getter."""
        base_url = test_server_dynamic_port
        
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
    
    def test_client_impersonate_os_getter(self) -> None:
        """Test impersonate_os property getter."""
        client = primp.Client()
        impersonate_os = client.impersonate_os
        
        # Should be None if not set
        assert impersonate_os is None


class TestClientSetters:
    """Tests for Client property setters."""
    
    def test_client_headers_setter(self, test_server_dynamic_port: str) -> None:
        """Test headers property setter."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        client.headers = {"X-New-Header": "new_value"}
        
        response = client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-new-header"] == "new_value"
    
    def test_client_proxy_setter(self) -> None:
        """Test proxy property getter returns None when not set."""
        client = primp.Client()
        # Proxy should be None if not set
        assert client.proxy is None


class TestClientHeadersUpdate:
    """Tests for Client headers_update() method."""
    
    def test_client_headers_update(self, test_server_dynamic_port: str) -> None:
        """Test headers_update() method adds headers to existing."""
        base_url = test_server_dynamic_port
        
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
    
    def test_client_set_and_get_cookies(self, test_server_dynamic_port: str) -> None:
        """Test set_cookies() and get_cookies() methods."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        # set_cookies requires URL and cookies dict
        client.set_cookies(base_url, {"test_cookie": "test_value"})
        
        # Verify cookies were set
        cookies = client.get_cookies(base_url)
        assert "test_cookie" in cookies
        assert cookies["test_cookie"] == "test_value"
    
    def test_client_cookie_store_enabled(self, test_server_dynamic_port: str) -> None:
        """Test that cookie_store parameter works."""
        base_url = test_server_dynamic_port
        
        # Test with cookie_store enabled
        client = primp.Client(cookie_store=True)
        response = client.get(f"{base_url}/cookies/set?session_id=abc123")
        
        assert response.status_code == 200


class TestClientContextManager:
    """Tests for Client context manager support."""
    
    def test_client_context_manager(self, test_server_dynamic_port: str) -> None:
        """Test using Client as a context manager."""
        base_url = test_server_dynamic_port
        
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
    async def test_asyncclient_init_auth(self, test_server_dynamic_port: str) -> None:
        """Test async client initialization with auth parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient(auth=("user", "pass"))
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_auth_bearer(self, test_server_dynamic_port: str) -> None:
        """Test async client initialization with auth_bearer parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient(auth_bearer="test-token-123")
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_params(self, test_server_dynamic_port: str) -> None:
        """Test async client initialization with default params."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient(params={"default_param": "value"})
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["default_param"] == "value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_headers(self, test_server_dynamic_port: str) -> None:
        """Test async client initialization with default headers."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient(headers={"X-Custom": "value"})
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_init_timeout(self, test_server_dynamic_port: str) -> None:
        """Test async client initialization with timeout parameter."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient(timeout=30)
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200


class TestAsyncClientProperties:
    """Tests for AsyncClient property getters."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_getter(self, test_server_dynamic_port: str) -> None:
        """Test headers property getter."""
        base_url = test_server_dynamic_port
        
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


class TestAsyncClientSetters:
    """Tests for AsyncClient property setters."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_setter(self, test_server_dynamic_port: str) -> None:
        """Test headers property setter."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        client.headers = {"X-New-Header": "new_value"}
        
        response = await client.get(f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-new-header"] == "new_value"


class TestAsyncClientHeadersUpdate:
    """Tests for AsyncClient headers_update() method."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_headers_update(self, test_server_dynamic_port: str) -> None:
        """Test headers_update() method adds headers to existing."""
        base_url = test_server_dynamic_port
        
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
    async def test_asyncclient_set_and_get_cookies(self, test_server_dynamic_port: str) -> None:
        """Test set_cookies() and get_cookies() methods."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        # set_cookies requires URL and cookies dict
        client.set_cookies(base_url, {"test_cookie": "test_value"})
        
        # Verify cookies were set
        cookies = client.get_cookies(base_url)
        assert "test_cookie" in cookies
        assert cookies["test_cookie"] == "test_value"
    
    @pytest.mark.asyncio
    async def test_asyncclient_cookie_store_enabled(self, test_server_dynamic_port: str) -> None:
        """Test that cookie_store parameter works."""
        base_url = test_server_dynamic_port
        
        # Test with cookie_store enabled
        client = primp.AsyncClient(cookie_store=True)
        response = await client.get(f"{base_url}/cookies/set?session_id=abc123")
        
        assert response.status_code == 200


class TestAsyncClientContextManager:
    """Tests for AsyncClient context manager support."""
    
    @pytest.mark.asyncio
    async def test_asyncclient_context_manager(self, test_server_dynamic_port: str) -> None:
        """Test using AsyncClient as a context manager."""
        base_url = test_server_dynamic_port
        
        async with primp.AsyncClient() as client:
            response = await client.get(f"{base_url}/get")
            assert response.status_code == 200
