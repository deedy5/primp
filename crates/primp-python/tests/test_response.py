"""
Tests for Response properties and methods.

This module tests:
- url: Response URL
- status_code: HTTP status code
- content: Raw bytes content
- encoding: Response encoding
- text: Text content
- headers: Response headers
- cookies: Response cookies
- text_markdown: Markdown conversion
- text_plain: Plain text extraction
- text_rich: Rich text conversion
- json(): JSON parsing
- raise_for_status(): Status code handling
"""

import pytest

import primp


class TestResponseUrl:
    """Tests for Response.url property."""
    
    def test_sync_client_response_url(self, test_server_dynamic_port: str) -> None:
        """Test response URL property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.url is not None
        assert "/get" in response.url
    
    @pytest.mark.asyncio
    async def test_async_client_response_url(self, test_server_dynamic_port: str) -> None:
        """Test response URL property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.url is not None
        assert "/get" in response.url


class TestResponseStatusCode:
    """Tests for Response.status_code property."""
    
    def test_sync_client_response_status_code(self, test_server_dynamic_port: str) -> None:
        """Test response status_code property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        assert isinstance(response.status_code, int)
    
    @pytest.mark.asyncio
    async def test_async_client_response_status_code(self, test_server_dynamic_port: str) -> None:
        """Test response status_code property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        assert isinstance(response.status_code, int)
    
    def test_sync_client_response_status_code_404(self, test_server_dynamic_port: str) -> None:
        """Test response status_code for 404 on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/status/404")
        
        assert response.status_code == 404
    
    @pytest.mark.asyncio
    async def test_async_client_response_status_code_404(self, test_server_dynamic_port: str) -> None:
        """Test response status_code for 404 on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/status/404")
        
        assert response.status_code == 404


class TestResponseContent:
    """Tests for Response.content property."""
    
    def test_sync_client_response_content(self, test_server_dynamic_port: str) -> None:
        """Test response content property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.content is not None
        assert isinstance(response.content, bytes)
        assert len(response.content) > 0
    
    @pytest.mark.asyncio
    async def test_async_client_response_content(self, test_server_dynamic_port: str) -> None:
        """Test response content property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.content is not None
        assert isinstance(response.content, bytes)
        assert len(response.content) > 0


class TestResponseEncoding:
    """Tests for Response.encoding property."""
    
    def test_sync_client_response_encoding(self, test_server_dynamic_port: str) -> None:
        """Test response encoding property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        # Encoding may be None or a string
        if response.encoding is not None:
            assert isinstance(response.encoding, str)
    
    @pytest.mark.asyncio
    async def test_async_client_response_encoding(self, test_server_dynamic_port: str) -> None:
        """Test response encoding property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        if response.encoding is not None:
            assert isinstance(response.encoding, str)


