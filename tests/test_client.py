import pytest
from pyreqwest_impersonate import Client

def test_client_creation():
    client = Client()
    assert isinstance(client, Client)

def test_client_request_get():
    client = Client()
    response = client.request("GET", "https://httpbin.org/get")
    assert response.status_code == 200
    assert "httpbin.org" in response.url