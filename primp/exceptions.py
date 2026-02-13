"""Exception types for primp.

All exceptions inherit from RequestException for easy catching.

Example:
    try:
        response = client.get("https://example.com")
    except primp.exceptions.HTTPError as e:
        print(f"HTTP error: {e.status_code}")
    except primp.exceptions.ConnectionError as e:
        print(f"Connection failed: {e}")
    except primp.exceptions.RequestException as e:
        print(f"Request failed: {e}")
"""

from .primp import (
    # Base exception
    RequestException,
    # HTTP errors
    HTTPError,
    # Connection errors
    ConnectionError,
    ConnectTimeout,
    SSLError,
    ProxyError,
    # Timeout errors
    Timeout,
    ReadTimeout,
    # Request errors
    RequestError,
    InvalidURL,
    InvalidHeader,
    # Body errors
    BodyError,
    StreamConsumedError,
    ChunkedEncodingError,
    # Decode errors
    DecodeError,
    ContentDecodingError,
    # JSON errors
    JSONError,
    InvalidJSONError,
    JSONDecodeError,
    # Redirect errors
    TooManyRedirects,
)

__all__ = [
    # Base exception
    "RequestException",
    # HTTP errors
    "HTTPError",
    # Connection errors
    "ConnectionError",
    "ConnectTimeout",
    "SSLError",
    "ProxyError",
    # Timeout errors
    "Timeout",
    "ReadTimeout",
    # Request errors
    "RequestError",
    "InvalidURL",
    "InvalidHeader",
    # Body errors
    "BodyError",
    "StreamConsumedError",
    "ChunkedEncodingError",
    # Decode errors
    "DecodeError",
    "ContentDecodingError",
    # JSON errors
    "JSONError",
    "InvalidJSONError",
    "JSONDecodeError",
    # Redirect errors
    "TooManyRedirects",
]
