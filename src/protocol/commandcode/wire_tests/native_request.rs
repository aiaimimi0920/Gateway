//! Public prepaid ingress must send native request structures, not only pass packer tests.
use super::*;

#[tokio::test]
async fn commandcode_native_request_public_json_and_sse_preserve_rich_wire() {
    for endpoint in ["responses", "messages"] {
        for stream in [false, true] {
            let peer = peer::Peer::new(endpoint, false, false, 200).await;
            let fixture = if endpoint == "responses" {
                Box::pin(Fixture::native_responses(&peer)).await
            } else {
                Box::pin(Fixture::native_messages(&peer)).await
            };
            super::super::test_support::origin(peer.origin.clone(), async {
                let mut raw = super::super::native_request_tests::raw(endpoint);
                raw["model"] = MODEL.into();
                raw["stream"] = stream.into();
                let response = fixture.open(endpoint, raw.clone()).await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "completed");
                assert_eq!(rows[0].total_tokens, Some(5));
                assert_eq!(fixture.remaining().await, 1);
                let calls = peer.calls.lock().unwrap();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].0, format!("/provider/v1/{endpoint}"));
                assert_eq!(calls[0].1["authorization"], format!("Bearer {KEY}"));
                raw["model"] = UPSTREAM_MODEL.into();
                assert_eq!(calls[0].2, raw);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    }
}
