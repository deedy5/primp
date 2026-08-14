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

    def test_sync_iter_bytes_stopiteration_is_idempotent(self, test_server: str) -> None:
        """Regression: calling next() on an iter_bytes iterator repeatedly after
        exhaustion must raise StopIteration every time (not PrimpError).

        The underlying Response is exhausted after the first full drain; a
        BytesIterator without a terminal guard would re-poll the exhausted
        Response and surface a stream_exhausted -> PrimpError instead of a
        clean StopIteration.
        """
        base_url = test_server

        client = primp.Client()

        with client.get(f"{base_url}/stream/3", stream=True) as response:
            it = response.iter_bytes()
            # Drain fully.
            for _ in it:
                pass
            # Every subsequent next() must raise StopIteration, idempotently.
            for _ in range(3):
                with pytest.raises(StopIteration):
                    next(it)

    def test_sync_iter_bytes_partial_buffer_at_eof_is_flushed_and_then_stops(
        self, test_server: str
    ) -> None:
        """A final partial chunk (body not a multiple of chunk_size) must be
        flushed, then StopIteration — not stream_exhausted."""
        base_url = test_server

        client = primp.Client()
        reference = client.get(f"{base_url}/ip")
        body = reference.content
        assert len(body) > 10

        # Guarantee a partial buffer at EOF: body length not a multiple of
        # chunk_size.
        chunk_size = len(body) - 3
        with client.get(f"{base_url}/ip", stream=True) as response:
            chunks = list(response.iter_bytes(chunk_size=chunk_size))

        assert b"".join(chunks) == body

    def test_sync_iterators_after_iterator_consumed_body_raise_stop_iteration(
        self, test_server: str
    ) -> None:
        """A new iterator after a full iterator drain must stop on first
        next() — the exhausted flag is shared with iterators."""
        base_url = test_server

        client = primp.Client()
        with client.get(f"{base_url}/ip", stream=True) as response:
            assert list(response.iter_bytes(chunk_size=4)) != []
            # The body is gone; every fresh iterator must end quietly.
            assert list(response.iter_bytes()) == []
            assert list(response.iter_text()) == []
            assert list(response.iter_lines()) == []

    def test_sync_iter_bytes_partial_buffer_is_flushed_after_external_drain(
        self, test_server: str
    ) -> None:
        """Buffered tail is flushed, not dropped, when an external drain exhausts the body."""
        base_url = test_server

        client = primp.Client()
        reference = client.get(f"{base_url}/ip")
        body = reference.content
        assert len(body) > 10

        chunk_size = len(body) - 3
        with client.get(f"{base_url}/ip", stream=True) as response:
            it = response.iter_bytes(chunk_size=chunk_size)
            first = next(it)
            assert len(first) == chunk_size
            # External drain by a sibling iterator: the core was already
            # polled to the end by `it`, so the sibling hits EOF and sets
            # the shared exhausted flag.
            assert list(response.iter_bytes()) == []
            # The tail buffered in the iterator must be flushed, not dropped.
            tail = next(it)
            assert tail == body[chunk_size:]
            with pytest.raises(StopIteration):
                next(it)

    def test_sync_iter_text_partial_buffer_at_eof(self, test_server: str) -> None:
        """iter_text() with chunk_size larger than the body must flush the
        final partial chunk and then stop cleanly."""
        base_url = test_server

        client = primp.Client()
        with client.get(f"{base_url}/ip", stream=True) as response:
            chunks = list(response.iter_text(chunk_size=100))
        assert "".join(chunks) == client.get(f"{base_url}/ip").text


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


