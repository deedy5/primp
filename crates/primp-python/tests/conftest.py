"""
Shared pytest fixtures and configuration for primp tests.

This module provides common fixtures used across all test modules:
- sync_client: Synchronous Client fixture
- async_client: Async client fixture
- test_server_dynamic_port: Test server fixture from server.py
"""

import pytest
import primp
from tests.server import test_server_dynamic_port


__all__ = ["sync_client", "async_client", "test_server_dynamic_port"]


@pytest.fixture
def sync_client() -> primp.Client:
    return primp.Client()


@pytest.fixture
async def async_client() -> primp.AsyncClient:
    return primp.AsyncClient()
