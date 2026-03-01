"""
Shared pytest fixtures and configuration for primp tests.

This module provides common fixtures used across all test modules:
- sync_client: Synchronous Client fixture (function-scoped)
- sync_client_session: Synchronous Client fixture (session-scoped)
- async_client: Async client fixture (function-scoped)
- async_client_session: Async client fixture (session-scoped)
- test_server: Test server fixture from server.py (session-scoped)
- test_server_dynamic_port: Test server fixture from server.py (function-scoped)
"""

import pytest
import primp
from tests.server import test_server, test_server_dynamic_port


__all__ = [
    "sync_client",
    "sync_client_session",
    "async_client",
    "async_client_session",
    "test_server",
    "test_server_dynamic_port",
]


# Function-scoped fixtures (for tests that need fresh clients)
@pytest.fixture
def sync_client() -> primp.Client:
    """Function-scoped synchronous Client fixture.
    
    Use this when test isolation is required.
    For most tests, consider using sync_client_session instead.
    """
    return primp.Client()


@pytest.fixture
async def async_client() -> primp.AsyncClient:
    """Function-scoped async Client fixture.
    
    Use this when test isolation is required.
    For most tests, consider using async_client_session instead.
    """
    return primp.AsyncClient()


# Session-scoped fixtures (for better performance when test isolation isn't needed)
@pytest.fixture(scope="session")
def sync_client_session() -> primp.Client:
    """Session-scoped synchronous Client fixture.
    
    This fixture creates a single Client instance that is reused across all tests
    in the session, providing better performance for tests that don't require
    complete isolation.
    
    Example:
        def test_something(sync_client_session):
            response = sync_client_session.get("http://example.com")
            assert response.status_code == 200
    """
    return primp.Client()


@pytest.fixture(scope="session")
async def async_client_session() -> primp.AsyncClient:
    """Session-scoped async Client fixture.
    
    This fixture creates a single AsyncClient instance that is reused across all tests
    in the session, providing better performance for tests that don't require
    complete isolation.
    
    Example:
        async def test_something(async_client_session):
            response = await async_client_session.get("http://example.com")
            assert response.status_code == 200
    """
    return primp.AsyncClient()
