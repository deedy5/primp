import json
import pytest

from pyreqwest_impersonate import Client


def test_client_get_custom_headers():
    headers = {"X-Test": "test"}
    client = Client(headers=headers, verify=False)
    response = client.get("https://httpbin.org/get")
    json_data = json.loads(response.text)
    assert response.status_code == 200
    assert "httpbin.org" in response.url
    assert json_data["headers"]["X-Test"] == "test"


def test_client_get_impersonate():
    client = Client(impersonate="chrome_123", verify=False)
    response = client.get("https://tls.peet.ws/api/all")
    json_data = json.loads(response.text)
    assert response.status_code == 200
    assert json_data["http_version"] == "h2"
    assert json_data["tls"]["ja4"].startswith("t13d")
    assert json_data["http2"]["akamai_fingerprint_hash"] == "90224459f8bf70b7d0a8797eb916dbc9"

   

