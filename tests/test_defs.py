from time import sleep

import pyreqwest_impersonate as pri


def retry(max_retries=5, delay=1):
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
    params = {"x": "aaa", "y": "bbb"}
    response = pri.request(
        "GET",
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


@retry()
def test_get():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    response = pri.get(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "GET"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


@retry()
def test_head():
    response = pri.head("https://httpbin.org/anything", verify=False)
    assert response.status_code == 200
    assert "content-length" in response.headers


@retry()
def test_options():
    response = pri.options("https://httpbin.org/anything", verify=False)
    assert response.status_code == 200
    assert sorted(response.headers["allow"].split(", ")) == ['DELETE', 'GET', 'HEAD', 'OPTIONS', 'PATCH', 'POST', 'PUT', 'TRACE']


@retry()
def test_delete():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    response = pri.delete(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "DELETE"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert "Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.text
    assert b"Bearer bearerXXXXXXXXXXXXXXXXXXXX" in response.content


@retry()
def test_post_content():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    content = b"test content"
    response = pri.post(
        "https://httpbin.org/anything",
        auth=auth,
        headers=headers,
        params=params,
        content=content,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["data"] == "test content"


@retry()
def test_post_data():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = pri.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


@retry()
def test_post_data2():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": ["value2_1", "value2_2"]}
    response = pri.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": ["value2_1", "value2_2"]}
    

@retry()
def test_post_json():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = pri.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        json=data,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["json"] == data


@retry()
def test_client_post_files():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    files = {"file1": b"aaa111", "file2": b"bbb222"}
    response = pri.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        files=files,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["files"] == {"file1": "aaa111", "file2": "bbb222"}


@retry()
def test_patch():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = pri.patch(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PATCH"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


@retry()
def test_put():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    response = pri.put(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
        verify=False,
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PUT"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["form"] == {"key1": "value1", "key2": "value2"}


@retry()
def test_get_impersonate_chrome126():
    response = pri.get("https://tls.peet.ws/api/all", impersonate="chrome_126", verify=False)
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["http_version"] == "h2"
    assert json_data["tls"]["ja4"].startswith("t13d")
    assert (
        json_data["http2"]["akamai_fingerprint_hash"]
        == "90224459f8bf70b7d0a8797eb916dbc9"
    )