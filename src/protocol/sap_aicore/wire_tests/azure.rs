//! Exercise the documented Azure EU20 Cloud origin through the actual public SAP Chat path.
use super::{super::test_support, *};

#[tokio::test]
async fn sap_aicore_azure_eu20_public_json_and_sse() {
    Box::pin(async {
        let peer = peer::Peer::chat_with_unique_ids("azure-eu20".into()).await;
        let fixture = Fixture::with_target(
            &peer,
            &test_support::azure_base(),
            &test_support::azure_key(),
        )
        .await;
        test_support::origin(peer.origin.clone(), async {
            for stream in [false, true] {
                let response = fixture
                    .open("chat/completions", body("chat/completions", stream))
                    .await;
                let status = response.status();
                let text = drain(response.into_body()).await;
                assert_eq!(status, StatusCode::OK, "{text}");
                assert!(text.contains("合成答案"));
                assert_eq!(text.contains("[DONE]"), stream);
            }
            for row in fixture.audits(2).await {
                assert_eq!(row.status, "completed");
                assert_eq!(row.total_tokens, Some(5));
                assert_identity(&row);
            }
            assert_wire(&peer, 2, "chat/completions");
            assert_eq!(fixture.remaining().await, 0);
        })
        .await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
