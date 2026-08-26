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

    def test_sync_per_request_cookies_do_not_persist(self, test_server: str) -> None:
        """Per-request cookies are one-shot and must not leak to the store."""
        base_url = test_server

        client = primp.Client()
        client.get(f"{base_url}/cookies", cookies={"one_shot": "abc"})

        response = client.get(f"{base_url}/cookies")
        assert response.status_code == 200
        data = response.json()
        assert "one_shot" not in data["cookies"]

    @pytest.mark.asyncio
    async def test_async_per_request_cookies_do_not_persist(self, test_server: str) -> None:
        """Per-request cookies are one-shot and must not leak to the store."""
        base_url = test_server

        client = primp.AsyncClient()
        await client.get(f"{base_url}/cookies", cookies={"one_shot": "abc"})

        response = await client.get(f"{base_url}/cookies")
        assert response.status_code == 200
        data = response.json()
        assert "one_shot" not in data["cookies"]

    def test_sync_client_and_per_request_cookies_merge(self, test_server: str) -> None:
        """Client-level cookies persist; per-request cookies are sent once only."""
        base_url = test_server

        client = primp.Client(cookies={"persistent": "p1"})
        response = client.get(
            f"{base_url}/cookies",
            cookies={"one_shot": "o1"},
        )
        assert response.status_code == 200
        data = response.json()
        assert data["cookies"]["persistent"] == "p1"
        assert data["cookies"]["one_shot"] == "o1"

        # Follow-up without per-request: persistent only.
        response = client.get(f"{base_url}/cookies")
        assert response.status_code == 200
        data = response.json()
        assert data["cookies"]["persistent"] == "p1"
        assert "one_shot" not in data["cookies"]


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


class TestRequestReadTimeout:
    """Tests for per-request read_timeout parameter."""

    def test_sync_client_read_timeout_per_request(self, test_server: str) -> None:
        """Test per-request read_timeout on sync Client."""
        base_url = test_server

        client = primp.Client()
        response = client.get(f"{base_url}/get", read_timeout=30)

        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_client_read_timeout_per_request(self, test_server: str) -> None:
        """Test per-request read_timeout on AsyncClient."""
        base_url = test_server

        client = primp.AsyncClient()
        response = await client.get(f"{base_url}/get", read_timeout=30)

        assert response.status_code == 200

    def test_module_read_timeout_per_request(self, test_server: str) -> None:
        """Test per-request read_timeout on module function."""
        base_url = test_server

        response = primp.get(f"{base_url}/get", read_timeout=30)

        assert response.status_code == 200

    def test_both_timeout_and_read_timeout(self, test_server: str) -> None:
        """Test both timeout and read_timeout together."""
        base_url = test_server

        client = primp.Client()
        response = client.get(f"{base_url}/get", timeout=30, read_timeout=15)

        assert response.status_code == 200


