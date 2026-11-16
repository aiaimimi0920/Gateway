//! Jev 精确账号探测、持久化重启和取消；不读取真实 Key，不扣普通接入 Key。
use super::*;
use crate::provider_runtime::ProviderPayloadProbeStatus;

fn reply() -> Value {
    json!({"model":UPSTREAM_MODEL,"answers":{"probe":{"type":"noul","noul":0.9,"debug":KEY}},
        "usage":{"input_tokens":10,"output_tokens":1}})
}

#[tokio::test]
async fn commandcode_systemone_probe_persists_catalog_and_redacts_preview_after_restart() {
    let peer = peer::Peer::raw(reply().to_string().into_bytes()).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    let first =
        super::super::test_support::origin(peer.origin.clone(), Box::pin(probe::call(&f))).await;
    assert_eq!(
        first.status,
        ProviderPayloadProbeStatus::Passed,
        "{first:?}"
    );
    assert!(first.answer.as_deref().unwrap().contains("[redacted]"));
    assert!(!format!("{first:?}").contains(KEY));
    let f = Box::pin(f.restart(&peer)).await;
    let second =
        super::super::test_support::origin(peer.origin.clone(), Box::pin(probe::call(&f))).await;
    assert_eq!(
        second.status,
        ProviderPayloadProbeStatus::Passed,
        "{second:?}"
    );
    for row in f.audits(2).await {
        assert_eq!(row.status, "completed");
        assert_eq!(row.total_tokens, Some(11));
        assert!(row.access_key_id.is_none());
        assert_eq!(
            row.route_trace.as_ref().unwrap()["realCredentialRef"],
            "exact"
        );
        assert!(!serde_json::to_string(&row).unwrap().contains(KEY));
    }
    assert_eq!(f.remaining().await, 2);
    let calls = peer.calls.lock().unwrap().clone();
    assert_eq!(calls.len(), 2);
    for (path, headers, body) in calls {
        assert_eq!(path, "/provider/v1/systemone");
        assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
        assert_eq!(body["state"], "only OK");
        assert_eq!(body["model"], UPSTREAM_MODEL);
        assert_eq!(body["questions"]["probe"]["type"], "noul");
        assert!(body.get("messages").is_none() && body.get("stream").is_none());
    }
    f.finish().await;
    peer.stop().await;
}

#[tokio::test]
async fn commandcode_systemone_probe_invalid_answers_fail_without_replay() {
    for broken in [
        json!({}),
        json!({"other":{"type":"noul","noul":0.9}}),
        json!({"probe":{"type":"noul","noul":1.1}}),
    ] {
        let mut raw = reply();
        raw["answers"] = broken;
        let peer = peer::Peer::raw(raw.to_string().into_bytes()).await;
        let f = Box::pin(Fixture::systemone(&peer)).await;
        let report =
            super::super::test_support::origin(peer.origin.clone(), Box::pin(probe::call(&f)))
                .await;
        assert_eq!(
            report.status,
            ProviderPayloadProbeStatus::Failed,
            "{report:?}"
        );
        assert!(report.answer.is_none());
        assert!(!format!("{report:?}").contains(KEY));
        assert_eq!(f.audits(1).await[0].status, "failed");
        assert_eq!(peer.calls.lock().unwrap().len(), 1);
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
        peer.stop().await;
    }
}

#[tokio::test]
async fn commandcode_systemone_probe_cancel_releases_body_without_healing_model() {
    let peer = peer::Peer::new("chat", false, true, 200).await;
    let f = Box::pin(Fixture::systemone(&peer)).await;
    let mut pending = Box::pin(super::super::test_support::origin(
        peer.origin.clone(),
        probe::call(&f),
    ));
    tokio::select! { _ = &mut pending => panic!("must wait for body"), _ = peer.wait_calls() => {} }
    drop(pending);
    peer.wait_closed().await;
    assert_eq!(f.audits(1).await[0].status, "cancelled");
    let states = f
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    assert!(states.iter().all(|state| state.failure_count == 0));
    assert_eq!(f.remaining().await, 2);
    f.finish().await;
    peer.stop().await;
}
