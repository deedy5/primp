import json
from urllib.parse import parse_qs


import pytest

from pyreqwest_impersonate import Client


def test_client_init_params():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    client = Client(auth=auth, params=params, headers=headers, verify=False)
    response = client.get("https://httpbin.org/anything")
    assert response.status_code == 200
    json_data = json.loads(response.text)
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}


def test_client_get():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    client = Client(verify=False)
    response = client.get(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
    )
    assert response.status_code == 200
    json_data = json.loads(response.text)
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}


def test_client_post_content():
    auth = ("user", "password")
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    content = b"test content"
    client = Client(verify=False)
    response = client.post(
        "https://httpbin.org/anything",
        auth=auth,
        headers=headers,
        params=params,
        content=content,
    )
    assert response.status_code == 200
    json_data = json.loads(response.text)
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Basic dXNlcjpwYXNzd29yZA=="
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    assert json_data["data"] == "test content"


def test_client_post_data():
    auth_bearer = "bearerXXXXXXXXXXXXXXXXXXXX"
    headers = {"X-Test": "test"}
    params = {"x": "aaa", "y": "bbb"}
    data = {"key1": "value1", "key2": "value2"}
    client = Client(verify=False)
    response = client.post(
        "https://httpbin.org/anything",
        auth_bearer=auth_bearer,
        headers=headers,
        params=params,
        data=data,
    )
    assert response.status_code == 200
    json_data = json.loads(response.text)
    assert json_data["headers"]["X-Test"] == "test"
    assert json_data["headers"]["Authorization"] == "Bearer bearerXXXXXXXXXXXXXXXXXXXX"
    assert json_data["args"] == {"x": "aaa", "y": "bbb"}
    received_data_dict = parse_qs(json_data["data"])
    assert received_data_dict == {"key1": ["value1"], "key2": ["value2"]}


def test_client_get_impersonate():
    client = Client(impersonate="chrome_123", verify=False)
    response = client.get("https://tls.peet.ws/api/all")
    json_data = json.loads(response.text)
    assert response.status_code == 200
    assert json_data["http_version"] == "h2"
    assert json_data["tls"]["ja4"].startswith("t13d")
    assert (
        json_data["http2"]["akamai_fingerprint_hash"]
        == "90224459f8bf70b7d0a8797eb916dbc9"
    )
