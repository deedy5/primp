from time import sleep
from urllib.parse import parse_qs

import pyreqwest_impersonate as pri


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
def test_get():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    response = pri.get(
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


@retry()
def test_head():
    response = pri.head("https://httpbin.org/anything")
    assert response.status_code == 200
    assert "content-length" in response.headers


@retry()
def test_options():
    response = pri.options("https://httpbin.org/anything")
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
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "POST"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    received_data_dict = parse_qs(json_data["data"])
    assert received_data_dict == {"key1": ["value1"], "key2": ["value2"]}


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
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PATCH"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    received_data_dict = parse_qs(json_data["data"])
    assert received_data_dict == {"key1": ["value1"], "key2": ["value2"]}


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
    )
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["method"] == "PUT"
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    received_data_dict = parse_qs(json_data["data"])
    assert received_data_dict == {"key1": ["value1"], "key2": ["value2"]}