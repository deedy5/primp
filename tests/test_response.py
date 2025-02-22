from time import sleep

import pytest

import certifi
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


@retry()
def test_response_stream():
    client = primp.Client(impersonate="chrome_133", impersonate_os="windows")
    resp = client.get("https://nytimes.com")
    for chunk in resp.stream():
        assert len(chunk) > 0