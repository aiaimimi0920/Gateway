//! OAuth preparation rechecks mutable eligibility without charging the provider window twice.
use super::*;
use crate::pipeline::{run_pipeline, PipelineContext, PipelineOutput};
use crate::protocol::sap_aicore::test_support;
use futures::StreamExt;

#[tokio::test]
async fn sap_aicore_oauth_recheck_consumes_one_provider_admission() {
    Box::pin(async {
        for stream in [false, true] {
            let peer = peer::Peer::new("chat", false, false, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            test_support::origin(peer.origin.clone(), async {
                let context = || {
                    let request = crate::protocol::openai::normalize_chat_completions(body(
                        "chat/completions",
                        stream,
                    ))
                    .unwrap();
                    let mut ctx = PipelineContext::new(request, Some(fixture.token.clone()));
                    ctx.route_policy_config = Some(db::GatewayRoutePolicyConfig {
                        rate_limit_enforcement_version: Some("v1".into()),
                        provider_attempt_rate_limit: Some(db::GatewayRateLimitDefinition {
                            window_seconds: 60,
                            max_requests: 1,
                        }),
                        ..Default::default()
                    });
                    ctx
                };
                let output = run_pipeline(context(), &fixture.state).await.unwrap();
                match output {
                    PipelineOutput::Json(value) => {
                        assert!(!stream);
                        assert!(value.to_string().contains("合成答案"));
                    }
                    PipelineOutput::Sse(mut source) => {
                        assert!(stream);
                        let mut bytes = Vec::new();
                        while let Some(chunk) = source.next().await {
                            bytes.extend_from_slice(&chunk.unwrap());
                        }
                        let text = String::from_utf8(bytes).unwrap();
                        assert!(text.contains("合成答案") && text.contains("[DONE]"));
                    }
                    PipelineOutput::Binary(_) => panic!("Unexpected binary response"),
                }
                let rows = fixture.audits(1).await;
                assert_eq!(rows[0].status, "completed");
                assert_eq!(rows[0].upstream_status, Some(200));
                assert_identity(&rows[0]);
                assert_eq!(
                    rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                    1
                );
                assert_eq!(fixture.remaining().await, 1);
                let error = match run_pipeline(context(), &fixture.state).await {
                    Err(error) => error,
                    Ok(_) => panic!("Second request must exhaust the provider window"),
                };
                assert_eq!(error.http_status, Some(429));
                assert_eq!(error.code.as_deref(), Some("rate_limit_exceeded"));
                let rows = fixture.audits(2).await;
                let rejected = rows.iter().find(|row| row.status == "failed").unwrap();
                assert!(rejected.provider_account_id.is_none());
                assert!(rejected.upstream_status.is_none());
                assert!(rejected.route_trace.as_ref().unwrap()["realCredentialRef"].is_null());
                assert_eq!(
                    rejected.route_trace.as_ref().unwrap()["routeAttemptCount"],
                    0
                );
                assert_eq!(peer.calls.lock().unwrap().len(), 1);
                assert_eq!(peer.oauth.lock().unwrap().len(), 1);
                assert_eq!(fixture.remaining().await, 1);
            })
            .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
