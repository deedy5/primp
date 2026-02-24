"""
Tests for streaming response methods.

This module tests:
- read(): Read all content
- iter_bytes(): Iterate by bytes
- iter_text(): Iterate by text
- iter_lines(): Iterate by lines
- Context manager for streaming
- Manual close for streaming
- Both sync and async streaming
"""

import json

import pytest

import primp


class TestSyncStreamingRead:
    """Tests for streaming read() method on sync Client."""
    
    def test_sync_client_streaming_read(self, test_server_dynamic_port: str) -> None:
        """Test streaming with read() method on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            content = response.read()
            
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0


class TestSyncStreamingIterBytes:
    """Tests for streaming iter_bytes() method on sync Client."""
    
    def test_sync_client_streaming_iter_bytes(self, test_server_dynamic_port: str) -> None:
        """Test streaming with iter_bytes() method on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            for chunk in response.iter_bytes():
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)


class TestSyncStreamingIterText:
    """Tests for streaming iter_text() method on sync Client."""
    
    def test_sync_client_streaming_iter_text(self, test_server_dynamic_port: str) -> None:
        """Test streaming with iter_text() method on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            text_chunks = []
            for chunk in response.iter_text():
                text_chunks.append(chunk)
            
            assert len(text_chunks) > 0
            for chunk in text_chunks:
                assert isinstance(chunk, str)


class TestSyncStreamingIterLines:
    """Tests for streaming iter_lines() method on sync Client."""
    
    def test_sync_client_streaming_iter_lines(self, test_server_dynamic_port: str) -> None:
        """Test streaming with iter_lines() method on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            lines = []
            for line in response.iter_lines():
                lines.append(line)
            
            assert len(lines) == 3
            for i, line in enumerate(lines):
                data = json.loads(line)
                assert data["id"] == i
                assert "message" in data
    
    def test_sync_client_streaming_iter_lines_partial(self, test_server_dynamic_port: str) -> None:
        """Test streaming with iter_lines() reading partial data."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            lines = []
            for line in response.iter_lines():
                lines.append(line)
                if len(lines) >= 2:
                    break  # Only read 2 of 5 lines
            
            assert len(lines) == 2


class TestSyncStreamingContextManager:
    """Tests for streaming context manager on sync Client."""
    
    def test_sync_client_streaming_context_manager(self, test_server_dynamic_port: str) -> None:
        """Test streaming with context manager on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        # Using context manager
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            lines = list(response.iter_lines())
            assert len(lines) == 3


class TestSyncStreamingManualClose:
    """Tests for streaming with manual close on sync Client."""
    
    def test_sync_client_streaming_manual_close(self, test_server_dynamic_port: str) -> None:
        """Test streaming with manual close on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        response = client.get(f"{base_url}/stream/3", stream=True)
        
        try:
            assert response.status_code == 200
            
            lines = []
            for line in response.iter_lines():
                lines.append(line)
                if len(lines) >= 2:
                    break  # Only read 2 of 3 lines
            
            assert len(lines) == 2
        finally:
            response.close()
    
    def test_sync_client_streaming_manual_close_after_full_read(self, test_server_dynamic_port: str) -> None:
        """Test manual close after reading all content."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        response = client.get(f"{base_url}/stream/2", stream=True)
        
        try:
            assert response.status_code == 200
            lines = list(response.iter_lines())
            assert len(lines) == 2
        finally:
            response.close()


class TestAsyncStreamingRead:
    """Tests for streaming read() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_read(self, test_server_dynamic_port: str) -> None:
        """Test streaming with aread() method on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            content = await response.aread()
            
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0


class TestAsyncStreamingIterBytes:
    """Tests for streaming aiter_bytes() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_bytes(self, test_server_dynamic_port: str) -> None:
        """Test streaming with aiter_bytes() method on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            async for chunk in response.aiter_bytes():
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)


class TestAsyncStreamingIterText:
    """Tests for streaming aiter_text() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_text(self, test_server_dynamic_port: str) -> None:
        """Test streaming with aiter_text() method on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            text_chunks = []
            async for chunk in response.aiter_text():
                text_chunks.append(chunk)
            
            assert len(text_chunks) > 0
            for chunk in text_chunks:
                assert isinstance(chunk, str)


class TestAsyncStreamingIterLines:
    """Tests for streaming aiter_lines() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_lines(self, test_server_dynamic_port: str) -> None:
        """Test streaming with aiter_lines() method on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
            
            assert len(lines) == 3
            for i, line in enumerate(lines):
                data = json.loads(line)
                assert data["id"] == i
                assert "message" in data
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_lines_partial(self, test_server_dynamic_port: str) -> None:
        """Test streaming with aiter_lines() reading partial data."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
                if len(lines) >= 2:
                    break  # Only read 2 of 5 lines
            
            assert len(lines) == 2


class TestAsyncStreamingContextManager:
    """Tests for streaming context manager on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_context_manager(self, test_server_dynamic_port: str) -> None:
        """Test streaming with context manager on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        # Using async context manager
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
            assert len(lines) == 3


class TestAsyncStreamingManualClose:
    """Tests for streaming with manual close on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_manual_close(self, test_server_dynamic_port: str) -> None:
        """Test streaming with manual close on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        response = await client.get(f"{base_url}/stream/3", stream=True)
        
        try:
            assert response.status_code == 200
            
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
                if len(lines) >= 2:
                    break  # Only read 2 of 3 lines
            
            assert len(lines) == 2
        finally:
            await response.aclose()
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_manual_close_after_full_read(self, test_server_dynamic_port: str) -> None:
        """Test manual close after reading all content."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        response = await client.get(f"{base_url}/stream/2", stream=True)
        
        try:
            assert response.status_code == 200
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
            assert len(lines) == 2
        finally:
            await response.aclose()


class TestStreamingLargeResponse:
    """Tests for streaming large responses."""
    
    def test_sync_client_streaming_large(self, test_server_dynamic_port: str) -> None:
        """Test streaming a larger response on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/10", stream=True) as response:
            assert response.status_code == 200
            
            lines = list(response.iter_lines())
            
            assert len(lines) == 10
            for i, line in enumerate(lines):
                data = json.loads(line)
                assert data["id"] == i
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_large(self, test_server_dynamic_port: str) -> None:
        """Test streaming a larger response on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/10", stream=True) as response:
            assert response.status_code == 200
            
            lines = []
            async for line in response.aiter_lines():
                lines.append(line)
            
            assert len(lines) == 10
            for i, line in enumerate(lines):
                data = json.loads(line)
                assert data["id"] == i


class TestStreamingNonStreamingFallback:
    """Tests for non-streaming requests (stream=False or default)."""
    
    def test_sync_client_non_streaming(self, test_server_dynamic_port: str) -> None:
        """Test non-streaming request on sync Client."""
        base_url = test_server_dynamic_port
        
        client = primp.Client()
        
        # Default behavior (no stream parameter)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        # Content should be immediately available
        assert response.content is not None
        data = response.json()
        assert data["method"] == "GET"
    
    @pytest.mark.asyncio
    async def test_async_client_non_streaming(self, test_server_dynamic_port: str) -> None:
        """Test non-streaming request on AsyncClient."""
        base_url = test_server_dynamic_port
        
        client = primp.AsyncClient()
        
        # Default behavior (no stream parameter)
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        # Content should be immediately available
        assert response.content is not None
        data = response.json()
        assert data["method"] == "GET"
