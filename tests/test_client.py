from time import sleep

import pytest

import certifi
import primp  # type: ignore


def retry(max_retries=3, delay=1):
    def decorator(func):
        def wrapper(*args, **kwargs):
            for attempt in range(max_retries):
                try:
                    return func(*args, **kwargs)
                except Exception as e:
                    if attempt < max_retries - 1:
                        sleep(delay)
                        continue
                    else:
                        raise e

        return wrapper

    return decorator


@retry()
def test_client_init_params():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    client = primp.Client(
        auth=auth,
        params=params,
        headers=headers,
        ca_cert_file=certifi.where(),
    )
    client.set_cookies("https://httpbin.org", cookies)
    response = client.get("https://httpbin.org/anything")
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/json"
    json_data = response.json()
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_setters():
    client = primp.Client()
    client.auth = ("user", "password")
    client.headers = {"X-Test": "TesT"}
    client.params = {"x": "aaa", "y": "bbb"}
    client.timeout = 20

    client.set_cookies("https://httpbin.org", {"ccc": "ddd", "cccc": "dddd"})
    assert client.get_cookies("https://httpbin.org/anything") == {"ccc": "ddd", "cccc": "dddd"}

    response = client.get("https://httpbin.org/anything")
    assert response.status_code == 200
    assert response.status_code == 200
    assert client.auth == ("user", "password")
    assert client.headers == {"x-test": "TesT"}
    assert client.params == {"x": "aaa", "y": "bbb"}
    assert client.timeout == 20.0
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "TesT"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Basic dXNlcjpwYXNzd29yZA==" in response.text
    assert b"Basic dXNlcjpwYXNzd29yZA==" in response.content
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_request_get():
    client = primp.Client()
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    client.set_cookies("https://httpbin.org", cookies)
    response = client.request(
        "GET",
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_get():
    client = primp.Client()
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    client.set_cookies("https://httpbin.org", cookies)
    response = client.get(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_post_content():
    client = primp.Client()
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    content = b"test content"
    client.set_cookies("https://httpbin.org", cookies)
    response = client.post(
        "https://httpbin.org/anything",
        auth=auth,
        headers=headers,
        params=params,
        content=content,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["data"] == "test content"
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_post_data():
    client = primp.Client()
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    client.set_cookies("https://httpbin.org", cookies)
    response = client.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_post_json():
    client = primp.Client()
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    client.set_cookies("https://httpbin.org", cookies)
    response = client.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        json=data,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["json"] == data
    # temp
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@pytest.fixture(scope="session")
def test_files(tmp_path_factory):
    tmp_path_factory.mktemp("data")
    temp_file1 = tmp_path_factory.mktemp("data") / "img1.png"
    with open(temp_file1, "w") as f:
        f.write("aaa111")
    temp_file2 = tmp_path_factory.mktemp("data") / "img2.png"
    with open(temp_file2, "w") as f:
        f.write("bbb222")
    return str(temp_file1), str(temp_file2)


def test_client_post_files(test_files):
    temp_file1, temp_file2 = test_files
    client = primp.Client()
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    files = {"file1": temp_file1, "file2": temp_file2}
    client.set_cookies("https://httpbin.org", cookies)
    response = client.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        files=files,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["files"] == {"file1": "aaa111", "file2": "bbb222"}
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_client_impersonate_chrome131():
    client = primp.Client(
        impersonate="chrome_131",
        impersonate_os="windows",
    )
    response = client.get("https://tls.peet.ws/api/all")
    #response = client.get("https://tls.http.rw/api/all")
    assert response.status_code == 200
    json_data = response.json()
    assert (
        json_data["user_agent"]
        == "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    )
    assert json_data["tls"]["ja4"] == "t13d1516h2_8daaf6152771_b1ff8ab2d16f"
    assert (
        json_data["http2"]["akamai_fingerprint_hash"]
        == "52d84b11737d980aef856699f885ca86"
    )
    assert json_data["tls"]["peetprint_hash"] == "7466733991096b3f4e6c0e79b0083559"
