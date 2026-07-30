mod support;
use support::server;

#[tokio::test]
#[should_panic(expected = "test server should not panic")]
async fn server_panics_should_propagate() {
    let server = server::http(|_| async {
        panic!("kaboom");
    });

    let _ = primp::get(format!("http://{}/ci", server.addr())).await;
}