class TestResponseText:
    """Tests for Response.text property."""
    
    def test_sync_client_response_text(self, test_server_dynamic_port: str) -> None:
        """Test response text property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.text is not None
        assert isinstance(response.text, str)
        assert len(response.text) > 0
    
    @pytest.mark.asyncio
    async def test_async_client_response_text(self, test_server_dynamic_port: str) -> None:
        """Test response text property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.text is not None
        assert isinstance(response.text, str)
        assert len(response.text) > 0
    
    def test_sync_client_response_text_html(self, test_server_dynamic_port: str) -> None:
        """Test response text property for HTML content."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        text = response.text
        assert "Welcome to Test Server" in text or "Test Page" in text


class TestResponseHeaders:
    """Tests for Response.headers property."""
    
    def test_sync_client_response_headers(self, test_server_dynamic_port: str) -> None:
        """Test response headers property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.headers is not None
        assert isinstance(response.headers, dict)
        # Check for common headers
        assert "content-type" in response.headers or "Content-Type" in response.headers
    
    @pytest.mark.asyncio
    async def test_async_client_response_headers(self, test_server_dynamic_port: str) -> None:
        """Test response headers property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.headers is not None
        assert isinstance(response.headers, dict)


class TestResponseCookies:
    """Tests for Response.cookies property."""
    
    def test_sync_client_response_cookies(self, test_server_dynamic_port: str) -> None:
        """Test response cookies property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/cookies/set?test_cookie=test_value")
        
        # Response should have cookies
        cookies = response.cookies
        assert cookies is not None
    
    @pytest.mark.asyncio
    async def test_async_client_response_cookies(self, test_server_dynamic_port: str) -> None:
        """Test response cookies property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/cookies/set?test_cookie=test_value")
        
        cookies = response.cookies
        assert cookies is not None


class TestResponseTextMarkdown:
    """Tests for Response.text_markdown property."""
    
    def test_sync_client_response_text_markdown(self, test_server_dynamic_port: str) -> None:
        """Test response text_markdown property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        # text_markdown should convert HTML to markdown
        markdown = response.text_markdown
        assert markdown is not None
        assert isinstance(markdown, str)
    
    @pytest.mark.asyncio
    async def test_async_client_response_text_markdown(self, test_server_dynamic_port: str) -> None:
        """Test response text_markdown property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        markdown = response.text_markdown
        assert markdown is not None
        assert isinstance(markdown, str)


class TestResponseTextPlain:
    """Tests for Response.text_plain property."""
    
    def test_sync_client_response_text_plain(self, test_server_dynamic_port: str) -> None:
        """Test response text_plain property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        # text_plain should extract plain text from HTML
        plain = response.text_plain
        assert plain is not None
        assert isinstance(plain, str)
    
    @pytest.mark.asyncio
    async def test_async_client_response_text_plain(self, test_server_dynamic_port: str) -> None:
        """Test response text_plain property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        plain = response.text_plain
        assert plain is not None
        assert isinstance(plain, str)


class TestResponseTextRich:
    """Tests for Response.text_rich property."""
    
    def test_sync_client_response_text_rich(self, test_server_dynamic_port: str) -> None:
        """Test response text_rich property on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        # text_rich should convert HTML to rich text
        rich = response.text_rich
        assert rich is not None
    
    @pytest.mark.asyncio
    async def test_async_client_response_text_rich(self, test_server_dynamic_port: str) -> None:
        """Test response text_rich property on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/html")
        
        assert response.status_code == 200
        rich = response.text_rich
        assert rich is not None


class TestResponseJson:
    """Tests for Response.json() method."""
    
    def test_sync_client_response_json(self, test_server_dynamic_port: str) -> None:
        """Test response json() method on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        
        assert isinstance(data, dict)
        assert "method" in data
        assert data["method"] == "GET"
    
    @pytest.mark.asyncio
    async def test_async_client_response_json(self, test_server_dynamic_port: str) -> None:
        """Test response json() method on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        data = response.json()
        
        assert isinstance(data, dict)
        assert "method" in data
        assert data["method"] == "GET"
    
    def test_sync_client_response_json_nested(self, test_server_dynamic_port: str) -> None:
        """Test response json() method with nested data."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        json_data = {
            "level1": {
                "level2": {
                    "level3": "deep_value"
                }
            },
            "array": [1, 2, 3]
        }
        response = client.post(f"{base_url}/post", json=json_data)
        
        assert response.status_code == 200
        data = response.json()
        
        assert data["json"]["level1"]["level2"]["level3"] == "deep_value"
        assert data["json"]["array"] == [1, 2, 3]


class TestResponseRaiseForStatus:
    """Tests for Response.raise_for_status() method."""
    
    def test_sync_client_raise_for_status_success(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() does not raise for successful status."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/get")
        
        # Should not raise
        response.raise_for_status()
    
    @pytest.mark.asyncio
    async def test_async_client_raise_for_status_success(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() does not raise for successful status."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get")
        
        # Should not raise
        response.raise_for_status()
    
    def test_sync_client_raise_for_status_404(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() raises for 404 status."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/status/404")
        
        assert response.status_code == 404
        
        with pytest.raises(Exception):
            response.raise_for_status()
    
    @pytest.mark.asyncio
    async def test_async_client_raise_for_status_404(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() raises for 404 status."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/status/404")
        
        assert response.status_code == 404
        
        with pytest.raises(Exception):
            response.raise_for_status()
    
    def test_sync_client_raise_for_status_500(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() raises for 500 status."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/status/500")
        
        assert response.status_code == 500
        
        with pytest.raises(Exception):
            response.raise_for_status()
    
    @pytest.mark.asyncio
    async def test_async_client_raise_for_status_500(self, test_server_dynamic_port: str) -> None:
        """Test raise_for_status() raises for 500 status."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/status/500")
        
        assert response.status_code == 500
        
        with pytest.raises(Exception):
            response.raise_for_status()


class TestResponseStatusCodeRanges:
    """Tests for various HTTP status code ranges."""
    
    @pytest.mark.parametrize("status_code", [200, 201, 202, 204, 301, 302, 400, 401, 403, 404, 500, 502, 503])
    def test_sync_client_status_codes(self, test_server_dynamic_port: str, status_code: int) -> None:
        """Test various status codes on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        response = client.get(f"{base_url}/status/{status_code}")
        
        assert response.status_code == status_code
    
    @pytest.mark.parametrize("status_code", [200, 201, 202, 204, 301, 302, 400, 401, 403, 404, 500, 502, 503])
    @pytest.mark.asyncio
    async def test_async_client_status_codes(self, test_server_dynamic_port: str, status_code: int) -> None:
        """Test various status codes on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/status/{status_code}")
        
        assert response.status_code == status_code
