//! Real sender, bounded response and cancellation evidence for explicit Agnes international console tests.
use super::*;
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};
async fn call(f: &Fixture) -> crate::provider_runtime::ProviderPayloadProbeReport {
    let target = f
        .state
        .route_config
        .snapshot()
        .select_credential_probe_target("exact")
        .unwrap();
    let request: ConsoleProbeRequest =
        serde_json::from_value(json!({"model":MODEL,"prompt":"only OK"})).unwrap();
    probe_console_target_with_request(&f.state, &target, Some(&request))
        .await
        .1
}
fn response(text: &str) -> Value {
    json!({"id":"synthetic","model":"agnes-live-text","choices":[{"index":0,"message":{"role":"assistant","content":text},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})
}
#[tokio::test]
async fn agnes_console_probe_redacts_secrets_and_records_real_usage() {
    Box::pin(async {
        let text = format!("OK {}", KEY);
        let peer = peer::Peer::raw(response(&text).to_string().into_bytes()).await;
        let f = Fixture::new(&peer).await;
        super::super::test_support::origin(peer.origin.clone(), async {
            for _ in 0..2 {
                let report = call(&f).await;
                assert_eq!(report.status, ProviderPayloadProbeStatus::Passed);
                let text = format!("{:?}", report);
                assert!(!text.contains(KEY), "{text}");
                assert!(report.answer.unwrap().contains("[redacted]"));
            }
        })
        .await;
        let audits = f.audits(2).await;
        for row in audits {
            assert_eq!(row.status, "completed");
            assert_eq!(row.total_tokens, Some(5));
            assert_eq!(row.route_trace.unwrap()["realCredentialRef"], "exact");
            assert!(row.access_key_id.is_none());
        }
        assert_eq!(f.remaining().await, 2);
        assert_wire(&peer, 2, "chat/completions");
        for (_, _, raw) in peer.calls.lock().unwrap().iter() {
            assert_eq!(raw["messages"][0]["content"], "only OK");
            assert_eq!(raw["max_tokens"], 256);
            assert_eq!(raw["stream"], false);
        }
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
#[tokio::test]
async fn agnes_console_probe_rejects_incomplete_or_oversized_replies() {
    Box::pin(async {
        let mut cases = vec![b"bad-json".to_vec(), vec![b'x';65_537], br#"{"choices":[],"choices":[]}"#.to_vec()];
        for kind in ["empty","length","refusal","tool","usage","large"] {
            let mut raw=response("OK");
            match kind {
                "empty" => raw["choices"][0]["message"]["content"]=json!(" "),
                "length" => raw["choices"][0]["finish_reason"]=json!("length"),
                "refusal" => raw["choices"][0]["message"]["refusal"]=json!("no"),
                "tool" => {raw["choices"][0]["finish_reason"]=json!("tool_calls");raw["choices"][0]["message"]["tool_calls"]=json!([{"id":"c1","type":"function","function":{"name":"f","arguments":"{}"}}]);},
                "usage" => raw["usage"]["total_tokens"]=json!(99),
                _ => raw["choices"][0]["message"]["content"]=json!("a".repeat(8193)),
            }
            cases.push(raw.to_string().into_bytes());
        }
        for bytes in cases {
            let peer=peer::Peer::raw(bytes).await; let f=Fixture::new(&peer).await;
            let report=super::super::test_support::origin(peer.origin.clone(),call(&f)).await;
            assert_eq!(report.status,ProviderPayloadProbeStatus::Failed,"{report:?}");
            assert!(report.answer.is_none());
            let audits=f.audits(1).await;
            assert_eq!(audits[0].status,"failed"); assert!(audits[0].total_tokens.is_none());
            assert_wire(&peer,1,"chat/completions"); assert_eq!(f.remaining().await,2);
            f.finish().await;peer.stop().await;
        }
    }).await;
}
#[tokio::test]
async fn agnes_console_probe_unknown_usage_stays_unknown() {
    Box::pin(async {
        let mut raw = response("OK");
        raw.as_object_mut().unwrap().remove("usage");
        let peer = peer::Peer::raw(raw.to_string().into_bytes()).await;
        let f = Fixture::new(&peer).await;
        let report = super::super::test_support::origin(peer.origin.clone(), call(&f)).await;
        assert_eq!(report.status, ProviderPayloadProbeStatus::Passed);
        let rows = f.audits(1).await;
        assert!(rows[0].total_tokens.is_none());
        f.finish().await;
        peer.stop().await;
    })
    .await;
}
#[tokio::test]
async fn agnes_console_probe_errors_never_replay_or_leak() {
    Box::pin(async {
        for status in [307, 401, 403, 429, 500] {
            let peer = peer::Peer::new("chat", false, false, status).await;
            let f = Fixture::new(&peer).await;
            let report = super::super::test_support::origin(peer.origin.clone(), call(&f)).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
            assert!(!format!("{report:?}").contains("synthetic-private-error"));
            assert_eq!(f.audits(1).await[0].status, "failed");
            assert_wire(&peer, 1, "chat/completions");
            f.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn agnes_console_probe_cancel_releases_body_and_never_heals() {
    Box::pin(async {
        let peer=peer::Peer::new("chat",false,true,200).await;let f=Fixture::new(&peer).await;
        let future=super::super::test_support::origin(peer.origin.clone(),call(&f));
        let mut pending=Box::pin(future);
        tokio::select! {_ = &mut pending => panic!("must wait for body"), _ = peer.wait_calls() => {}}
        drop(pending);peer.wait_closed().await;
        assert_eq!(f.audits(1).await[0].status,"cancelled");
        let states=f.state.local_runtime.as_ref().unwrap().list_model_states(db::CredentialModelStateFilters::default()).await.unwrap();
        assert!(states.iter().all(|s| s.failure_count==0));assert_eq!(f.remaining().await,2);
        f.finish().await;peer.stop().await;
    }).await;
}