class TestSyncStreamingIterTextUtf8Boundary:
    """iter_text() must respect multi-byte UTF-8 boundaries across chunks."""

    def test_sync_iter_text_multibyte_boundary(self, test_server: str) -> None:
        """Sub-character HTTP chunking must not produce U+FFFD."""
        base_url = test_server
        client = primp.Client()
        text = "中文测试abc"

        with client.get(f"{base_url}/utf8-stream?bpc=2", stream=True) as response:
            assert response.status_code == 200
            chunks = list(response.iter_text())

        joined = "".join(chunks)
        assert "\ufffd" not in joined, f"replacement chars emitted: {chunks!r}"
        assert joined == text

    def test_sync_iter_text_multibyte_single_char(self, test_server: str) -> None:
        """A single CJK character split byte-by-byte must decode."""
        base_url = test_server
        client = primp.Client()
        text = "中"

        with client.get(f"{base_url}/utf8-stream?text=%E4%B8%AD&bpc=1", stream=True) as response:
            assert response.status_code == 200
            chunks = list(response.iter_text())

        joined = "".join(chunks)
        assert "\ufffd" not in joined
        assert joined == text


class TestAsyncStreamingIterTextUtf8Boundary:
    """aiter_text() must respect multi-byte UTF-8 boundaries across chunks."""

    @pytest.mark.asyncio
    async def test_async_aiter_text_multibyte_boundary(self, test_server: str) -> None:
        """Async version of the multi-byte boundary regression test."""
        base_url = test_server
        client = primp.AsyncClient()
        text = "中文测试abc"

        async with await client.get(f"{base_url}/utf8-stream?bpc=2", stream=True) as response:
            assert response.status_code == 200
            chunks = []
            async for chunk in response.aiter_text():
                chunks.append(chunk)

        joined = "".join(chunks)
        assert "\ufffd" not in joined, f"replacement chars emitted: {chunks!r}"
        assert joined == text


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

    def test_sync_client_streaming_next_after_exhaustion(self, test_server: str) -> None:
        """Regression: next() must keep returning None after EOF, not raise.

        primp's underlying stream returns a ``stream_exhausted`` error on any
        read after the first end-of-stream, so the binding must remember that
        the body is exhausted and return ``None`` instead of surfacing an error.
        """
        base_url = test_server

        client = primp.Client()
        response = client.get(f"{base_url}/stream/2", stream=True)
        try:
            assert response.status_code == 200

            # Drain the stream.
            while response.next() is not None:
                pass

            # Calls after exhaustion must keep returning None, not raise.
            assert response.next() is None
            assert response.next() is None
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

    @pytest.mark.asyncio
    async def test_async_aiter_bytes_stopasynciteration_is_idempotent(
        self, test_server: str
    ) -> None:
        """Regression: repeated __anext__ after exhaustion must raise
        StopAsyncIteration every time (not PrimpError)."""
        base_url = test_server

        client = primp.AsyncClient()

        async with await client.get(f"{base_url}/stream/3", stream=True) as response:
            it = response.aiter_bytes()
            async for _ in it:
                pass
            for _ in range(3):
                with pytest.raises(StopAsyncIteration):
                    await it.__anext__()

    @pytest.mark.asyncio
    async def test_async_aiter_bytes_partial_buffer_at_eof_is_flushed_and_then_stops(
        self, test_server: str
    ) -> None:
        """Async mirror: a final partial chunk must be flushed, then
        StopAsyncIteration."""
        base_url = test_server

        client = primp.AsyncClient()
        reference = await client.get(f"{base_url}/ip")
        body = await reference.aread()
        assert len(body) > 10

        chunk_size = len(body) - 3
        async with await client.get(f"{base_url}/ip", stream=True) as response:
            chunks = [c async for c in response.aiter_bytes(chunk_size=chunk_size)]

        assert b"".join(chunks) == body

    @pytest.mark.asyncio
    async def test_async_aiter_bytes_partial_buffer_is_flushed_after_external_drain(
        self, test_server: str
    ) -> None:
        """Buffered tail is flushed, not dropped, when an external drain exhausts the body."""
        base_url = test_server

        client = primp.AsyncClient()
        reference = await client.get(f"{base_url}/ip")
        body = await reference.aread()
        assert len(body) > 10

        chunk_size = len(body) - 3
        async with await client.get(f"{base_url}/ip", stream=True) as response:
            it = response.aiter_bytes(chunk_size=chunk_size)
            first = await it.__anext__()
            assert len(first) == chunk_size
            # External drain by a sibling iterator: the core was already
            # polled to the end by `it`, so the sibling hits EOF and sets
            # the shared exhausted flag.
            assert [c async for c in response.aiter_bytes()] == []
            # The tail buffered in the iterator must be flushed, not dropped.
            tail = await it.__anext__()
            assert tail == body[chunk_size:]
            with pytest.raises(StopAsyncIteration):
                await it.__anext__()

    @pytest.mark.asyncio
    async def test_async_aiterators_after_iterator_consumed_body_raise_stop_async_iteration(
        self, test_server: str
    ) -> None:
        """A new iterator after a full iterator drain must stop on first
        __anext__ — the exhausted flag is shared with iterators."""
        base_url = test_server

        client = primp.AsyncClient()
        async with await client.get(f"{base_url}/ip", stream=True) as response:
            assert [c async for c in response.aiter_bytes(chunk_size=4)] != []
            assert [c async for c in response.aiter_bytes()] == []
            assert [t async for t in response.aiter_text()] == []
            assert [l async for l in response.aiter_lines()] == []


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


