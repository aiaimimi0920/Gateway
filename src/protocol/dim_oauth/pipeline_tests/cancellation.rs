//! Body cancellation and a dropped pre-response Gemini future must release the real native socket.
use super::*;
#[tokio::test]
async fn dim_public_live_sse_drop_cancels_once_and_does_not_damage_account_health() {
    Box::pin(async {
        for family in ["chat", "messages", "responses"] {
            let peer = peer::Peer::new(family, false, true, 200).await;
            let fixture = Box::pin(Fixture::new(&peer, family)).await;
            super::super::test_support::origin(peer.origin.clone(), async {
                let response = fixture
                    .open("chat/completions", body("chat/completions", true))
                    .await;
                assert_eq!(response.status(), StatusCode::OK);
                let mut body = response.into_body();
                let mut seen = Vec::new();
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !String::from_utf8_lossy(&seen).contains("合成答案") {
                        let frame = body.frame().await.unwrap().unwrap().into_data().unwrap();
                        seen.extend_from_slice(&frame);
                    }
                })
                .await
                .unwrap();
                assert!(!String::from_utf8_lossy(&seen).contains("[DONE]"));
                drop(body);
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "cancelled");
                assert_identity(&rows[0]);
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(fixture.remaining().await, 2);
                assert_wire(&peer, family, 1);
                peer.wait_closed().await;
                let states = fixture
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_model_states(db::CredentialModelStateFilters::default())
                    .await
                    .unwrap();
                assert!(states.iter().all(|s| s.failure_count == 0));
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn dim_public_gemini_aggregation_cancel_releases_inflight_native_read() {
    Box::pin(async {
        let peer=peer::Peer::new("gemini",false,true,200).await;
        let fixture=Box::pin(Fixture::new(&peer,"gemini")).await;
        super::super::test_support::origin(peer.origin.clone(),async {
            let mut request=Box::pin(fixture.open("chat/completions",body("chat/completions",true)));
            tokio::select! {response=&mut request=>panic!("aggregated incomplete Gemini replied early: {}",response.status()), _=peer.wait_calls()=>{}}
            drop(request);peer.wait_closed().await;
            assert_wire(&peer,"gemini",1);
            let rows=fixture.audits(1).await;
            assert_eq!(rows[0].status,"cancelled");assert_identity(&rows[0]);
            assert!(rows[0].total_tokens.is_none());assert_eq!(fixture.remaining().await,2);
            // This is cancellation of the public request future, not a real client TCP-disconnect test.
        }).await;
        fixture.finish().await;peer.stop().await;
    }).await;
}
