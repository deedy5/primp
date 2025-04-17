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
def test_request_get():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    response = primp.request(
        "GET",
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
        params=params,
        ca_cert_file=certifi.where(),
    )
    assert response.status_code == 200
    assert response.headers["content-type"] == "application/json"
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
def test_get():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    response = primp.get(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
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
def test_head():
    response = primp.head("https://httpbin.org/anything", ca_cert_file=certifi.where())
    assert response.status_code == 200
    assert "content-length" in response.headers


@retry()
def test_options():
    response = primp.options(
        "https://httpbin.org/anything", ca_cert_file=certifi.where()
    )
    assert response.status_code == 200
    assert sorted(response.headers["allow"].split(", ")) == [
        "DELETE",
        "GET",
        "HEAD",
        "OPTIONS",
        "PATCH",
        "POST",
        "PUT",
        "TRACE",
    ]


@retry()
def test_delete():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    response = primp.delete(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
        params=params,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "DELETE"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_post_content():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    content = b"test content"
    response = primp.post(
        "https://httpbin.org/anything",
        auth=auth,
        headers=headers,
        cookies=cookies,
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
def test_post_data():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = primp.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
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
def test_post_json():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = primp.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
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
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
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
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    files = {"file1": temp_file1, "file2": temp_file2}
    response = primp.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
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
def test_patch():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = primp.patch(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
        params=params,
        data=data,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PATCH"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_put():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = primp.put(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        cookies=cookies,
        params=params,
        data=data,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PUT"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}
    # assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert "ccc=ddd" in json_data["headers"]["Cookie"]
    assert "cccc=dddd" in json_data["headers"]["Cookie"]


@retry()
def test_get_impersonate_firefox133():
    response = primp.get(
        "https://tls.peet.ws/api/all",
        #"https://tls.http.rw/api/all",
        impersonate="firefox_133",
        impersonate_os="linux",
    )
    assert response.status_code == 200
    json_data = response.json()
    assert (
        json_data["user_agent"]
        == "Mozilla/5.0 (X11; Ubuntu; Linux x86_64; rv:133.0) Gecko/20100101 Firefox/133.0"
    )
    assert json_data["tls"]["ja4"] == "t13d1716h2_5b57614c22b0_bed828528d07"
    assert (
        json_data["http2"]["akamai_fingerprint_hash"]
        == "6ea73faa8fc5aac76bded7bd238f6433"
    )
    assert json_data["tls"]["peetprint_hash"] == "199f9cf4a47bfc51995a9f3942190094"
