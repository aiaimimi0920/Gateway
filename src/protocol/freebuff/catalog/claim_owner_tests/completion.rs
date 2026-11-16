//! Original SQLite claim -> START -> exact signed completion -> decode -> reported-credit FINISH.
use super::super::execution::CatalogCompletionResponse;
use super::*;
use crate::local_runtime::freebuff_sessions::ActiveAdmission;

pub(super) fn answer() -> Value {
    json!({"id":"answer","model":"reported-model","choices":[{"index":0,
        "message":{"role":"assistant","content":"answer"},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":3,"completion_tokens":4,"total_tokens":7,"cost":0.25,
        "cost_details":{"upstream_inference_cost":0.125}}})
}
pub(super) async fn activate(
    db: &LocalRuntime,
    requests: &BusinessRequests<'_>,
    peer: &fixture::Peer,
) -> ActiveAdmission {
    let prepared = pending(db, requests).await;
    let claim = prepared.claim().clone();
    drop(prepared);
    peer.push(fixture::Spec::json(200, active(&claim, "open")));
    let scope = requests.recovery_requests();
    with_test_origin(
        peer.origin.clone(),
        recovery(db, &scope).await.heartbeat(&scope, BUDGET),
    )
    .await
    .unwrap()
}
pub(super) async fn send<'a>(
    owner: &'a mut ActiveAdmission,
    requests: &'a BusinessRequests<'a>,
    peer: &fixture::Peer,
) -> CatalogCompletionResponse<'a> {
    with_test_origin(peer.origin.clone(),async {
        owner.start_catalog_run(requests,BUDGET).await.unwrap().prepare_completion(
            json!({"model":"fbm1.synthetic-open","messages":[{"role":"user","content":"synthetic"}]}),4096)
            .unwrap().send(BUDGET).await.unwrap()
    }).await
}

#[tokio::test]
async fn freebuff_catalog_completion_json_and_sse_finish_one_reported_step_without_losing_claim() {
    for (sse, zero) in [(false, false), (true, false), (false, true), (true, true)] {
        let (root, db) = setup().await;
        let (session, device) = account();
        let requests = session.business_requests(&device, "open").unwrap();
        let peer = fixture::Peer::start(vec![]).await;
        let mut owner = activate(&db, &requests, &peer).await;
        let original = row(&db).await;
        peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
        let credits = if zero { 0 } else { 40 };
        let mut response = answer();
        if zero {
            response["usage"]["cost"] = json!(0);
            response["usage"]["cost_details"]["upstream_inference_cost"] = json!(0);
        }
        let mut spec = fixture::Spec::json(200, response.clone());
        if sse {
            spec.headers.insert(
                "content-type",
                "text/event-stream; charset=utf-8".parse().unwrap(),
            );
            spec.body=format!("data: {}\n\ndata: [DONE]\n\n",json!({"id":"answer","model":"reported-model",
                "choices":[{"index":0,"delta":{"role":"assistant","content":"answer"},"finish_reason":"stop"}],
                "usage":response["usage"]})).into_bytes();
        }
        peer.push(spec);
        peer.push(fixture::Spec::json(204, json!(null)));
        let decoded = send(&mut owner, &requests, &peer)
            .await
            .decode(BUDGET)
            .await;
        assert_eq!(
            decoded.completion().unwrap()["choices"][0]["message"]["content"],
            "answer"
        );
        assert_eq!(decoded.accounting().sdk_credits().unwrap(), credits);
        let other = LocalRuntime::open(&root).await.unwrap();
        assert_eq!(
            other
                .recover_freebuff_admission("pool", "account", &requests.recovery_requests())
                .await
                .err()
                .unwrap()
                .code
                .as_deref(),
            Some("freebuff_session_claim_busy")
        );
        let finished = with_test_origin(peer.origin.clone(), decoded.finish_single_step(BUDGET))
            .await
            .unwrap();
        assert_eq!(finished.finish_http_status, 204);
        assert!(finished.completion.is_ok());
        assert_eq!(finished.accounting.sdk_credits().unwrap(), credits);
        assert_eq!(
            row(&db).await,
            original,
            "FINISH does not end/refund the session"
        );
        {
            let hits = peer.hits.lock().unwrap();
            assert_eq!(hits.len(), 4);
            let finish = &hits[3];
            assert_eq!(finish.uri.path(), "/api/v1/agent-runs");
            let body: Value = serde_json::from_slice(&finish.body).unwrap();
            assert_eq!(
                body,
                json!({"action":"FINISH","runId":"original-run","status":"completed", "totalSteps":1,"directCredits":credits,"totalCredits":credits,"steps":[]})
            );
            assert_eq!(finish.headers["authorization"], "Bearer synthetic-account");
            assert_eq!(finish.headers["content-type"], "text/plain;charset=UTF-8");
            assert!(!finish.headers.contains_key("x-freebuff-instance-id"));
            assert!(!finish.headers.contains_key("x-freebuff-device-signature"));
        }
        drop(owner);
        drop(recovery(&other, &requests.recovery_requests()).await);
        other.close().await;
        peer.close().await;
        cleanup(&root, db).await;
    }
}

#[tokio::test]
async fn freebuff_catalog_completion_unknown_cost_never_synthesizes_zero_finish() {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    let original = row(&db).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    let mut body = answer();
    body["usage"].as_object_mut().unwrap().remove("cost");
    body["usage"]
        .as_object_mut()
        .unwrap()
        .remove("cost_details");
    peer.push(fixture::Spec::json(200, body));
    let decoded = send(&mut owner, &requests, &peer)
        .await
        .decode(BUDGET)
        .await;
    assert!(decoded.completion().is_ok());
    assert_eq!(
        decoded.finish_single_step(BUDGET).await.err(),
        Some(BusinessError::AccountingUnknown)
    );
    assert_eq!(peer.count(), 3);
    assert_eq!(row(&db).await, original);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}

#[tokio::test]
async fn freebuff_catalog_completion_semantic_error_preserves_reported_partial_credits_and_safe_finish(
) {
    let (root, db) = setup().await;
    let (session, device) = account();
    let requests = session.business_requests(&device, "open").unwrap();
    let peer = fixture::Peer::start(vec![]).await;
    let mut owner = activate(&db, &requests, &peer).await;
    peer.push(fixture::Spec::json(200, json!({"runId":"original-run"})));
    peer.push(fixture::Spec::json(
        200,
        json!({"error":{"message":"synthetic-secret"},"usage":{"cost":0.25}}),
    ));
    peer.push(fixture::Spec::json(
        200,
        json!({"not_a_settlement_receipt":true}),
    ));
    let decoded = send(&mut owner, &requests, &peer)
        .await
        .decode(BUDGET)
        .await;
    assert!(decoded.completion().is_err());
    assert_eq!(decoded.accounting().sdk_credits().unwrap(), 26);
    let finished = with_test_origin(peer.origin.clone(), decoded.finish_single_step(BUDGET))
        .await
        .unwrap();
    assert!(finished.completion.is_err());
    assert_eq!(finished.finish_http_status, 200);
    let hits = peer.hits.lock().unwrap();
    let finish: Value = serde_json::from_slice(&hits[3].body).unwrap();
    assert_eq!(finish["status"], "failed");
    assert_eq!(finish["directCredits"], 26);
    assert_eq!(finish["totalCredits"], 26);
    assert!(!String::from_utf8(hits[3].body.clone())
        .unwrap()
        .contains("synthetic-secret"));
    drop(hits);
    drop(owner);
    peer.close().await;
    cleanup(&root, db).await;
}
