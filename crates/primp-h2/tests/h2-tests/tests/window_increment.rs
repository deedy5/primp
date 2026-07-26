//! Tests for the per-stream initial WINDOW_UPDATE sent after HEADERS
//! (`initial_stream_window_size_increment`).
//!
//! Regression: when several streams are opened before the driver flushes,
//! each stream's WINDOW_UPDATE must be sent. A single-slot implementation
//! overwrote the earlier stream's update, so it never reached the wire.

use h2_support::prelude::*;

#[tokio::test]
async fn initial_window_update_sent_for_each_stream_opened_before_driver_flush() {
    h2_support::trace_init!();
    let (io, mut srv) = mock::new();

    let srv = async move {
        let settings = srv.assert_client_handshake().await;
        assert_default_settings!(settings);
        srv.recv_frame(
            frames::headers(1)
                .request("GET", "https://example.com/")
                .eos(),
        )
        .await;
        srv.recv_frame(
            frames::headers(3)
                .request("GET", "https://example.com/")
                .eos(),
        )
        .await;
        // Both streams were opened before the driver flushed; each stream's
        // initial WINDOW_UPDATE must be sent, not overwritten.
        srv.recv_frame(frames::window_update(1, 12451840)).await;
        srv.recv_frame(frames::window_update(3, 12451840)).await;
        srv.send_frame(frames::headers(1).response(200).eos()).await;
        srv.send_frame(frames::headers(3).response(200).eos()).await;
    };

    let h2 = async move {
        let mut builder = client::Builder::new();
        builder.initial_stream_window_size_increment(12451840);
        let (mut client, mut h2) = builder.handshake::<_, Bytes>(io).await.unwrap();

        let request = || {
            Request::builder()
                .method(Method::GET)
                .uri("https://example.com/")
                .body(())
                .unwrap()
        };
        // Queue both requests before the driver runs so both streams exist
        // before the first WINDOW_UPDATE flush.
        let (response1, _) = client.send_request(request(), true).unwrap();
        let (response2, _) = client.send_request(request(), true).unwrap();

        h2.drive(response1).await.unwrap();
        h2.drive(response2).await.unwrap();
    };

    join(srv, h2).await;
}
