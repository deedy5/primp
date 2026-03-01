"""
Tests for per-request parameters.

This module tests all per-request parameters:
- auth: Basic authentication per-request
- auth_bearer: Bearer token per-request
- params: Query parameters per-request
- headers: Headers per-request
- cookies: Cookies per-request
- timeout: Timeout per-request
- content: Raw content body
- data: Form data body
- json: JSON body
- files: File uploads
"""

import os
import tempfile

import pytest

import primp


class TestRequestAuth:
    """Tests for per-request auth parameter."""
    
    def test_sync_client_auth_per_request(self, test_server: str) -> None:
        """Test per-request auth on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/get", auth=("user", "pass"))
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
    
    @pytest.mark.asyncio
    async def test_async_client_auth_per_request(self, test_server: str) -> None:
        """Test per-request auth on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", auth=("user", "pass"))
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
    
    def test_module_auth_per_request(self, test_server: str) -> None:
        """Test per-request auth on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/get", auth=("user", "pass"))
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]


class TestRequestAuthBearer:
    """Tests for per-request auth_bearer parameter."""
    
    def test_sync_client_auth_bearer_per_request(self, test_server: str) -> None:
        """Test per-request auth_bearer on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/get", auth_bearer="test-token-123")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
        assert "test-token-123" in data["headers"]["authorization"]
    
    @pytest.mark.asyncio
    async def test_async_client_auth_bearer_per_request(self, test_server: str) -> None:
        """Test per-request auth_bearer on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", auth_bearer="test-token-123")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]
    
    def test_module_auth_bearer_per_request(self, test_server: str) -> None:
        """Test per-request auth_bearer on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/get", auth_bearer="test-token-123")
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert "authorization" in data["headers"]
        assert "Bearer" in data["headers"]["authorization"]


class TestRequestParams:
    """Tests for per-request params parameter."""
    
    def test_sync_client_params_per_request(self, test_server: str) -> None:
        """Test per-request params on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/get", params={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["key1"] == "value1"
        assert data["args"]["key2"] == "value2"
    
    @pytest.mark.asyncio
    async def test_async_client_params_per_request(self, test_server: str) -> None:
        """Test per-request params on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", params={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["key1"] == "value1"
        assert data["args"]["key2"] == "value2"
    
    def test_module_params_per_request(self, test_server: str) -> None:
        """Test per-request params on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/get", params={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["args"]["key1"] == "value1"
        assert data["args"]["key2"] == "value2"
    
    def test_params_override_client_defaults(self, test_server: str) -> None:
        """Test that per-request params override client default params."""
        base_url = test_server
        
        client = primp.Client(params={"default": "value"})
        # Per-request params replace client defaults
        response = client.get(f"{base_url}/get", params={"extra": "param"})
        
        assert response.status_code == 200
        data = response.json()
        # Per-request params replace client defaults
        assert data["args"]["extra"] == "param"


class TestRequestHeaders:
    """Tests for per-request headers parameter."""
    
    def test_sync_client_headers_per_request(self, test_server: str) -> None:
        """Test per-request headers on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/get", headers={"X-Custom": "custom-value"})
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "custom-value"
    
    @pytest.mark.asyncio
    async def test_async_client_headers_per_request(self, test_server: str) -> None:
        """Test per-request headers on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", headers={"X-Custom": "custom-value"})
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "custom-value"
    
    def test_module_headers_per_request(self, test_server: str) -> None:
        """Test per-request headers on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/get", headers={"X-Custom": "custom-value"})
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response
        assert data["headers"]["x-custom"] == "custom-value"
    
    def test_headers_merge_with_client_defaults(self, test_server: str) -> None:
        """Test that per-request headers merge with client default headers."""
        base_url = test_server
        
        client = primp.Client(headers={"X-Default": "default-value"})
        response = client.get(f"{base_url}/get", headers={"X-Custom": "custom-value"})
        
        assert response.status_code == 200
        data = response.json()
        # Headers are lowercase in response, and they merge
        assert data["headers"]["x-default"] == "default-value"
        assert data["headers"]["x-custom"] == "custom-value"


