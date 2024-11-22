from time import sleep

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
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["data"] == "test content"


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["json"] == data


@retry()
def test_client_post_files():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    cookies = {"ccc": "ddd", "cccc": "dddd"}
    params = {"x": "aaa", "y": "bbb"}
    files = {"file1": b"aaa111", "file2": b"bbb222"}
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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["files"] == {"file1": "aaa111", "file2": "bbb222"}


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


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
    assert json_data["headers"]["Cookie"] == "ccc=ddd; cccc=dddd"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


@retry()
def test_get_impersonate_chrome130():
    response = primp.get(
        # "https://tls.peet.ws/api/clean",
        "https://tls.http.rw/api/clean",
        impersonate="chrome_130",
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["ja4"] == "t13d1516h2_8daaf6152771_b1ff8ab2d16f"
    assert json_data["akamai_hash"] == "90224459f8bf70b7d0a8797eb916dbc9"
    assert json_data["peetprint_hash"] == "b8ce945a4d9a7a9b5b6132e3658fe033"


@retry()
def test_get_impersonate_chrome131():
    response = primp.get(
        # "https://tls.peet.ws/api/clean",
        "https://tls.http.rw/api/clean",
        impersonate="chrome_131",
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["ja4"] == "t13d1516h2_8daaf6152771_b1ff8ab2d16f"
    assert json_data["akamai_hash"] == "90224459f8bf70b7d0a8797eb916dbc9"
    assert json_data["peetprint_hash"] == "7466733991096b3f4e6c0e79b0083559"