from __future__ import annotations

import sys
from typing import Any, Literal, TypedDict

if sys.version_info <= (3, 11):
    from typing_extensions import Unpack
else:
    from typing import Unpack

HttpMethod = Literal["GET", "HEAD", "OPTIONS", "DELETE", "POST", "PUT", "PATCH"]
IMPERSONATE = Literal[
        "chrome_100", "chrome_101", "chrome_104", "chrome_105", "chrome_106",
        "chrome_107", "chrome_108", "chrome_109", "chrome_114", "chrome_116",
        "chrome_117", "chrome_118", "chrome_119", "chrome_120", "chrome_123",
        "chrome_124", "chrome_126", "chrome_127", "chrome_128", "chrome_129",
        "chrome_130", "chrome_131",
        "safari_15.3", "safari_15.5", "safari_15.6.1", "safari_16",
        "safari_16.5", "safari_17.0", "safari_17.2.1", "safari_17.4.1",
        "safari_17.5", "safari_18",  "safari_18.2",
        "safari_ios_16.5", "safari_ios_17.2", "safari_ios_17.4.1", "safari_ios_18.1.1",
        "safari_ipad_18",
        "okhttp_3.9", "okhttp_3.11", "okhttp_3.13", "okhttp_3.14", "okhttp_4.9",
        "okhttp_4.10", "okhttp_5",
        "edge_101", "edge_122", "edge_127", "edge_131",
        "firefox_109", "firefox_117", "firefox_128", "firefox_133",
    ]  # fmt: skip
IMPERSONATE_OS = Literal["android", "ios", "linux", "macos", "windows"]

class RequestParams(TypedDict, total=False):
    auth: tuple[str, str | None] | None
    auth_bearer: str | None
    params: dict[str, str] | None
    headers: dict[str, str] | None
    cookies: dict[str, str] | None
    timeout: float | None
    content: bytes | None
    data: dict[str, Any] | None
    json: Any | None
    files: dict[str, str] | None

class ClientRequestParams(RequestParams):
    impersonate: IMPERSONATE | None
    impersonate_os: IMPERSONATE_OS | None
    verify: bool | None
    ca_cert_file: str | None

class Response:
    @property
    def content(self) -> bytes: ...
    @property
    def cookies(self) -> dict[str, str]: ...
    @property
    def headers(self) -> dict[str, str]: ...
    @property
    def status_code(self) -> int: ...
    @property
    def url(self) -> str: ...
    @property
    def encoding(self) -> str: ...
    @property
    def text(self) -> str: ...
    def json(self) -> Any: ...
    @property
    def text_markdown(self) -> str: ...
    @property
    def text_plain(self) -> str: ...
    @property
    def text_rich(self) -> str: ...

class RClient:
    def __init__(
        self,
        auth: tuple[str, str | None] | None = None,
        auth_bearer: str | None = None,
        params: dict[str, str] | None = None,
        headers: dict[str, str] | None = None,
        cookies: dict[str, str] | None = None,
        timeout: float | None = None,
        cookie_store: bool | None = True,
        referer: bool | None = True,
        proxy: str | None = None,
        impersonate: IMPERSONATE | None = None,
        impersonate_os: IMPERSONATE_OS | None = None,
        follow_redirects: bool | None = True,
        max_redirects: int | None = 20,
        verify: bool | None = True,
        ca_cert_file: str | None = None,
        https_only: bool | None = False,
        http2_only: bool | None = False,
    ): ...
    @property
    def headers(self) -> dict[str, str]: ...
    @headers.setter
    def headers(self, headers: dict[str, str]) -> None: ...
    @property
    def cookies(self) -> dict[str, str]: ...
    @cookies.setter
    def cookies(self, cookies: dict[str, str]) -> None: ...
    @property
    def proxy(self) -> str | None: ...
    @proxy.setter
    def proxy(self, proxy: str) -> None: ...
    @property
    def impersonate(self) -> str | None: ...
    @impersonate.setter
    def impersonate(self, impersonate: IMPERSONATE) -> None: ...
    @property
    def impersonate_os(self) -> str | None: ...
    @impersonate_os.setter
    def impersonate_os(self, impersonate: IMPERSONATE_OS) -> None: ...
    def request(self, method: HttpMethod, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def get(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def head(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def options(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def delete(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def post(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def put(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...
    def patch(self, url: str, **kwargs: Unpack[RequestParams]) -> Response: ...

def request(method: HttpMethod, url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def get(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def head(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def options(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def delete(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def post(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def put(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
def patch(url: str, **kwargs: Unpack[ClientRequestParams]) -> Response: ...