class TestRequestCookies:
    """Tests for per-request cookies parameter."""
    
    def test_sync_client_cookies_per_request(self, test_server: str) -> None:
        """Test per-request cookies on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/cookies", cookies={"test_cookie": "test_value"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["cookies"]["test_cookie"] == "test_value"
    
    @pytest.mark.asyncio
    async def test_async_client_cookies_per_request(self, test_server: str) -> None:
        """Test per-request cookies on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/cookies", cookies={"test_cookie": "test_value"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["cookies"]["test_cookie"] == "test_value"
    
    def test_module_cookies_per_request(self, test_server: str) -> None:
        """Test per-request cookies on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/cookies", cookies={"test_cookie": "test_value"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["cookies"]["test_cookie"] == "test_value"


class TestRequestTimeout:
    """Tests for per-request timeout parameter."""
    
    def test_sync_client_timeout_per_request(self, test_server: str) -> None:
        """Test per-request timeout on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.get(f"{base_url}/get", timeout=30)
        
        assert response.status_code == 200
    
    @pytest.mark.asyncio
    async def test_async_client_timeout_per_request(self, test_server: str) -> None:
        """Test per-request timeout on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", timeout=30)
        
        assert response.status_code == 200
    
    def test_module_timeout_per_request(self, test_server: str) -> None:
        """Test per-request timeout on module function."""
        base_url = test_server
        
        response = primp.get(f"{base_url}/get", timeout=30)
        
        assert response.status_code == 200


class TestRequestContent:
    """Tests for per-request content parameter (raw body)."""
    
    def test_sync_client_content_per_request(self, test_server: str) -> None:
        """Test per-request content on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.post(f"{base_url}/post", content=b"raw binary data")
        
        assert response.status_code == 200
        data = response.json()
        assert data["data"] == "raw binary data"
    
    @pytest.mark.asyncio
    async def test_async_client_content_per_request(self, test_server: str) -> None:
        """Test per-request content on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.post(f"{base_url}/post", content=b"raw binary data")
        
        assert response.status_code == 200
        data = response.json()
        assert data["data"] == "raw binary data"
    
    def test_module_content_per_request(self, test_server: str) -> None:
        """Test per-request content on module function."""
        base_url = test_server
        
        response = primp.post(f"{base_url}/post", content=b"raw binary data")
        
        assert response.status_code == 200
        data = response.json()
        assert data["data"] == "raw binary data"


class TestRequestData:
    """Tests for per-request data parameter (form data)."""
    
    def test_sync_client_data_per_request(self, test_server: str) -> None:
        """Test per-request data on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        response = client.post(f"{base_url}/post", data={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"
    
    @pytest.mark.asyncio
    async def test_async_client_data_per_request(self, test_server: str) -> None:
        """Test per-request data on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        response = await client.post(f"{base_url}/post", data={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"
    
    def test_module_data_per_request(self, test_server: str) -> None:
        """Test per-request data on module function."""
        base_url = test_server
        
        response = primp.post(f"{base_url}/post", data={"key1": "value1", "key2": "value2"})
        
        assert response.status_code == 200
        data = response.json()
        assert data["form"]["key1"] == "value1"
        assert data["form"]["key2"] == "value2"


class TestRequestJson:
    """Tests for per-request json parameter."""
    
    def test_sync_client_json_per_request(self, test_server: str) -> None:
        """Test per-request json on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        json_data = {"name": "test", "value": 123, "nested": {"key": "value"}}
        response = client.post(f"{base_url}/post", json=json_data)
        
        assert response.status_code == 200
        data = response.json()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123
        assert data["json"]["nested"]["key"] == "value"
    
    @pytest.mark.asyncio
    async def test_async_client_json_per_request(self, test_server: str) -> None:
        """Test per-request json on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        json_data = {"name": "test", "value": 123, "nested": {"key": "value"}}
        response = await client.post(f"{base_url}/post", json=json_data)
        
        assert response.status_code == 200
        data = response.json()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123
    
    def test_module_json_per_request(self, test_server: str) -> None:
        """Test per-request json on module function."""
        base_url = test_server
        
        json_data = {"name": "test", "value": 123}
        response = primp.post(f"{base_url}/post", json=json_data)
        
        assert response.status_code == 200
        data = response.json()
        assert data["json"]["name"] == "test"
        assert data["json"]["value"] == 123


class TestRequestFiles:
    """Tests for per-request files parameter."""
    
    def test_sync_client_files_per_request(self, test_server: str) -> None:
        """Test per-request files on sync Client."""
        base_url = test_server
        
        # Create a temporary file
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("This is test file content")
            temp_path = f.name
        
        try:
            client = primp.Client()
            response = client.post(f"{base_url}/post", files={"file": temp_path})
            
            assert response.status_code == 200
            data = response.json()
            assert data["method"] == "POST"
            assert "multipart/form-data" in data["headers"].get("content-type", "")
        finally:
            os.unlink(temp_path)
    
    @pytest.mark.asyncio
    async def test_async_client_files_per_request(self, test_server: str) -> None:
        """Test per-request files on AsyncClient."""
        base_url = test_server
        
        # Create a temporary file
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("This is test file content")
            temp_path = f.name
        
        try:
            client = primp.AsyncClient()
            response = await client.post(f"{base_url}/post", files={"file": temp_path})
            
            assert response.status_code == 200
            data = response.json()
            assert data["method"] == "POST"
            assert "multipart/form-data" in data["headers"].get("content-type", "")
        finally:
            os.unlink(temp_path)
    
    def test_module_files_per_request(self, test_server: str) -> None:
        """Test per-request files on module function."""
        base_url = test_server
        
        # Create a temporary file
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("This is test file content")
            temp_path = f.name
        
        try:
            response = primp.post(f"{base_url}/post", files={"file": temp_path})
            
            assert response.status_code == 200
            data = response.json()
            assert data["method"] == "POST"
            assert "multipart/form-data" in data["headers"].get("content-type", "")
        finally:
            os.unlink(temp_path)


class TestRequestMultipleFiles:
    """Tests for uploading multiple files."""
    
    def test_sync_client_multiple_files(self, test_server: str) -> None:
        """Test uploading multiple files on sync Client."""
        base_url = test_server
        
        # Create temporary files
        files = []
        try:
            for i in range(2):
                with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
                    f.write(f"File content {i}")
                    files.append((f"file{i}", f.name))
            
            client = primp.Client()
            response = client.post(
                f"{base_url}/post",
                files={name: path for name, path in files}
            )
            
            assert response.status_code == 200
            data = response.json()
            assert "multipart/form-data" in data["headers"].get("content-type", "")
        finally:
            for _, path in files:
                os.unlink(path)
