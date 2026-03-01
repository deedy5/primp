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


# All HTTP methods to test
ALL_METHODS = ["get", "head", "options", "delete", "post", "put", "patch"]
METHODS_TRADITIONALLY_WITH_BODY = ["post", "put", "patch"]


class TestSyncClientHttpMethods:
    """Tests for synchronous Client HTTP methods."""

    @pytest.mark.parametrize("method", ALL_METHODS)
    def test_sync_client_method(self, test_server: str, method: str) -> None:
        """Test HTTP method on sync Client."""
        base_url = test_server
        client = primp.Client()
        http_method = getattr(client, method)

        if method in METHODS_TRADITIONALLY_WITH_BODY:
            response = http_method(f"{base_url}/{method}", json={"test": "data"})
        else:
            endpoint = "/anything" if method in ("head", "options") else f"/{method}"
            response = http_method(f"{base_url}{endpoint}")

        assert response.status_code == 200
        if method not in ("head", "options"):
            assert response.json()["method"].upper() == method.upper()

    def test_sync_client_request_method(self, test_server: str) -> None:
        """Test generic request() method on sync Client."""
        client = primp.Client()
        response = client.request("GET", f"{test_server}/get")
        assert response.status_code == 200
        assert response.json()["method"] == "GET"


class TestAsyncClientHttpMethods:
    """Tests for AsyncClient HTTP methods."""

    @pytest.mark.parametrize("method", ALL_METHODS)
    @pytest.mark.asyncio
    async def test_async_client_method(self, test_server: str, method: str) -> None:
        """Test HTTP method on AsyncClient."""
        base_url = test_server
        client = primp.AsyncClient()
        http_method = getattr(client, method)

        if method in METHODS_TRADITIONALLY_WITH_BODY:
            response = await http_method(f"{base_url}/{method}", json={"test": "data"})
        else:
            endpoint = "/anything" if method in ("head", "options") else f"/{method}"
            response = await http_method(f"{base_url}{endpoint}")

        assert response.status_code == 200
        if method not in ("head", "options"):
            assert response.json()["method"].upper() == method.upper()

    @pytest.mark.asyncio
    async def test_async_client_request_method(self, test_server: str) -> None:
        """Test generic request() method on AsyncClient."""
        client = primp.AsyncClient()
        response = await client.request("GET", f"{test_server}/get")
        assert response.status_code == 200
        assert response.json()["method"] == "GET"


class TestModuleHttpMethods:
    """Tests for module-level HTTP functions."""

    @pytest.mark.parametrize("method", ALL_METHODS)
    def test_module_method(self, test_server: str, method: str) -> None:
        """Test HTTP method as module-level function."""
        base_url = test_server
        http_func = getattr(primp, method)

        if method in METHODS_TRADITIONALLY_WITH_BODY:
            response = http_func(f"{base_url}/{method}", json={"test": "data"})
        else:
            endpoint = "/anything" if method in ("head", "options") else f"/{method}"
            response = http_func(f"{base_url}{endpoint}")

        assert response.status_code == 200
        if method not in ("head", "options"):
            assert response.json()["method"].upper() == method.upper()

    def test_module_request_function(self, test_server: str) -> None:
        """Test module-level request() function."""
        response = primp.request("GET", f"{test_server}/get")
        assert response.status_code == 200
        assert response.json()["method"] == "GET"