class TestIterLinesCharset:
    """iter_lines/aiter_lines must honor the Content-Type charset (finding A2)."""

    def test_sync_iter_lines_honors_charset(self, test_server: str) -> None:
        """iter_lines decodes a latin-1 body with the declared charset."""
        client = primp.Client()
        with client.get(f"{test_server}/latin1", stream=True) as response:
            lines = list(response.iter_lines())
        assert lines == ["café", "naïve line"]

    @pytest.mark.asyncio
    async def test_async_aiter_lines_honors_charset(self, test_server: str) -> None:
        """aiter_lines decodes a latin-1 body with the declared charset."""
        client = primp.AsyncClient()
        async with await client.get(f"{test_server}/latin1", stream=True) as response:
            lines = [line async for line in response.aiter_lines()]
        assert lines == ["café", "naïve line"]


class TestPostDrainIterators:
    """Iterators created after a full-body drain must raise StopIteration on
    the first next(), not PrimpError/stream_exhausted (finding A3)."""

    def test_sync_iterators_after_full_drain_raise_stop_iteration(self, test_server: str) -> None:
        """iter_bytes/iter_text/iter_lines after text() drain end quietly."""
        client = primp.Client()
        response = client.get(f"{test_server}/get")
        response.text
        assert list(response.iter_bytes()) == []
        assert list(response.iter_text()) == []
        assert list(response.iter_lines()) == []

    @pytest.mark.asyncio
    async def test_async_iterators_after_full_drain_raise_stop_async_iteration(
        self, test_server: str
    ) -> None:
        """aiter_bytes/aiter_text/aiter_lines after text() drain end quietly."""
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/get")
        response.text
        assert [c async for c in response.aiter_bytes()] == []
        assert [t async for t in response.aiter_text()] == []
        assert [l async for l in response.aiter_lines()] == []


