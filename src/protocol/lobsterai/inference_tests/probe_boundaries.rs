//! Explicit opt-in, single text answer and cancellation boundaries for console generation.
use super::super::test_support::{Peer, Reply};
use super::{
    probe::{call, chat},
    *,
};
use crate::provider_runtime::{
    probe_console_target_with_request, ConsoleProbeRequest, ProviderPayloadProbeStatus,
};

async fn fixture(peer: &Peer) -> Fixture {
    Box::pin(Fixture::with_http(
        peer.builder()
            .retry(rquest::retry::Policy::never())
            .build()
            .unwrap(),
        peer.origin.clone(),
    ))
    .await
}
#[tokio::test]
async fn lobsterai_console_probe_default_disabled_wrong_model_and_stale_never_send() {
    Box::pin(async {
        let peer = Peer::start(vec![]).await;
        let f = fixture(&peer).await;
        let target = f
            .state
            .route_config
            .snapshot()
            .select_credential_probe_target("exact")
            .unwrap();
        let options: ConsoleProbeRequest =
            serde_json::from_value(json!({"model":MODEL,"prompt":"only OK"})).unwrap();
        super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), async {
                assert_eq!(
                    probe_console_target_with_request(&f.state, &target, None)
                        .await
                        .1
                        .status,
                    ProviderPayloadProbeStatus::Unsupported
                );
                let mut disabled = target.clone();
                disabled.enabled = false;
                assert_eq!(
                    probe_console_target_with_request(&f.state, &disabled, Some(&options))
                        .await
                        .1
                        .status,
                    ProviderPayloadProbeStatus::Unsupported
                );
                let wrong: ConsoleProbeRequest =
                    serde_json::from_value(json!({"model":"not-configured","prompt":"only OK"}))
                        .unwrap();
                assert_eq!(
                    probe_console_target_with_request(&f.state, &target, Some(&wrong))
                        .await
                        .1
                        .status,
                    ProviderPayloadProbeStatus::Unsupported
                );
                let mut stale = target.clone();
                stale.payload.api_key = "superseded-token".into();
                let result = probe_console_target_with_request(&f.state, &stale, Some(&options))
                    .await
                    .1;
                assert_eq!(result.status, ProviderPayloadProbeStatus::Failed);
                assert!(result.message.contains("call not sent"));
            })
            .await;
        peer.inspect(|calls| assert!(calls.is_empty()));
        assert_eq!(f.audits(1).await[0].status, "failed");
        f.finish().await;
    })
    .await;
}

#[tokio::test]
async fn lobsterai_console_probe_rejects_extra_choices_refusal_tools_truncation_and_oversize() {
    Box::pin(async {
        let mut extra = chat(); extra["choices"].as_array_mut().unwrap().push(chat()["choices"][0].clone());
        let mut refusal = chat(); refusal["choices"][0]["message"]["refusal"] = "denied".into();
        let mut empty = chat(); empty["choices"][0]["message"]["content"] = " ".into();
        let mut truncated = chat(); truncated["choices"][0]["finish_reason"] = "length".into();
        let mut long = chat(); long["choices"][0]["message"]["content"] = "x".repeat(8193).into();
        let mut large = chat(); large["padding"] = "x".repeat(65536).into();
        let refused_sse = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\",\"refusal\":\"denied\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n".to_vec();
        let mut replies: Vec<Reply> = [extra, refusal, empty, truncated, long, large].into_iter().map(Reply::json).collect();
        replies.push(Reply::raw(200, native::reply("chat", true, false, true)));
        replies.push(Reply::raw(200, refused_sse).header("content-type", "text/event-stream"));
        replies.push(Reply::raw(200, native::reply("chat", false, true, false)).header("content-type", "text/event-stream"));
        for reply in replies {
            let peer = Peer::start(vec![reply]).await;
            let f = fixture(&peer).await;
            let report = super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), Box::pin(call(&f))).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed, "{report:?}");
            assert!(report.answer.is_none());
            assert_eq!(f.audits(1).await[0].status, "failed");
            peer.inspect(|calls| assert_eq!(calls.len(), 1));
            f.finish().await;
        }
    }).await;
}

#[tokio::test]
async fn lobsterai_console_probe_unknown_usage_redaction_and_done_terminal() {
    Box::pin(async {
        let mut body = chat(); body.as_object_mut().unwrap().remove("usage");
        body["choices"][0]["message"]["content"] = format!("OK {KEY}").into();
        let done = b"data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n".to_vec();
        for reply in [Reply::json(body), Reply::raw(200, done).header("content-type", "text/event-stream")] {
            let peer = Peer::start(vec![reply]).await;
            let f = fixture(&peer).await;
            let report = super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), Box::pin(call(&f))).await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Passed, "{report:?}");
            assert!(!format!("{report:?}").contains(KEY));
            let rows = f.audits(1).await;
            assert!(rows[0].total_tokens.is_none());
            assert!(!serde_json::to_string(&rows).unwrap().contains(KEY));
            f.finish().await;
        }
    }).await;
}

#[tokio::test]
async fn lobsterai_console_probe_http_errors_do_not_replay() {
    Box::pin(async {
        for status in [307, 403, 429, 500] {
            let peer = Peer::start(vec![Reply::raw(status, KEY.as_bytes().to_vec())
                .header("location", "https://example.invalid/replay")])
            .await;
            let f = fixture(&peer).await;
            let report = super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), Box::pin(call(&f)))
                .await;
            assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
            assert!(!format!("{report:?}").contains(KEY));
            assert!(f.audits(1).await[0].total_tokens.is_none());
            peer.inspect(|calls| assert_eq!(calls.len(), 1));
            f.finish().await;
        }
    })
    .await;
}

#[tokio::test]
async fn lobsterai_console_probe_cancel_preserves_unknown_audit_and_releases_body() {
    Box::pin(async {
        let peer = peer::Peer::new("chat", false, true, 200).await;
        let f = Box::pin(Fixture::new(&peer)).await;
        let mut pending = Box::pin(super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), Box::pin(call(&f))));
        tokio::select! { _ = &mut pending => panic!("must wait for held body"), _ = peer.wait_calls() => {} }
        drop(pending); peer.wait_closed().await;
        let rows = f.audits(1).await;
        assert_eq!(rows[0].status, "cancelled");
        assert!(rows[0].total_tokens.is_none());
        assert!(f.state.local_runtime.as_ref().unwrap().list_model_states(db::CredentialModelStateFilters::default()).await.unwrap().is_empty());
        assert_eq!(f.remaining().await, 2);
        f.finish().await; peer.stop().await;
    }).await;
}
