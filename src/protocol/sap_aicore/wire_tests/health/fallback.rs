//! A releasable OAuth barrier proves zero-send rejection without timing races.
use super::*;
use crate::pipeline::{stage_send, PipelineContext};
use crate::protocol::sap_aicore::test_support;
use std::sync::atomic::Ordering;

const FALLBACK: &str = "ordinary-fallback";
const FALLBACK_MODEL: &str = "fallback-model";
const FALLBACK_KEY: &str = "synthetic-fallback-key";

fn add_fallback(fixture: &Fixture, peer: &peer::Peer) {
    let snapshot = fixture.state.route_config.snapshot();
    let mut doc = serde_json::to_value(snapshot.document()).unwrap();
    doc["providers"].as_array_mut().unwrap().push(json!({
        "id":FALLBACK,"adapter":"openai_compatible","protocol_family":"openai",
        "base_url":format!("{}/v1",peer.origin),
        "explicit_credentials_only":true,"request_wait":{"mode":"disabled"},
        "supported_models":[MODEL],"model_map":{MODEL:FALLBACK_MODEL},
        "credentials":[{"id":"fallback-exact","api_key":FALLBACK_KEY,"supported_models":[MODEL]}]
    }));
    doc["model_routes"] = json!([{"pattern":MODEL,"provider_ids":["sap_aicore-public",FALLBACK]}]);
    fixture
        .state
        .route_config
        .replace_document(serde_json::from_value(doc).unwrap())
        .unwrap();
}

async fn revoke_before_token(fixture: &Fixture, peer: &peer::Peer) {
    peer.wait_oauth().await;
    assert!(peer.calls.lock().unwrap().is_empty());
    let snapshot = fixture.state.route_config.snapshot();
    let mut document = snapshot.document().clone();
    let exact = document.providers[0]
        .credentials
        .iter_mut()
        .find(|item| item.id.as_deref() == Some("exact"))
        .unwrap();
    exact.enabled = Some(false);
    // Use the real validated route owner, not a synthetic send error or a timer.
    fixture
        .state
        .route_config
        .replace_document(document)
        .unwrap();
    peer.oauth_release.add_permits(1);
}

fn assert_rejection(trace: &Value) {
    let rejected = trace["rejectedCandidates"].as_array().unwrap();
    let matching: Vec<_> = rejected
        .iter()
        .filter(|row| {
            row["providerAccountId"] == "sap_aicore-public" && row["realCredentialRef"] == "exact"
        })
        .collect();
    assert_eq!(matching.len(), 1, "{trace}");
    assert_eq!(matching[0]["resolvedModel"], UPSTREAM_MODEL);
    assert_eq!(matching[0]["reason"], "upstream_credential_unavailable");
}