class TestRequestFollowRedirects:
    """Tests for per-request follow_redirects parameter."""

    def test_sync_client_follow_redirects_true(self, test_server: str) -> None:
        """Test per-request follow_redirects=True on sync Client."""
        base_url = test_server

        client = primp.Client(follow_redirects=False)
        response = client.get(f"{base_url}/get", follow_redirects=True)

        assert response.status_code == 200

    def test_sync_client_follow_redirects_false(self, test_server: str) -> None:
        """Test per-request follow_redirects=False on sync Client."""
        base_url = test_server

        client = primp.Client(follow_redirects=True)
        response = client.get(f"{base_url}/get", follow_redirects=False)

        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_client_follow_redirects_per_request(self, test_server: str) -> None:
        """Test per-request follow_redirects on AsyncClient."""
        base_url = test_server

        client = primp.AsyncClient(follow_redirects=False)
        response = await client.get(f"{base_url}/get", follow_redirects=True)

        assert response.status_code == 200

    def test_module_follow_redirects_per_request(self, test_server: str) -> None:
        """Test per-request follow_redirects on module function."""
        base_url = test_server

        response = primp.get(f"{base_url}/get", follow_redirects=True)

        assert response.status_code == 200

    def test_sync_follow_redirects_override_restored_after_error(self, test_server: str) -> None:
        """An erroring request with a follow_redirects override must not leak
        the override into the shared client (finding #24, error path)."""
        client = primp.Client(follow_redirects=False)
        with pytest.raises((primp.ConnectError, primp.TimeoutError)):
            client.get("http://127.0.0.1:1/", follow_redirects=True, timeout=2)
        # The default policy must be restored: redirects are NOT followed.
        response = client.get(f"{test_server}/redirect/2")
        assert response.status_code == 302

    def test_sync_follow_redirects_override_restored_after_error_true_default(
        self, test_server: str
    ) -> None:
        """Same leak check with the opposite default: after an erroring
        follow_redirects=False override, redirects must be followed again."""
        client = primp.Client(follow_redirects=True)
        with pytest.raises((primp.ConnectError, primp.TimeoutError)):
            client.get("http://127.0.0.1:1/", follow_redirects=False, timeout=2)
        response = client.get(f"{test_server}/redirect/2")
        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_follow_redirects_override_restored_after_error(
        self, test_server: str
    ) -> None:
        """An erroring async request with a follow_redirects override must
        not leak the override into the shared client (finding #24, error
        path)."""
        client = primp.AsyncClient(follow_redirects=False)
        with pytest.raises((primp.ConnectError, primp.TimeoutError)):
            await client.get("http://127.0.0.1:1/", follow_redirects=True, timeout=2)
        response = await client.get(f"{test_server}/redirect/2")
        assert response.status_code == 302

    def test_sync_follow_redirects_attr_setter_takes_effect(self, test_server: str) -> None:
        """Setting `client.follow_redirects` after construction must affect
        subsequent requests (previously a no-op: only the constructor param
        was honored by the built client)."""
        base_url = test_server

        client = primp.Client(follow_redirects=True)
        assert client.get(f"{base_url}/redirect/2").status_code == 200

        client.follow_redirects = False
        response = client.get(f"{base_url}/redirect/2")
        assert response.status_code == 302

        client.follow_redirects = True
        assert client.get(f"{base_url}/redirect/2").status_code == 200

    def test_sync_follow_redirects_attr_is_per_request_only(self, test_server: str) -> None:
        """The follow_redirects attribute change must not leak into requests
        made by OTHER clients."""
        base_url = test_server

        client_a = primp.Client(follow_redirects=True)
        client_b = primp.Client(follow_redirects=True)
        client_a.follow_redirects = False
        assert client_b.get(f"{base_url}/redirect/2").status_code == 200

    @pytest.mark.asyncio
    async def test_async_follow_redirects_attr_setter_takes_effect(
        self, test_server: str
    ) -> None:
        """Async mirror: the follow_redirects attribute setter takes effect."""
        base_url = test_server

        client = primp.AsyncClient(follow_redirects=True)
        assert (await client.get(f"{base_url}/redirect/2")).status_code == 200

        client.follow_redirects = False
        response = await client.get(f"{base_url}/redirect/2")
        assert response.status_code == 302

        client.follow_redirects = True
        assert (await client.get(f"{base_url}/redirect/2")).status_code == 200


