"""
Comprehensive tests for all exception types in primp.

Tests each exception type with real scenarios that trigger them,
using the local test server to raise real exceptions.
"""
import pytest

import primp
from primp import (
    PrimpError,
    BuilderError,
    RequestError,
    ConnectError,
    TimeoutError,
    StatusError,
    RedirectError,
    BodyError,
    DecodeError,
    UpgradeError,
)


class TestExceptionHierarchy:
    """Test that the exception hierarchy is correct."""

    def test_primp_error_is_base(self):
        """PrimpError should be the base exception for all primp exceptions."""
        assert issubclass(BuilderError, PrimpError)
        assert issubclass(RequestError, PrimpError)
        assert issubclass(StatusError, PrimpError)
        assert issubclass(RedirectError, PrimpError)
        assert issubclass(BodyError, PrimpError)
        assert issubclass(DecodeError, PrimpError)
        assert issubclass(UpgradeError, PrimpError)

    def test_request_error_children(self):
        """RequestError should have ConnectError and TimeoutError as children."""
        assert issubclass(ConnectError, RequestError)
        assert issubclass(TimeoutError, RequestError)


class TestBuilderError:
    """Test BuilderError exception with real scenarios.

    BuilderError is raised for URL and header errors, providing a descriptive
    message explaining the specific error.
    """

    def test_missing_scheme(self):
        """URL without scheme should raise BuilderError."""
        client = primp.Client()
        with pytest.raises(BuilderError):
            client.get("example.com")

    def test_empty_url(self):
        """Empty URL should raise BuilderError."""
        client = primp.Client()
        with pytest.raises(BuilderError):
            client.get("")

    def test_invalid_url_format(self):
        """Invalid URL format should raise BuilderError."""
        client = primp.Client()
        with pytest.raises(BuilderError):
            client.get("://invalid")

    def test_can_catch_as_primp_error(self):
        """BuilderError should be catchable as PrimpError."""
        client = primp.Client()
        with pytest.raises(PrimpError):
            client.get("example.com")

    def test_invalid_header_with_newline(self, test_server_dynamic_port):
        """Header with newline character should raise BuilderError."""
        client = primp.Client()
        # Header values with newlines are invalid
        with pytest.raises(BuilderError):
            client.get(f"{test_server_dynamic_port}/get", headers={"X-Test": "value\nwith-newline"})

    def test_invalid_header_with_carriage_return(self, test_server_dynamic_port):
        """Header with carriage return should raise BuilderError."""
        client = primp.Client()
        with pytest.raises(BuilderError):
            client.get(f"{test_server_dynamic_port}/get", headers={"X-Test": "value\rwith-cr"})

    def test_invalid_header_name(self, test_server_dynamic_port):
        """Invalid header name should raise BuilderError."""
        client = primp.Client()
        # Header names with special characters are invalid
        with pytest.raises(BuilderError):
            client.get(f"{test_server_dynamic_port}/get", headers={"Invalid:Name": "value"})


class TestConnectError:
    """Test ConnectError exception with real scenarios."""

    def test_connection_refused(self):
        """Connection to non-listening port should raise ConnectError."""
        client = primp.Client()
        with pytest.raises(ConnectError):
            client.get("http://localhost:1", timeout=5)

    def test_can_catch_as_request_error(self):
        """ConnectError should be catchable as RequestError."""
        client = primp.Client()
        with pytest.raises(RequestError):
            client.get("http://localhost:1", timeout=0.5)

    def test_can_catch_as_primp_error(self):
        """ConnectError should be catchable as PrimpError."""
        client = primp.Client()
        with pytest.raises(PrimpError):
            client.get("http://localhost:1", timeout=0.5)


class TestTimeoutError:
    """Test TimeoutError exception with real scenarios."""

    def test_request_timeout(self, test_server_dynamic_port):
        """Request that exceeds timeout should raise TimeoutError."""
        client = primp.Client()
        # Request a 5 second delay with a 0.5 second timeout
        with pytest.raises(TimeoutError):
            client.get(f"{test_server_dynamic_port}/delay/5", timeout=0.5)

    def test_can_catch_as_request_error(self, test_server_dynamic_port):
        """TimeoutError should be catchable as RequestError."""
        client = primp.Client()
        with pytest.raises(RequestError):
            client.get(f"{test_server_dynamic_port}/delay/5", timeout=0.5)

    def test_can_catch_as_primp_error(self, test_server_dynamic_port):
        """TimeoutError should be catchable as PrimpError."""
        client = primp.Client()
        with pytest.raises(PrimpError):
            client.get(f"{test_server_dynamic_port}/delay/5", timeout=0.5)


