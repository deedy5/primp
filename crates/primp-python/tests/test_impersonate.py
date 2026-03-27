"""
Impersonate tests that require internet access.

These tests make requests to tls.browserleaks.com to verify browser impersonation
functionality. They require an active internet connection.
"""

from time import sleep

import pytest
import primp


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


@pytest.mark.internet
@retry()
def test_client_impersonate_chrome144():
    client = primp.Client(
        impersonate="chrome_144",
        impersonate_os="windows",
    )
    #response = client.get("https://tls.peet.ws/api/all")
    #response = client.get("https://tls.http.rw/api/all")
    response = client.get("https://tls.browserleaks.com/json")
    assert response.status_code == 200
    json_data = response.json()
    assert (
        json_data["user_agent"]
        == "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/144.0.0.0 Safari/537.36"
    )
    assert json_data["ja4"] == "t13d1516h2_8daaf6152771_d8a2da3f94cd"
    assert json_data["akamai_hash"] == "52d84b11737d980aef856699f885ca86"


@pytest.mark.internet
@retry()
def test_get_impersonate_safari18_5():
    response = primp.get(
        #"https://tls.peet.ws/api/all",
        #"https://tls.http.rw/api/all",
        "https://tls.browserleaks.com/json",
        impersonate="safari_18.5",
        impersonate_os="ios",
    )
    assert response.status_code == 200
    json_data = response.json()
    assert (
        json_data["user_agent"]
        == "Mozilla/5.0 (iPhone; CPU iPhone OS 18_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.5 Mobile/15E148 Safari/604.1"
    )
    assert json_data["ja4"] == "t13d2014h2_a09f3c656075_e42f34c56612"
    assert json_data["akamai_hash"] == "c52879e43202aeb92740be6e8c86ea96"