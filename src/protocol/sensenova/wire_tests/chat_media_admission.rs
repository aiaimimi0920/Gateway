//! Undocumented Chat media must not bypass admission as opaque Raw content.
use super::*;

#[tokio::test]
async fn sensenova_chat_unknown_media_rejects_before_send_without_poisoning_account() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, false, 200).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            let mut count = 0;
            for stream in [false, true] {
                for role in ["user", "assistant", "system", "tool"] {
                    for part in [
                        json!({"type":"video_url","video_url":{"url":"https://media.invalid/clip.mp4"}}),
                        json!({"type":"input_audio","input_audio":{"data":"AAAB","format":"wav"}}),
                        json!({"type":"file","file":{"file_id":"file-unverified"}}),
                        json!({"type":"future_media","payload":"opaque"}),
                        json!({"type":"text","text":17}),
                    ] {
                        let request = json!({"model":MODEL,"stream":stream,"messages":[
                            {"role":role,"tool_call_id":"call-test","content":[
                                {"type":"text","text":"inspect"},part]}]});
                        let reply = fixture.open("chat/completions", request).await;
                        let status = reply.status();
                        let output = drain(reply.into_body()).await;
                        assert_eq!(status, StatusCode::BAD_REQUEST, "{role}/{stream}: {output}");
                        assert!(output.contains("sensenova_chat_content_unsupported"), "{output}");
                        count += 1;
                    }
                }
            }
            let rows = fixture.audits(count).await;
            for row in &rows {
                assert_eq!(row.status, "failed");
                assert!(row.provider_account_id.is_none() && row.upstream_status.is_none());
                let trace = row.route_trace.as_ref().unwrap();
                assert_eq!(trace["routeAttemptCount"], 0);
                assert!(trace["realCredentialRef"].is_null());
                assert_eq!(trace["attemptedProviderIds"], json!([]));
                let rejected = trace["rejectedCandidates"].as_array().unwrap();
                assert_eq!(rejected.iter().filter(|row|
                    row["providerAccountId"] == "sensenova-public"
                    && row["realCredentialRef"] == "exact"
                    && row["resolvedModel"] == UPSTREAM_MODEL
                    && row["reason"] == "sensenova_chat_content_unsupported"
                ).count(), 1, "{trace}");
            }
            assert_eq!(fixture.remaining().await, 2);
            assert!(peer.calls.lock().unwrap().is_empty());
            assert!(fixture.state.local_runtime.as_ref().unwrap()
                .list_model_states(db::CredentialModelStateFilters::default()).await.unwrap().is_empty());
            // Consecutive refusals must not poison the otherwise healthy account.
            for stream in [false, true] {
                let reply = fixture.open("chat/completions", body("chat/completions", stream)).await;
                assert_eq!(reply.status(), StatusCode::OK);
                assert!(drain(reply.into_body()).await.contains("合成答案"));
            }
            let rows = fixture.audits(count + 2).await;
            assert_eq!(rows.iter().filter(|row| row.status == "completed").count(), 2);
            assert_eq!(peer.calls.lock().unwrap().len(), 2);
            assert_eq!(fixture.remaining().await, 0);
        }).await;
        fixture.finish().await;
        peer.stop().await;
    }).await;
}
