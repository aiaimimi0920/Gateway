//! Exact-account probes select one declared native route before sending, never retry another wire.
use super::*;
use crate::provider_runtime::ProviderPayloadProbeStatus;

async fn fixture(peer: &peer::Peer, family: &str) -> Fixture {
    if family == "messages" {
        Fixture::native_messages(peer).await
    } else {
        Fixture::native_responses(peer).await
    }
}
fn wire(peer: &peer::Peer, family: &str) {
    let calls = peer.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    let (path, headers, body) = &calls[0];
    assert_eq!(path, &format!("/provider/v1/{family}"));
    assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
    assert!(!headers.contains_key("cookie") && !headers.contains_key("x-api-key"));
    assert_eq!(body["model"], UPSTREAM_MODEL);
    assert_eq!(body["stream"], false);
    assert!(!body.to_string().contains("gateway_commandcode_catalog"));
    if family == "messages" {
        assert_eq!(body["messages"][0]["content"], "only OK");
        assert_eq!(body["max_tokens"], 256);
    } else {
        assert!(body["input"].to_string().contains("only OK"));
        assert_eq!(body["max_output_tokens"], 256);
    }
}

#[tokio::test]
async fn commandcode_native_probe_uses_declared_route_and_records_usage_without_charging_key() {
    Box::pin(async {
        for family in ["messages", "responses"] {
            let peer = peer::Peer::raw(native::reply(family, false, false, true)).await;
            let f = Box::pin(fixture(&peer, family)).await;
            let report =
                super::super::test_support::origin(peer.origin.clone(), probe::call(&f)).await;
            assert_eq!(
                report.status,
                ProviderPayloadProbeStatus::Passed,
                "{report:?}"
            );
            assert_eq!(report.answer.as_deref(), Some("合成答案"));
            let rows = f.audits(1).await;
            assert_eq!(rows[0].status, "completed");
            assert_eq!(rows[0].total_tokens, Some(5));
            assert_eq!(
                rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
                "exact"
            );
            assert!(rows[0].access_key_id.is_none());
            assert_eq!(f.remaining().await, 2);
            wire(&peer, family);
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn commandcode_native_probe_redacts_secret_and_keeps_unknown_usage_unknown() {
    Box::pin(async {
        for family in ["messages", "responses"] {
            let mut raw: Value =
                serde_json::from_slice(&native::reply(family, false, false, true)).unwrap();
            raw.as_object_mut().unwrap().remove("usage");
            let bytes = raw
                .to_string()
                .replace("合成答案", &format!("OK {KEY}"))
                .into_bytes();
            let peer = peer::Peer::raw(bytes).await;
            let f = Box::pin(fixture(&peer, family)).await;
            let report =
                super::super::test_support::origin(peer.origin.clone(), probe::call(&f)).await;
            assert_eq!(
                report.status,
                ProviderPayloadProbeStatus::Passed,
                "{report:?}"
            );
            assert_eq!(report.answer.as_deref(), Some("OK [redacted]"));
            let rows = f.audits(1).await;
            assert_eq!(rows[0].status, "completed");
            assert!(rows[0].total_tokens.is_none());
            assert!(!serde_json::to_string(&rows).unwrap().contains(KEY));
            wire(&peer, family);
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn commandcode_native_probe_rejects_bad_replies_without_cross_protocol_replay() {
    Box::pin(async {
        for family in ["messages", "responses"] {
            let base: Value =
                serde_json::from_slice(&native::reply(family, false, false, true)).unwrap();
            let mut incomplete = base.clone();
            if family == "messages" {
                incomplete["stop_reason"] = "max_tokens".into();
            } else {
                incomplete["status"] = "incomplete".into();
            }
            let mut usage = base.clone();
            usage["usage"]["input_tokens"] = json!(-1);
            let mut cases = vec![
                b"bad-json".to_vec(),
                vec![b'x'; 65_537],
                native::reply(family, true, false, true),
                incomplete.to_string().into_bytes(),
                usage.to_string().into_bytes(),
            ];
            if family == "responses" {
                let mut bad = base;
                bad["usage"]["total_tokens"] = json!(99);
                cases.push(bad.to_string().into_bytes());
            }
            for bytes in cases {
                let peer = peer::Peer::raw(bytes).await;
                let f = Box::pin(fixture(&peer, family)).await;
                let report =
                    super::super::test_support::origin(peer.origin.clone(), probe::call(&f)).await;
                assert_eq!(
                    report.status,
                    ProviderPayloadProbeStatus::Failed,
                    "{report:?}"
                );
                assert!(report.answer.is_none());
                let rows = f.audits(1).await;
                assert_eq!(rows[0].status, "failed");
                assert!(rows[0].total_tokens.is_none());
                assert_eq!(f.remaining().await, 2);
                wire(&peer, family);
                f.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