class TestClientScopedParamsNotPerRequest:
    """Documented behavior: impersonate/connect_timeout/https_only/http2_only are client-scoped.

    Per-request impersonation is NOT feasible in primp because the TLS ClientHello and
    browser-emulation config are built at Client::build() time, not per request. These options
    must be set on the Client (or via module-level helpers that build a throwaway client).
    """

    def test_per_request_timeout_overrides_client_setting(self, test_server: str) -> None:
        """Per-request timeout is supported and overrides the client-level value."""
        base_url = test_server

        # Client with a very short connect/overall timeout, but a working per-request override.
        client = primp.Client(timeout=0.0001)
        # A real per-request timeout larger than the client setting succeeds (override works).
        response = client.get(f"{base_url}/get", timeout=30)
        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_per_request_timeout_overrides_client_setting(
        self, test_server: str
    ) -> None:
        """Per-request timeout is supported on AsyncClient too."""
        base_url = test_server

        client = primp.AsyncClient(timeout=0.0001)
        response = await client.get(f"{base_url}/get", timeout=30)
        assert response.status_code == 200

    def test_impersonate_is_client_scoped(self, test_server: str) -> None:
        """impersonate must be set on the Client; module helper builds a throwaway client."""
        base_url = test_server

        # Constructing a client with impersonate works (client-scoped).
        client = primp.Client(impersonate="chrome_144")
        assert client.impersonate == "chrome_144"

        # The per-request request() API does not accept impersonate; it is client-scoped.
        # Passing it would be a TypeError (unknown keyword argument).
        with pytest.raises(TypeError):
            client.get(f"{base_url}/get", impersonate="chrome_144")

    @pytest.mark.asyncio
    async def test_async_impersonate_is_client_scoped(self, test_server: str) -> None:
        """impersonate must be set on the AsyncClient; per-request is not supported."""
        base_url = test_server

        client = primp.AsyncClient(impersonate="chrome_144")
        assert client.impersonate == "chrome_144"

        with pytest.raises(TypeError):
            await client.get(f"{base_url}/get", impersonate="chrome_144")

    def test_module_helper_builds_throwaway_client_with_impersonate(
        self, test_server: str
    ) -> None:
        """Module-level get/post build a throwaway client honoring impersonate."""
        base_url = test_server

        response = primp.get(
            f"{base_url}/get",
            impersonate="chrome_144",
            impersonate_os="windows",
        )
        assert response.status_code == 200

    def test_connect_timeout_is_client_scoped(self, test_server: str) -> None:
        """connect_timeout is client-scoped; per-request request() rejects it."""
        base_url = test_server

        client = primp.Client(connect_timeout=5)
        with pytest.raises(TypeError):
            client.get(f"{base_url}/get", connect_timeout=5)

    def test_https_only_is_client_scoped(self, test_server: str) -> None:
        """https_only is client-scoped; per-request request() rejects it."""
        base_url = test_server

        client = primp.Client()
        with pytest.raises(TypeError):
            client.get(f"{base_url}/get", https_only=True)

    def test_http2_only_is_client_scoped(self, test_server: str) -> None:
        """http2_only is client-scoped; per-request request() rejects it."""
        base_url = test_server

        client = primp.Client()
        with pytest.raises(TypeError):
            client.get(f"{base_url}/get", http2_only=True)


class TestClientBaseUrl:
    """Tests for base_url parameter and URL resolution."""

    def test_sync_client_base_url_resolves_relative_path(self, test_server: str) -> None:
        """Test that base_url resolves relative paths on sync Client."""
        client = primp.Client(base_url=test_server)
        response = client.get("/get")

        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"

    def test_sync_client_base_url_absolute_url_overrides(self, test_server: str) -> None:
        """Test that absolute URLs override base_url on sync Client."""
        base_url = test_server

        client = primp.Client(base_url="https://example.com")
        response = client.get(f"{base_url}/get")

        assert response.status_code == 200

    @pytest.mark.asyncio
    async def test_async_client_base_url_resolves_relative_path(self, test_server: str) -> None:
        """Test that base_url resolves relative paths on AsyncClient."""
        client = primp.AsyncClient(base_url=test_server)
        response = await client.get("/get")

        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"

    @pytest.mark.asyncio
    async def test_async_client_base_url_absolute_url_overrides(self, test_server: str) -> None:
        """Test that absolute URLs override base_url on AsyncClient."""
        base_url = test_server

        client = primp.AsyncClient(base_url="https://example.com")
        response = await client.get(f"{base_url}/get")

        assert response.status_code == 200