class TestHttpMethodsWithBody:
    """Tests for HTTP methods with request body parameters."""

    @pytest.mark.parametrize("method", ALL_METHODS)
    @pytest.mark.parametrize("body_type,body_value,expected_key,expected_value", [
        ("content", b"raw content", "data", "raw content"),
        ("data", {"key": "val"}, "form", {"key": "val"}),
        ("json", {"name": "test"}, "json", {"name": "test"}),
    ])
    def test_sync_client_with_body(
        self, test_server: str, method: str,
        body_type: str, body_value: object, expected_key: str, expected_value: object
    ) -> None:
        """Test sync Client HTTP method with various body types."""
        base_url = test_server
        client = primp.Client()
        http_method = getattr(client, method)

        endpoint = "/anything" if method in ("head", "options") else f"/{method}"
        response = http_method(f"{base_url}{endpoint}", **{body_type: body_value})

        assert response.status_code == 200
        if method == "head":
            assert response.text == ""  # HEAD has no body
        else:
            data = response.json()
            assert data["method"].upper() == method.upper()
            if expected_key == "form":
                assert data[expected_key] == expected_value
            else:
                assert data[expected_key] == expected_value

    @pytest.mark.parametrize("method", ALL_METHODS)
    @pytest.mark.asyncio
    async def test_async_client_with_body_content(
        self, test_server: str, method: str
    ) -> None:
        """Test async Client HTTP method with content body."""
        base_url = test_server
        client = primp.AsyncClient()
        http_method = getattr(client, method)

        endpoint = "/anything" if method in ("head", "options") else f"/{method}"
        response = await http_method(f"{base_url}{endpoint}", content=b"async content")

        assert response.status_code == 200
        if method != "head":
            assert response.json()["data"] == "async content"

    @pytest.mark.parametrize("method", ALL_METHODS)
    def test_module_with_body_json(self, test_server: str, method: str) -> None:
        """Test module-level HTTP method with JSON body."""
        base_url = test_server
        http_func = getattr(primp, method)

        endpoint = "/anything" if method in ("head", "options") else f"/{method}"
        response = http_func(f"{base_url}{endpoint}", json={"module": "test"})

        assert response.status_code == 200
        if method != "head":
            assert response.json()["json"] == {"module": "test"}


class TestBodyParameterEdgeCases:
    """Edge case tests for body parameters."""

    @pytest.mark.parametrize("method", ["get", "delete"])
    def test_sync_body_with_query_params(self, test_server: str, method: str) -> None:
        """Test sync Client HTTP method with both body and query parameters."""
        base_url = test_server
        client = primp.Client()
        http_method = getattr(client, method)

        response = http_method(
            f"{base_url}/{method}",
            params={"page": "1"},
            json={"filter": "active"},
        )

        assert response.status_code == 200
        data = response.json()
        assert data["args"]["page"] == "1"
        assert data["json"]["filter"] == "active"

    @pytest.mark.parametrize("method", ["get", "delete"])
    @pytest.mark.asyncio
    async def test_async_body_with_query_params(self, test_server: str, method: str) -> None:
        """Test async Client HTTP method with both body and query parameters."""
        base_url = test_server
        client = primp.AsyncClient()
        http_method = getattr(client, method)

        response = await http_method(
            f"{base_url}/{method}",
            params={"page": "1"},
            json={"filter": "active"},
        )

        assert response.status_code == 200
        data = response.json()
        assert data["args"]["page"] == "1"
        assert data["json"]["filter"] == "active"

    @pytest.mark.parametrize("method", ["get", "delete", "options"])
    def test_method_with_files(self, test_server: str, method: str) -> None:
        """Test HTTP method with files parameter (multipart upload)."""
        import os
        import tempfile

        base_url = test_server
        endpoint = "/anything" if method == "options" else f"/{method}"
        client = primp.Client()
        http_method = getattr(client, method)

        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("test file content")
            temp_path = f.name

        try:
            response = http_method(f"{base_url}{endpoint}", files={"upload": temp_path})
            assert response.status_code == 200
            assert "multipart/form-data" in response.json()["headers"].get("content-type", "")
        finally:
            os.unlink(temp_path)

    def test_custom_content_type_with_body(self, test_server: str) -> None:
        """Test GET with custom content-type header and body."""
        base_url = test_server
        client = primp.Client()

        response = client.get(
            f"{base_url}/get",
            content=b"custom content",
            headers={"Content-Type": "application/octet-stream"},
        )

        assert response.status_code == 200
        data = response.json()
        assert data["data"] == "custom content"
        assert "application/octet-stream" in data["headers"].get("content-type", "")
