from time import sleep

from pyreqwest_impersonate import Client


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
def test_client_init_params():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    client = Client(auth=auth, params=params, headers=headers, verify=False)
    response = client.get("https://httpbin.org/anything")
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}


@retry()
def test_client_request_get():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
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


@retry()
def test_client_get():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
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


@retry()
def test_client_post_content():
    client = Client(verify=False)
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    content = b"test content"
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


@retry()
def test_client_post_data():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
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


@retry()
def test_client_post_data2():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": ["value2_1", "value2_2"]}
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
    assert json_data["form"] == {"key1": "value1", "key2": ["value2_1", "value2_2"]}


@retry()
def test_client_post_json():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
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


@retry()
def test_client_post_files():
    client = Client(verify=False)
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    files = {"file1": b"aaa111", "file2": b"bbb222"}
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


@retry()
def test_client_impersonate_chrome126():
    client = Client(impersonate="chrome_126", verify=False)
    response = client.get("https://tls.peet.ws/api/all")
    assert response.status_code == 200
    json_data = response.json()
    assert json_data["http_version"] == "h2"
    assert json_data["tls"]["ja4"].startswith("t13d")
    assert (
        json_data["http2"]["akamai_fingerprint_hash"]
        == "90224459f8bf70b7d0a8797eb916dbc9"
    )
