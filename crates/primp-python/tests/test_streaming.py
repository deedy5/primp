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
    
    def test_sync_client_streaming_read(self, test_server: str) -> None:
        """Test streaming with read() method on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            content = response.read()
            
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0


class TestSyncStreamingIterBytes:
    """Tests for streaming iter_bytes() method on sync Client."""
    
    def test_sync_client_streaming_iter_bytes(self, test_server: str) -> None:
        """Test streaming with iter_bytes() method on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            for chunk in response.iter_bytes():
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
    
    def test_sync_client_streaming_iter_bytes_chunk_size(self, test_server: str) -> None:
        """Test streaming with iter_bytes() with custom chunk_size."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            chunks = list(response.iter_bytes(chunk_size=10))
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
                assert len(chunk) <= 10


class TestSyncStreamingIterText:
    """Tests for streaming iter_text() method on sync Client."""
    
    def test_sync_client_streaming_iter_text(self, test_server: str) -> None:
        """Test streaming with iter_text() method on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            text_chunks = []
            for chunk in response.iter_text():
                text_chunks.append(chunk)
            
            assert len(text_chunks) > 0
            for chunk in text_chunks:
                assert isinstance(chunk, str)
    
    def test_sync_client_streaming_iter_text_chunk_size(self, test_server: str) -> None:
        """Test streaming with iter_text() with custom chunk_size."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            chunks = list(response.iter_text(chunk_size=10))
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, str)


class TestSyncStreamingIterLines:
    """Tests for streaming iter_lines() method on sync Client."""
    
    def test_sync_client_streaming_iter_lines(self, test_server: str) -> None:
        """Test streaming with iter_lines() method on sync Client."""
        base_url = test_server
        
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
    
    def test_sync_client_streaming_iter_lines_partial(self, test_server: str) -> None:
        """Test streaming with iter_lines() reading partial data."""
        base_url = test_server
        
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
    
    def test_sync_client_streaming_context_manager(self, test_server: str) -> None:
        """Test streaming with context manager on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        # Using context manager
        with client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            lines = list(response.iter_lines())
            assert len(lines) == 3


class TestSyncStreamingManualClose:
    """Tests for streaming with manual close on sync Client."""
    
    def test_sync_client_streaming_manual_close(self, test_server: str) -> None:
        """Test streaming with manual close on sync Client."""
        base_url = test_server
        
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
    
    def test_sync_client_streaming_manual_close_after_full_read(self, test_server: str) -> None:
        """Test manual close after reading all content."""
        base_url = test_server
        
        client = primp.Client()
        
        response = client.get(f"{base_url}/stream/2", stream=True)
        
        try:
            assert response.status_code == 200
            lines = list(response.iter_lines())
            assert len(lines) == 2
        finally:
            response.close()


class TestSyncStreamingNext:
    """Tests for streaming next() method on sync Client."""

    def test_sync_client_streaming_next(self, test_server: str) -> None:
        """Test streaming with next() method on sync Client."""
        base_url = test_server

        client = primp.Client()

        response = client.get(f"{base_url}/stream/3", stream=True)

        try:
            assert response.status_code == 200

            chunks = []
            while True:
                chunk = response.next()
                if chunk is None:
                    break
                chunks.append(chunk)

            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
        finally:
            response.close()


class TestAsyncStreamingRead:
    """Tests for streaming read() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_read(self, test_server: str) -> None:
        """Test streaming with aread() method on AsyncClient."""
        base_url = test_server
        
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
    async def test_async_client_streaming_aiter_bytes(self, test_server: str) -> None:
        """Test streaming with aiter_bytes() method on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            async for chunk in response.aiter_bytes():
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_bytes_chunk_size(self, test_server: str) -> None:
        """Test streaming with aiter_bytes() with custom chunk_size."""
        base_url = test_server
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            async for chunk in response.aiter_bytes(chunk_size=10):
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
                assert len(chunk) <= 10


