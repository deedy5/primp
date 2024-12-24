from typing import Dict, Optional, Tuple, Union

class Response:
    @property
    def content(self) -> bytes: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
    @property
    def headers(self) -> Dict[str, str]: ...
    @property
    def status_code(self) -> int: ...
    @property
    def url(self) -> str: ...
    @property
    def encoding(self) -> str: ...
    @property
    def text(self) -> str: ...
    @property
    def json(self) -> Union[dict, list]: ...
    @property
    def text_markdown(self) -> str: ...
    @property
    def text_plain(self) -> str: ...
    @property
    def text_rich(self) -> str: ...

class Client:
    def __init__(
        self,
        auth: Optional[tuple] = None,
        auth_bearer: Optional[str] = None,
        params: Optional[dict] = None,
        headers: Optional[dict] = None,
        cookies: Optional[dict] = None,
        timeout: Optional[float] = None,
        cookie_store: Optional[bool] = True,
        referer: Optional[bool] = True,
        proxy: Optional[str] = None,
        impersonate: Optional[str] = None,
        follow_redirects: bool = True,
        max_redirects: int = 20,
        verify: Optional[bool] = True,
        ca_cert_file: Optional[str] = None,
        https_only: Optional[bool] = False,
        http2_only: Optional[bool] = False,
    ) -> None: ...
    @property
    def headers(self) -> Dict[str, str]: ...
    @headers.setter
    def headers(self, new_headers: Dict[str, str]) -> None: ...
    @property
    def cookies(self) -> Dict[str, str]: ...
    @cookies.setter
    def cookies(self, cookies: Dict[str, str]) -> None: ...
    @property
    def proxy(self) -> Optional[str]: ...
    @proxy.setter
    def proxy(self, proxy: str) -> None: ...
    def request(
        self,
        method: str,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Union[dict, list]] = None,
        json: Optional[Union[dict, list]] = None,
        files: Optional[Union[Dict[str, bytes]]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def get(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def head(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def options(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def delete(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def post(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Union[dict, list]] = None,
        json: Optional[Union[dict, list]] = None,
        files: Optional[Dict[str, bytes]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def put(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Union[dict, list]] = None,
        json: Optional[Union[dict, list]] = None,
        files: Optional[Dict[str, bytes]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...
    def patch(
        self,
        url: str,
        params: Optional[Dict[str, str]] = None,
        headers: Optional[Dict[str, str]] = None,
        cookies: Optional[Dict[str, str]] = None,
        content: Optional[bytes] = None,
        data: Optional[Union[dict, list]] = None,
        json: Optional[Union[dict, list]] = None,
        files: Optional[Dict[str, bytes]] = None,
        auth: Optional[Tuple[str, Optional[str]]] = None,
        auth_bearer: Optional[str] = None,
        timeout: Optional[float] = None,
    ) -> Response: ...

def request(
    method: str,
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Union[dict, list]] = None,
    json: Optional[Union[dict, list]] = None,
    files: Optional[Dict[str, bytes]] = None,
    auth: Optional[Tuple[str, Optional[str],]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def get(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def head(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def options(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def delete(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def post(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Union[dict, list]] = None,
    json: Optional[Union[dict, list]] = None,
    files: Optional[Dict[str, bytes]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def put(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Union[dict, list]] = None,
    json: Optional[Union[dict, list]] = None,
    files: Optional[Dict[str, bytes]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
def patch(
    url: str,
    params: Optional[Dict[str, str]] = None,
    headers: Optional[Dict[str, str]] = None,
    cookies: Optional[Dict[str, str]] = None,
    content: Optional[bytes] = None,
    data: Optional[Union[dict, list]] = None,
    json: Optional[Union[dict, list]] = None,
    files: Optional[Dict[str, bytes]] = None,
    auth: Optional[Tuple[str, Optional[str]]] = None,
    auth_bearer: Optional[str] = None,
    timeout: Optional[float] = None,
    impersonate: Optional[str] = None,
    verify: Optional[bool] = None,
    ca_cert_file: Optional[str] = None,
) -> Response: ...