async fn public_fallback(stream: bool) {
    let peer = peer::Peer::new("chat", false, false, 200).await;
    peer.oauth_hold.store(true, Ordering::SeqCst);
    let fixture = Box::pin(Fixture::new(&peer)).await;
    add_fallback(&fixture, &peer);
    test_support::origin(peer.origin.clone(), async {
        let (reply, ()) = tokio::join!(
            fixture.open("chat/completions", body("chat/completions", stream)),
            revoke_before_token(&fixture, &peer)
        );
        assert_eq!(reply.status(), StatusCode::OK);
        let output = drain(reply.into_body()).await;
        assert!(output.contains("合成答案"), "{output}");
        if stream {
            let mut done = 0;
            let mut frames = 0;
            for data in output
                .lines()
                .filter_map(|line| line.strip_prefix("data: "))
            {
                if data == "[DONE]" {
                    done += 1;
                } else {
                    assert_eq!(done, 0, "output after the terminal");
                    let frame: Value = serde_json::from_str(data).unwrap();
                    assert_eq!(frame["model"], MODEL);
                    frames += 1;
                }
            }
            assert!(frames > 0);
            assert_eq!(done, 1);
        } else {
            let reply: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(reply["model"], MODEL);
        }
        let rows = fixture.audits(1).await;
        let row = &rows[0];
        assert_eq!(row.status, "completed");
        assert_eq!(row.upstream_status, Some(200));
        assert_eq!(row.provider_account_id.as_deref(), Some(FALLBACK));
        assert_eq!(row.resolved_model.as_deref(), Some(FALLBACK_MODEL));
        assert_eq!(row.total_tokens, Some(5));
        let trace = row.route_trace.as_ref().unwrap();
        assert_eq!(trace["routeAttemptCount"], 1);
        assert_eq!(trace["attemptedProviderIds"], json!([FALLBACK]));
        assert_eq!(trace["realCredentialRef"], "fallback-exact");
        assert_eq!(trace["selectedCandidate"]["providerAccountId"], FALLBACK);
        assert_eq!(trace["selectedCandidate"]["resolvedModel"], FALLBACK_MODEL);
        assert_eq!(
            trace["selectedCandidate"]["realCredentialRef"],
            "fallback-exact"
        );
        assert_eq!(peer.oauth.lock().unwrap().len(), 1);
        {
            let calls = peer.calls.lock().unwrap();
            assert_eq!(calls.len(), 1, "zero SAP inference and one ordinary send");
            assert_eq!(calls[0].0, "/v1/chat/completions");
            assert_eq!(
                calls[0].1["authorization"],
                format!("Bearer {FALLBACK_KEY}")
            );
            assert_eq!(calls[0].2["model"], FALLBACK_MODEL);
        }
        assert_eq!(fixture.remaining().await, 1);
        assert!(!states(&fixture)
            .await
            .iter()
            .any(|row| row.failure_count > 0));
        assert_rejection(trace);
        let saved = serde_json::to_string(row).unwrap();
        assert!(!saved.contains(KEY) && !saved.contains(FALLBACK_KEY));
    })
    .await;
    fixture.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn sap_aicore_unsent_next_to_ordinary_json_keeps_exact_facts() {
    Box::pin(public_fallback(false)).await;
}

#[tokio::test]
async fn sap_aicore_unsent_next_to_ordinary_sse_keeps_exact_facts() {
    Box::pin(public_fallback(true)).await;
}

#[tokio::test]
async fn sap_aicore_unsent_next_restores_each_previous_observation() {
    Box::pin(async {
        for stream in [false, true] {
            for previous in [None, Some(None), Some(Some(201))] {
                let peer = peer::Peer::new("chat", false, false, 200).await;
                peer.oauth_hold.store(true, Ordering::SeqCst);
                let fixture = Box::pin(Fixture::new(&peer)).await;
                let request = crate::protocol::openai::normalize_chat_completions(body(
                    "chat/completions",
                    stream,
                ))
                .unwrap();
                let mut ctx = PipelineContext::new(request, None);
                ctx.candidates = fixture
                    .state
                    .route_config
                    .snapshot()
                    .resolve_candidates(Some(MODEL));
                assert_eq!(ctx.candidates.len(), 1);
                ctx.observed_upstream_status = previous;
                test_support::origin(peer.origin.clone(), async {
                    let (result, ()) = tokio::join!(
                        Box::pin(stage_send::run(&mut ctx, &fixture.state)),
                        revoke_before_token(&fixture, &peer)
                    );
                    assert_eq!(
                        result.err().unwrap().code.as_deref(),
                        Some("upstream_credential_unavailable")
                    );
                })
                .await;
                assert_eq!(ctx.observed_upstream_status, previous);
                assert_eq!(ctx.route_attempt_count(), 0);
                assert!(ctx.attempted_provider_ids().is_empty());
                assert!(peer.calls.lock().unwrap().is_empty());
                assert_eq!(peer.oauth.lock().unwrap().len(), 1);
                assert_eq!(fixture.remaining().await, 2);
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