class TestAsyncStreamingIterText:
    """Tests for streaming aiter_text() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_text(self, test_server: str) -> None:
        """Test streaming with aiter_text() method on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            assert response.status_code == 200
            
            text_chunks = []
            async for chunk in response.aiter_text():
                text_chunks.append(chunk)
            
            assert len(text_chunks) > 0
            for chunk in text_chunks:
                assert isinstance(chunk, str)
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_text_chunk_size(self, test_server: str) -> None:
        """Test streaming with aiter_text() with custom chunk_size."""
        base_url = test_server
        
        client = primp.AsyncClient()
        
        async with await client.get(f"{base_url}/stream/5", stream=True) as response:
            assert response.status_code == 200
            
            chunks = []
            async for chunk in response.aiter_text(chunk_size=10):
                chunks.append(chunk)
            
            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, str)


class TestAsyncStreamingIterLines:
    """Tests for streaming aiter_lines() method on AsyncClient."""
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_aiter_lines(self, test_server: str) -> None:
        """Test streaming with aiter_lines() method on AsyncClient."""
        base_url = test_server
        
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
    async def test_async_client_streaming_aiter_lines_partial(self, test_server: str) -> None:
        """Test streaming with aiter_lines() reading partial data."""
        base_url = test_server
        
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
    async def test_async_client_streaming_context_manager(self, test_server: str) -> None:
        """Test streaming with context manager on AsyncClient."""
        base_url = test_server
        
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
    async def test_async_client_streaming_manual_close(self, test_server: str) -> None:
        """Test streaming with manual close on AsyncClient."""
        base_url = test_server
        
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
    async def test_async_client_streaming_manual_close_after_full_read(self, test_server: str) -> None:
        """Test manual close after reading all content."""
        base_url = test_server
        
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


class TestAsyncStreamingAnext:
    """Tests for streaming anext() method on AsyncClient."""

    @pytest.mark.asyncio
    async def test_async_client_streaming_anext(self, test_server: str) -> None:
        """Test streaming with anext() method on AsyncClient."""
        base_url = test_server

        client = primp.AsyncClient()

        response = await client.get(f"{base_url}/stream/3", stream=True)

        try:
            assert response.status_code == 200

            chunks = []
            while True:
                chunk = await response.anext()
                if chunk is None:
                    break
                chunks.append(chunk)

            assert len(chunks) > 0
            for chunk in chunks:
                assert isinstance(chunk, bytes)
        finally:
            await response.aclose()