class TestPreDrainIterators:
    """Iterators created BEFORE a drain must raise Stop* when used AFTER the
    drain by another consumer, never re-poll the exhausted core into a
    PrimpError (finding F8)."""

    def test_sync_bytes_iterator_stale_after_sibling_drain(self, test_server: str) -> None:
        """iter_bytes created first, sibling iterator drains, first stays quiet."""
        client = primp.Client()
        response = client.get(f"{test_server}/stream/3")
        stale = response.iter_bytes()
        other = response.iter_bytes()
        drained = list(other)
        assert len(drained) > 0
        assert list(stale) == []

    def test_sync_text_iterator_stale_after_sibling_drain(self, test_server: str) -> None:
        """iter_text created first, sibling iterator drains, first stays quiet."""
        client = primp.Client()
        response = client.get(f"{test_server}/stream/3")
        stale = response.iter_text()
        other = response.iter_text()
        assert len(list(other)) > 0
        assert list(stale) == []

    def test_sync_bytes_iterator_stale_after_content_drain(self, test_server: str) -> None:
        """iter_bytes created, then content drains, iterator stays quiet."""
        client = primp.Client()
        response = client.get(f"{test_server}/stream/3")
        stale = response.iter_bytes()
        assert response.content
        assert list(stale) == []

    @pytest.mark.asyncio
    async def test_async_bytes_iterator_stale_after_sibling_drain(
        self, test_server: str
    ) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/stream/3")
        stale = response.aiter_bytes()
        other = response.aiter_bytes()
        drained = [c async for c in other]
        assert len(drained) > 0
        assert [c async for c in stale] == []

    @pytest.mark.asyncio
    async def test_async_text_iterator_stale_after_sibling_drain(
        self, test_server: str
    ) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/stream/3")
        stale = response.aiter_text()
        other = response.aiter_text()
        assert len([t async for t in other]) > 0
        assert [t async for t in stale] == []

    @pytest.mark.asyncio
    async def test_async_bytes_iterator_stale_after_content_drain(
        self, test_server: str
    ) -> None:
        client = primp.AsyncClient()
        response = await client.get(f"{test_server}/stream/3")
        stale = response.aiter_bytes()
        assert response.content
        assert [c async for c in stale] == []

    @pytest.mark.asyncio
    async def test_async_aiter_bit_and_concurrent_aread_does_not_surface_stream_error(
        self, test_server: str
    ) -> None:
        """An iterator created BEFORE a slow drain must not surface
        `stream_exhausted` as a PrimpError when it wakes after the drain
        exhausted the core (TOCTOU: iterator loads the shared flag, then
        polls after the drain set it)."""
        import asyncio

        client = primp.AsyncClient()
        response = await client.get(
            f"{test_server}/stream-delay?text=abcdefghi&bpc=3&delay=0.3",
            stream=True,
        )
        it = response.aiter_bytes(chunk_size=2)
        # Drain stays mid-flight (holding `resp.lock()`, blocked on the slow
        # body) when the iterator's first poll runs.
        drain = asyncio.create_task(response.aread())
        await asyncio.sleep(0.1)
        anext_task = asyncio.create_task(it.__anext__())
        # The drain finishes, exhausting the core and setting the shared
        # ``exhausted`` after the iterator's earlier load.
        await drain
        try:
            result = await asyncio.wait_for(anext_task, timeout=2.0)
            # A partial chunk is fine; never a PrimpError from re-polling.
            assert isinstance(result, bytes)
        except primp.PrimpError:
            raise AssertionError(
                "iterator surfaced PrimpError after concurrent external drain"
                "instead of stopping quietly"
            )
        except StopAsyncIteration:
            pass


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

    @pytest.mark.asyncio
    async def test_async_aread_after_aclose_raises_body_error(self, test_server: str) -> None:
        """aread() after aclose() must raise BodyError like sync read() after
        close(), not silently return b''."""
        base_url = test_server

        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/stream/2", stream=True)
        await response.aclose()
        with pytest.raises(primp.BodyError):
            await response.aread()

    def test_sync_read_after_close_raises_body_error(self, test_server: str) -> None:
        """Baseline: sync read() after close() raises BodyError."""
        base_url = test_server

        client = primp.Client()
        response = client.get(f"{base_url}/stream/2", stream=True)
        response.close()
        with pytest.raises(primp.BodyError):
            response.read()


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

    @pytest.mark.asyncio
    async def test_async_client_streaming_anext_after_exhaustion(self, test_server: str) -> None:
        """Regression: anext() must keep returning None after EOF, not raise."""
        base_url = test_server

        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/stream/2", stream=True)
        try:
            assert response.status_code == 200

            # Drain the stream.
            while await response.anext() is not None:
                pass

            # Calls after exhaustion must keep returning None, not raise.
            assert await response.anext() is None
            assert await response.anext() is None
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