class TestClientInitCookies:
    """Tests for cookies parameter at client initialization."""

    def test_sync_client_init_cookies(self, test_server: str) -> None:
        """Test client initialization with cookies on sync Client."""
        base_url = test_server

        client = primp.Client(cookies={"init_cookie": "init_value"})
        response = client.get(f"{base_url}/cookies")

        assert response.status_code == 200
        data = response.json()
        assert "init_cookie" in data["cookies"]
        assert data["cookies"]["init_cookie"] == "init_value"

    @pytest.mark.asyncio
    async def test_async_client_init_cookies(self, test_server: str) -> None:
        """Test client initialization with cookies on AsyncClient."""
        base_url = test_server

        client = primp.AsyncClient(cookies={"init_cookie": "init_value"})
        response = await client.get(f"{base_url}/cookies")

        assert response.status_code == 200
        data = response.json()
        assert "init_cookie" in data["cookies"]
        assert data["cookies"]["init_cookie"] == "init_value"


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


class TestDataFilesConflict:
    """data and files are mutually exclusive body sources."""

    def test_sync_data_and_files_conflict_raises(self, test_server: str) -> None:
        """Passing both data and files must raise; they used to silently
        overwrite each other."""
        base_url = test_server

        client = primp.Client()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("content")
            temp_path = f.name
        try:
            with pytest.raises(primp.PrimpError):
                client.post(
                    f"{base_url}/post",
                    data={"field": "value"},
                    files={"file": temp_path},
                )
        finally:
            os.unlink(temp_path)

    @pytest.mark.asyncio
    async def test_async_data_and_files_conflict_raises(self, test_server: str) -> None:
        """Async mirror of the data+files conflict check."""
        base_url = test_server

        client = primp.AsyncClient()
        with tempfile.NamedTemporaryFile(mode="w", suffix=".txt", delete=False) as f:
            f.write("content")
            temp_path = f.name
        try:
            with pytest.raises(primp.PrimpError):
                await client.post(
                    f"{base_url}/post",
                    data={"field": "value"},
                    files={"file": temp_path},
                )
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


class TestModuleVerify:
    """Tests for module-level verify parameter."""

    def test_module_verify_false(self, test_server: str) -> None:
        """Test module-level function with verify=False."""
        base_url = test_server

        response = primp.get(f"{base_url}/get", verify=False)
        assert response.status_code == 200
        data = response.json()
        assert data["method"] == "GET"

    def test_module_verify_true(self, test_server: str) -> None:
        """Test module-level function with verify=True (default)."""
        base_url = test_server

        response = primp.get(f"{base_url}/get", verify=True)
        assert response.status_code == 200


class TestRequestFollowRedirectsNoLeak:
    """Regression tests for per-request ``follow_redirects`` (audit finding #24).

    A per-request ``follow_redirects`` override must not permanently mutate the
    shared client's redirect policy. Constructing a client with
    ``follow_redirects=False`` and then issuing a single request with
    ``follow_redirects=True`` should leave the client itself still NOT following
    redirects afterwards.

    NOTE: not named `TestRequestFollowRedirects` (a duplicate would shadow
    the earlier class and pytest would skip its tests).
    """

    def test_sync_per_request_does_not_mutate_client(self, test_server: str) -> None:
        base_url = test_server
        client = primp.Client(follow_redirects=False)

        # Per-request override follows the redirect chain to the final 200.
        r1 = client.get(f"{base_url}/redirect/2", follow_redirects=True)
        assert r1.status_code == 200

        # The client itself is still configured to NOT follow redirects, so a
        # plain call must surface the intermediate 302 rather than the final 200.
        r2 = client.get(f"{base_url}/redirect/2")
        assert r2.status_code == 302

    @pytest.mark.asyncio
    async def test_async_per_request_does_not_mutate_client(
        self, test_server: str
    ) -> None:
        base_url = test_server
        client = primp.AsyncClient(follow_redirects=False)

        r1 = await client.get(f"{base_url}/redirect/2", follow_redirects=True)
        assert r1.status_code == 200

        r2 = await client.get(f"{base_url}/redirect/2")
        assert r2.status_code == 302

    def test_sync_default_follows_per_request_disable(self, test_server: str) -> None:
        """A per-request ``follow_redirects=False`` must override the default."""
        base_url = test_server
        client = primp.Client()  # default follow_redirects=True

        r = client.get(f"{base_url}/redirect/2", follow_redirects=False)
        assert r.status_code == 302