class TestStreamingLargeResponse:
    """Tests for streaming large responses."""
    
    def test_sync_client_streaming_large(self, test_server: str) -> None:
        """Test streaming a larger response on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        with client.get(f"{base_url}/stream/10", stream=True) as response:
            assert response.status_code == 200
            
            lines = list(response.iter_lines())
            
            assert len(lines) == 10
            for i, line in enumerate(lines):
                data = json.loads(line)
                assert data["id"] == i
    
    @pytest.mark.asyncio
    async def test_async_client_streaming_large(self, test_server: str) -> None:
        """Test streaming a larger response on AsyncClient."""
        base_url = test_server
        
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
    
    def test_sync_client_non_streaming(self, test_server: str) -> None:
        """Test non-streaming request on sync Client."""
        base_url = test_server
        
        client = primp.Client()
        
        # Default behavior (no stream parameter)
        response = client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        # Content should be immediately available
        assert response.content is not None
        data = response.json()
        assert data["method"] == "GET"
    
    @pytest.mark.asyncio
    async def test_async_client_non_streaming(self, test_server: str) -> None:
        """Test non-streaming request on AsyncClient."""
        base_url = test_server
        
        client = primp.AsyncClient()
        
        # Default behavior (no stream parameter)
        response = await client.get(f"{base_url}/get")
        
        assert response.status_code == 200
        # Content should be immediately available
        assert response.content is not None
        data = response.json()
        assert data["method"] == "GET"


class TestSyncResponseStreaming:
    """Tests for Response properties on sync Client."""

    def test_stream_response_url(self, test_server: str) -> None:
        """Test Response.url property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.url is not None
            assert "/stream/1" in response.url

    def test_stream_response_status_code(self, test_server: str) -> None:
        """Test Response.status_code property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.status_code == 200

    def test_stream_response_headers(self, test_server: str) -> None:
        """Test Response.headers property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.headers is not None
            assert isinstance(response.headers, dict)

    def test_stream_response_cookies(self, test_server: str) -> None:
        """Test Response.cookies property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/cookies/set?stream_cookie=val", stream=True) as response:
            assert response.cookies is not None
            assert isinstance(response.cookies, dict)

    def test_stream_response_encoding(self, test_server: str) -> None:
        """Test Response.encoding property (getter and setter)."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/1", stream=True) as response:
            if response.encoding is not None:
                assert isinstance(response.encoding, str)
            response.encoding = "utf-8"
            assert response.encoding == "utf-8"

    def test_stream_response_content(self, test_server: str) -> None:
        """Test Response.content property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/2", stream=True) as response:
            content = response.content
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0

    def test_stream_response_text(self, test_server: str) -> None:
        """Test Response.text property."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/2", stream=True) as response:
            text = response.text
            assert text is not None
            assert isinstance(text, str)
            assert len(text) > 0

    def test_stream_response_raise_for_status_success(self, test_server: str) -> None:
        """Test Response.raise_for_status() for success."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/stream/1", stream=True) as response:
            response.raise_for_status()  # Should not raise

    def test_stream_response_raise_for_status_error(self, test_server: str) -> None:
        """Test Response.raise_for_status() for error status."""
        base_url = test_server
        client = primp.Client()

        with client.get(f"{base_url}/status/404", stream=True) as response:
            with pytest.raises(Exception):
                response.raise_for_status()


class TestAsyncResponseStreaming:
    """Tests for AsyncResponse properties on AsyncClient."""

    @pytest.mark.asyncio
    async def test_async_stream_response_url(self, test_server: str) -> None:
        """Test AsyncResponse.url property."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.url is not None
            assert "/stream/1" in response.url

    @pytest.mark.asyncio
    async def test_async_stream_response_status_code(self, test_server: str) -> None:
        """Test AsyncResponse.status_code property."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_stream_response_headers(self, test_server: str) -> None:
        """Test AsyncResponse.headers property."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/1", stream=True) as response:
            assert response.headers is not None
            assert isinstance(response.headers, dict)

    @pytest.mark.asyncio
    async def test_async_stream_response_cookies(self, test_server: str) -> None:
        """Test AsyncResponse.cookies property."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/cookies/set?astream_cookie=val", stream=True) as response:
            assert response.cookies is not None
            assert isinstance(response.cookies, dict)

    @pytest.mark.asyncio
    async def test_async_stream_response_encoding(self, test_server: str) -> None:
        """Test AsyncResponse.encoding property (getter and setter)."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/1", stream=True) as response:
            if response.encoding is not None:
                assert isinstance(response.encoding, str)
            response.encoding = "utf-8"
            assert response.encoding == "utf-8"

    @pytest.mark.asyncio
    async def test_async_stream_response_content(self, test_server: str) -> None:
        """Test AsyncResponse.content property."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/2", stream=True) as response:
            content = await response.aread()
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0

    @pytest.mark.asyncio
    async def test_async_stream_response_text(self, test_server: str) -> None:
        """Test AsyncResponse.text property via aread()."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/2", stream=True) as response:
            content = await response.aread()
            assert content is not None
            assert isinstance(content, bytes)
            assert len(content) > 0

    @pytest.mark.asyncio
    async def test_async_stream_response_raise_for_status_success(self, test_server: str) -> None:
        """Test AsyncResponse.raise_for_status() for success."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/1", stream=True) as response:
            response.raise_for_status()  # Should not raise

    @pytest.mark.asyncio
    async def test_async_stream_response_raise_for_status_error(self, test_server: str) -> None:
        """Test AsyncResponse.raise_for_status() for error status."""
        base_url = test_server
        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/status/404", stream=True) as response:
            with pytest.raises(Exception):
                response.raise_for_status()
