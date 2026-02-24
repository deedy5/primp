"""
Tests for HTTP methods across all client types.

This module tests all HTTP methods (get, head, options, delete, post, put, patch)
using pytest parametrization to reduce code duplication across:
- Synchronous Client
- AsyncClient
- Module-level functions
"""

import pytest

import primp


# HTTP methods that support request body
METHODS_WITH_BODY = ["post", "put", "patch"]

# All HTTP methods to test
ALL_METHODS = ["get", "head", "options", "delete", "post", "put", "patch"]


class TestSyncClientHttpMethods:
    """Tests for synchronous Client HTTP methods."""
    
    @pytest.mark.parametrize("method", ALL_METHODS)
    def test_sync_client_method(self, test_server_dynamic_port: str, method: str) -> None:
        """Test HTTP method on sync Client.
        
        Args:
            test_server_dynamic_port: Test server URL fixture.
            method: HTTP method name to test.
        """
        base_url = test_server_dynamic_port
        client = primp.Client()
        
        # Get the method from the client
        http_method = getattr(client, method)
        
        # Call appropriate endpoint
        if method == "head":
            response = http_method(f"{base_url}/anything")
        elif method == "options":
            response = http_method(f"{base_url}/anything")
        elif method in METHODS_WITH_BODY:
            response = http_method(f"{base_url}/{method}", json={"test": "data"})
        else:
            response = http_method(f"{base_url}/{method}")
        
        assert response.status_code == 200
        
        # HEAD and OPTIONS don't return body content in the same way
        if method not in ["head", "options"]:
            data = response.json()
            assert data["method"].upper() == method.upper()
    
    def test_sync_client_request_method(self, test_server_dynamic_port: str) -> None:
        """Test generic request() method on sync Client."""
        base_url = test_server_dynamic_port
        client = primp.Client()
        
        response = client.request("GET", f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"


class TestAsyncClientHttpMethods:
    """Tests for AsyncClient HTTP methods."""
    
    @pytest.mark.parametrize("method", ALL_METHODS)
    @pytest.mark.asyncio
    async def test_async_client_method(self, test_server_dynamic_port: str, method: str) -> None:
        """Test HTTP method on AsyncClient.
        
        Args:
            test_server_dynamic_port: Test server URL fixture.
            method: HTTP method name to test.
        """
        base_url = test_server_dynamic_port
        client = primp.AsyncClient()
        
        # Get the method from the client
        http_method = getattr(client, method)
        
        # Call appropriate endpoint
        if method == "head":
            response = await http_method(f"{base_url}/anything")
        elif method == "options":
            response = await http_method(f"{base_url}/anything")
        elif method in METHODS_WITH_BODY:
            response = await http_method(f"{base_url}/{method}", json={"test": "data"})
        else:
            response = await http_method(f"{base_url}/{method}")
        
        assert response.status_code == 200
        
        # HEAD and OPTIONS don't return body content in the same way
        if method not in ["head", "options"]:
            data = response.json()
            assert data["method"].upper() == method.upper()
    
    @pytest.mark.asyncio
    async def test_async_client_request_method(self, test_server_dynamic_port: str) -> None:
        """Test generic request() method on AsyncClient."""
        base_url = test_server_dynamic_port
        client = primp.AsyncClient()
        
        response = await client.request("GET", f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"


class TestModuleHttpMethods:
    """Tests for module-level HTTP functions."""
    
    @pytest.mark.parametrize("method", ALL_METHODS)
    def test_module_method(self, test_server_dynamic_port: str, method: str) -> None:
        """Test HTTP method as module-level function.
        
        Args:
            test_server_dynamic_port: Test server URL fixture.
            method: HTTP method name to test.
        """
        base_url = test_server_dynamic_port
        
        # Get the function from the module
        http_func = getattr(primp, method)
        
        # Call appropriate endpoint
        if method == "head":
            response = http_func(f"{base_url}/anything")
        elif method == "options":
            response = http_func(f"{base_url}/anything")
        elif method in METHODS_WITH_BODY:
            response = http_func(f"{base_url}/{method}", json={"test": "data"})
        else:
            response = http_func(f"{base_url}/{method}")
        
        assert response.status_code == 200
        
        # HEAD and OPTIONS don't return body content in the same way
        if method not in ["head", "options"]:
            data = response.json()
            assert data["method"].upper() == method.upper()
    
    def test_module_request_function(self, test_server_dynamic_port: str) -> None:
        """Test module-level request() function."""
        base_url = test_server_dynamic_port
        
        response = primp.request("GET", f"{base_url}/get")
        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"


class TestHttpMethodsWithBody:
    """Tests for HTTP methods that support request body."""
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_sync_client_method_with_content(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with raw content body on sync Client."""
        base_url = test_server_dynamic_port
        client = primp.Client()
        
        http_method = getattr(client, method)
        response = http_method(
            f"{base_url}/{method}",
            content=b"raw binary data",
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["data"] == "raw binary data"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    @pytest.mark.asyncio
    async def test_async_client_method_with_content(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with raw content body on AsyncClient."""
        base_url = test_server_dynamic_port
        client = primp.AsyncClient()
        
        http_method = getattr(client, method)
        response = await http_method(
            f"{base_url}/{method}",
            content=b"raw binary data",
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["data"] == "raw binary data"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_module_method_with_content(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with raw content body as module function."""
        base_url = test_server_dynamic_port
        
        http_func = getattr(primp, method)
        response = http_func(
            f"{base_url}/{method}",
            content=b"raw binary data",
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["data"] == "raw binary data"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_sync_client_method_with_data(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with form data body on sync Client."""
        base_url = test_server_dynamic_port
        client = primp.Client()
        
        http_method = getattr(client, method)
        response = http_method(
            f"{base_url}/{method}",
            data={"key1": "value1", "key2": "value2"},
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    @pytest.mark.asyncio
    async def test_async_client_method_with_data(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with form data body on AsyncClient."""
        base_url = test_server_dynamic_port
        client = primp.AsyncClient()
        
        http_method = getattr(client, method)
        response = await http_method(
            f"{base_url}/{method}",
            data={"key1": "value1", "key2": "value2"},
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_module_method_with_data(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with form data body as module function."""
        base_url = test_server_dynamic_port
        
        http_func = getattr(primp, method)
        response = http_func(
            f"{base_url}/{method}",
            data={"key1": "value1", "key2": "value2"},
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_sync_client_method_with_json(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with JSON body on sync Client."""
        base_url = test_server_dynamic_port
        client = primp.Client()
        
        http_method = getattr(client, method)
        json_data = {"name": "test", "value": 123}
        response = http_method(
            f"{base_url}/{method}",
            json=json_data,
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    @pytest.mark.asyncio
    async def test_async_client_method_with_json(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with JSON body on AsyncClient."""
        base_url = test_server_dynamic_port
        client = primp.AsyncClient()
        
        http_method = getattr(client, method)
        json_data = {"name": "test", "value": 123}
        response = await http_method(
            f"{base_url}/{method}",
            json=json_data,
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123
    
    @pytest.mark.parametrize("method", METHODS_WITH_BODY)
    def test_module_method_with_json(
        self, test_server_dynamic_port: str, method: str
    ) -> None:
        """Test HTTP method with JSON body as module function."""
        base_url = test_server_dynamic_port
        
        http_func = getattr(primp, method)
        json_data = {"name": "test", "value": 123}
        response = http_func(
            f"{base_url}/{method}",
            json=json_data,
        )
        
        assert response.status_code == 200
        data = response.json()
        assert data["method"].upper() == method.upper()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123