class TestStatusError:
    """Test StatusError exception with real scenarios."""

    def test_404_status(self, test_server_dynamic_port):
        """HTTP 404 should raise StatusError when raise_for_status is called."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/status/404")
        with pytest.raises(StatusError):
            response.raise_for_status()

    def test_500_status(self, test_server_dynamic_port):
        """HTTP 500 should raise StatusError when raise_for_status is called."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/status/500")
        with pytest.raises(StatusError):
            response.raise_for_status()

    def test_403_status(self, test_server_dynamic_port):
        """HTTP 403 should raise StatusError when raise_for_status is called."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/status/403")
        with pytest.raises(StatusError):
            response.raise_for_status()

    def test_can_catch_as_primp_error(self, test_server_dynamic_port):
        """StatusError should be catchable as PrimpError."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/status/404")
        with pytest.raises(PrimpError):
            response.raise_for_status()

    def test_successful_status_no_error(self, test_server_dynamic_port):
        """Successful status codes should not raise StatusError."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/status/200")
        # Should not raise
        response.raise_for_status()


class TestRedirectError:
    """Test RedirectError exception with real scenarios."""

    def test_max_redirects_exceeded(self, test_server_dynamic_port):
        """Exceeding max_redirects should raise RedirectError."""
        client = primp.Client(follow_redirects=True, max_redirects=2)
        # Request 10 redirects with max_redirects=2
        with pytest.raises(RedirectError):
            client.get(f"{test_server_dynamic_port}/redirect/10")

    def test_can_catch_as_primp_error(self, test_server_dynamic_port):
        """RedirectError should be catchable as PrimpError."""
        client = primp.Client(follow_redirects=True, max_redirects=2)
        with pytest.raises(PrimpError):
            client.get(f"{test_server_dynamic_port}/redirect/10")


class TestDecodeError:
    """Test DecodeError exception with real scenarios.

    DecodeError is raised when the server returns content with a
    Content-Encoding header (like gzip or deflate) but the actual
    content is not properly encoded. The client automatically decompresses
    based on the Content-Encoding header.
    """

    def test_invalid_gzip_content(self, test_server_dynamic_port):
        """Invalid gzip content should raise DecodeError when decompressing."""
        client = primp.Client()
        # The server returns invalid gzip data with Content-Encoding: gzip
        # This should trigger DecodeError when the client tries to decompress
        # The error is raised when accessing the content, not during the request
        response = client.get(f"{test_server_dynamic_port}/invalid-gzip")
        with pytest.raises(DecodeError):
            _ = response.content

    def test_invalid_deflate_content(self, test_server_dynamic_port):
        """Invalid deflate content should raise DecodeError when decompressing."""
        client = primp.Client()
        # The server returns invalid deflate data with Content-Encoding: deflate
        response = client.get(f"{test_server_dynamic_port}/invalid-deflate")
        with pytest.raises(DecodeError):
            _ = response.content

    def test_can_catch_as_primp_error(self, test_server_dynamic_port):
        """DecodeError should be catchable as PrimpError."""
        client = primp.Client()
        response = client.get(f"{test_server_dynamic_port}/invalid-gzip")
        with pytest.raises(PrimpError):
            _ = response.content


class TestBodyError:
    """Test BodyError exception.

    BodyError is raised when there's an error with the request or response body,
    such as when streaming a request body that encounters an error, or when
    there's a connection error while reading/writing the body.

    BodyError can be triggered by:
    1. Timeout while reading/writing body
    2. Connection errors while streaming body
    3. Request body stream errors

    Note: DecodeError is raised for transfer encoding errors (like broken chunked encoding),
    not BodyError. BodyError is for lower-level body I/O issues.
    """

    def test_request_body_error_on_closed_connection(self, test_server_dynamic_port):
        """Sending a large request body to a server that closes early may trigger BodyError.

        This test attempts to trigger BodyError by sending a large request body to a server
        that closes the connection immediately. Depending on timing, this may result in
        BodyError, RequestError, or no error if the server responds before the body is sent.
        """
        client = primp.Client(timeout=2)
        # Create a large body that will take time to send
        large_body = b"x" * 10_000_000  # 10MB of data
        # The server may close the connection while we're still sending
        # This could trigger either BodyError or RequestError depending on timing
        try:
            response = client.post(f"{test_server_dynamic_port}/broken-body", content=large_body)
            # If we get here, the request completed - try to read the response
            _ = response.content
        except BodyError as e:
            assert isinstance(e, PrimpError)


class TestUpgradeError:
    """Test UpgradeError exception.

    UpgradeError is raised when there's an error during HTTP protocol upgrade
    (e.g., WebSocket upgrade). This occurs when calling response.upgrade() on a
    response that indicated a protocol upgrade (101 Switching Protocols) but
    the upgrade handshake failed.

    Note: The upgrade() method is not currently exposed in the Python bindings,
    so UpgradeError cannot be reliably triggered with real HTTP requests.
    Therefore, no real tests are available for this exception type.
    """

    # No tests available - upgrade() is not exposed in Python bindings
    # UpgradeError cannot be reliably triggered with real HTTP requests
    pass


class TestAsyncExceptions:
    """Test exceptions with async client."""

    @pytest.mark.asyncio
    async def test_invalid_url_async(self):
        """Invalid URL should raise BuilderError with async client."""
        client = primp.AsyncClient()
        with pytest.raises(BuilderError):
            await client.get("example.com")

    @pytest.mark.asyncio
    async def test_invalid_header_async(self, test_server_dynamic_port):
        """Invalid header should raise BuilderError with async client."""
        client = primp.AsyncClient()
        with pytest.raises(BuilderError):
            await client.get(f"{test_server_dynamic_port}/get", headers={"X-Test": "value\nwith-newline"})

    @pytest.mark.asyncio
    async def test_connect_error_async(self):
        """Connection error should raise ConnectError with async client."""
        client = primp.AsyncClient()
        with pytest.raises(ConnectError):
            await client.get("http://localhost:1", timeout=5)

    @pytest.mark.asyncio
    async def test_timeout_error_async(self, test_server_dynamic_port):
        """Timeout should raise TimeoutError with async client."""
        client = primp.AsyncClient()
        with pytest.raises(TimeoutError):
            await client.get(f"{test_server_dynamic_port}/delay/5", timeout=0.5)

    @pytest.mark.asyncio
    async def test_status_error_async(self, test_server_dynamic_port):
        """HTTP error status should raise StatusError with async client."""
        client = primp.AsyncClient()
        response = await client.get(f"{test_server_dynamic_port}/status/404")
        with pytest.raises(StatusError):
            response.raise_for_status()

    @pytest.mark.asyncio
    async def test_redirect_error_async(self, test_server_dynamic_port):
        """Exceeding max_redirects should raise RedirectError with async client."""
        client = primp.AsyncClient(follow_redirects=True, max_redirects=2)
        with pytest.raises(RedirectError):
            await client.get(f"{test_server_dynamic_port}/redirect/10")

    @pytest.mark.asyncio
    async def test_decode_error_async(self, test_server_dynamic_port):
        """Invalid gzip content should raise DecodeError with async client."""
        client = primp.AsyncClient()
        response = await client.get(f"{test_server_dynamic_port}/invalid-gzip")
        with pytest.raises(DecodeError):
            _ = response.content

    @pytest.mark.asyncio
    async def test_can_catch_as_primp_error_async(self):
        """Exceptions should be catchable as PrimpError with async client."""
        client = primp.AsyncClient()
        with pytest.raises(PrimpError):
            await client.get("example.com")

    @pytest.mark.asyncio
    async def test_body_error_async(self, test_server_dynamic_port):
        """Sending a large request body to a server that closes early may trigger BodyError with async client.

        This test attempts to trigger BodyError by sending a large request body to a server
        that closes the connection immediately. Depending on timing, this may result in
        BodyError, RequestError, or no error if the server responds before the body is sent.
        """
        client = primp.AsyncClient(timeout=2)
        # Create a large body that will take time to send
        large_body = b"x" * 10_000_000  # 10MB of data
        # The server may close the connection while we're still sending
        try:
            response = await client.post(f"{test_server_dynamic_port}/broken-body", content=large_body)
            _ = response.content
        except (BodyError, RequestError, ConnectError) as e:
            # Any of these errors is acceptable - the server closed the connection
            assert isinstance(e, PrimpError)
