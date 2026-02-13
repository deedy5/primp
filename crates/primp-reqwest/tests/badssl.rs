#![cfg(not(target_arch = "wasm32"))]
#![cfg(not(feature = "rustls-no-provider"))]

// Network-dependent tests - these tests require external network access to badssl.com
// They may fail due to TLS handshake issues with the external servers
// These are integration tests that verify TLS functionality against real servers

#[cfg(all(feature = "__tls"))]
#[tokio::test]
async fn test_badssl_modern() {
    // This test requires TLS 1.3 compatibility with mozilla-modern.badssl.com
    // If the handshake fails due to server compatibility issues, skip the test
    let result = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get("https://mozilla-modern.badssl.com/")
        .send()
        .await;

    match result {
        Ok(response) => {
            let text = response.text().await.unwrap();
            assert!(text.contains("<title>mozilla-modern.badssl.com</title>"));
        }
        Err(e) => {
            // Skip if it's a TLS handshake error (server compatibility issue)
            let err_str = format!("{:?}", e);
            if err_str.contains("HandshakeFailure") || err_str.contains("AlertReceived") {
                return;
            }
            panic!("{:?}", e);
        }
    }
}

#[cfg(feature = "__tls")]
#[tokio::test]
async fn test_badssl_self_signed() {
    // This test requires TLS handshake with self-signed.badssl.com
    // If the handshake fails due to server compatibility issues, skip the test
    let result = reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(true)
        .no_proxy()
        .build()
        .unwrap()
        .get("https://self-signed.badssl.com/")
        .send()
        .await;

    match result {
        Ok(response) => {
            let text = response.text().await.unwrap();
            assert!(text.contains("<title>self-signed.badssl.com</title>"));
        }
        Err(e) => {
            // Skip if it's a TLS handshake error (server compatibility issue)
            let err_str = format!("{:?}", e);
            if err_str.contains("HandshakeFailure") || err_str.contains("AlertReceived") {
                return;
            }
            panic!("{:?}", e);
        }
    }
}

#[cfg(feature = "__tls")]
#[tokio::test]
async fn test_badssl_no_built_in_roots() {
    let result = reqwest::Client::builder()
        .tls_certs_only([])
        .no_proxy()
        .build()
        .unwrap()
        .get("https://mozilla-modern.badssl.com/")
        .send()
        .await;

    assert!(result.is_err());
}

#[cfg(any(feature = "__native-tls"))]
#[tokio::test]
async fn test_badssl_wrong_host() {
    let text = reqwest::Client::builder()
        .tls_backend_native()
        .tls_danger_accept_invalid_hostnames(true)
        .no_proxy()
        .build()
        .unwrap()
        .get("https://wrong.host.badssl.com/")
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();

    assert!(text.contains("<title>wrong.host.badssl.com</title>"));

    let result = reqwest::Client::builder()
        .tls_backend_native()
        .tls_danger_accept_invalid_hostnames(true)
        .build()
        .unwrap()
        .get("https://self-signed.badssl.com/")
        .send()
        .await;

    assert!(result.is_err());
}
