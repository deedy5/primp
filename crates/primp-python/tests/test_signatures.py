"""`proxy` stays last for positional binding."""

import inspect

import primp

FNS = ["get", "head", "options", "delete", "post", "put", "patch", "request"]


def test_proxy_is_last_param():
    for name in FNS:
        params = list(inspect.signature(getattr(primp, name)).parameters)
        assert params[-1] == "proxy", f"{name}: tail is {params[-3:]}"
        assert params.index("proxy") > params.index("stream"), name


def test_old_positionals_bind_as_before():
    # Positional order preserved.
    sig = inspect.signature(primp.get)
    bound = sig.bind(
        "http://x",
        None,  # params
        None,  # headers
        None,  # cookies
        None,  # content
        None,  # data
        None,  # json
        None,  # files
        None,  # auth
        None,  # auth_bearer
        None,  # timeout
        None,  # connect_timeout
        None,  # read_timeout
        None,  # dns_timeout
        None,  # impersonate
        None,  # impersonate_os
        None,  # verify
        None,  # ca_cert_file
        True,  # follow_redirects
        False,  # stream
    )
    bound.apply_defaults()
    assert bound.arguments["follow_redirects"] is True
    assert bound.arguments["stream"] is False
    assert bound.arguments["proxy"] is None
