//! One explicitly charged account uses the ordinary validated-image sender, not a public Key.
use super::{pipeline_fixture::*, test_support::*};
use crate::provider_runtime::*;
use serde_json::{json, Value};

pub(super) fn options() -> Value {
    json!({"prompt":"合成图片测试","model":MODEL,"credentialIds":["exact"],
        "media":{"operation":"image","confirmCharge":true,"parameters":{}}})
}
pub(super) fn parsed(value: Value) -> ConsoleProbeRequest {
    ConsoleProbeRequest::parse(value.to_string().as_bytes())
        .unwrap()
        .unwrap()
}
pub(super) fn completed() -> Vec<Reply> {
    vec![
        Reply::json(json!({"status":"pending","jobId":"synthetic-probe-job"})),
        Reply::json(json!({"status":"completed","data":{"imageUrl":ASSET}})),
        Reply::png(),
    ]
}
#[tokio::test]
async fn chataibot_probe_exact_generate_edit_merge_validate_images_without_key_charge() {
    Box::pin(async {
        for (count, path) in [
            (0, "/api/image/generate"),
            (1, "/api/file/recognize"),
            (4, "/api/file/merge"),
        ] {
            let server = Server::start(completed()).await;
            let fixture = Box::pin(Fixture::new(5)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let mut input = options();
            if count > 0 {
                input["media"]["operation"] = json!("image_edit");
                input["media"]["parameters"]["images"] =
                    json!(vec!["data:image/png;base64,aGVsbG8="; count]);
            }
            let input = parsed(input);
            within(
                &server.origin,
                Box::pin(async {
                    let (_, report) = Box::pin(probe_console_target_with_request(
                        &fixture.state,
                        &target,
                        Some(&input),
                    ))
                    .await;
                    assert_eq!(
                        report.status,
                        ProviderPayloadProbeStatus::Passed,
                        "{}",
                        report.message
                    );
                    assert!(report.answer.is_none());
                    assert!(report.health_recovery.is_some());
                    assert!(report.message.contains("扣额未知"));
                    let row = fixture.audit().await;
                    assert_eq!(row.status, "completed");
                    assert_eq!(row.protocol_family, "chataibot_web");
                    assert_eq!(row.requested_model.as_deref(), Some(MODEL));
                    assert_eq!(row.resolved_model.as_deref(), Some(RESOLVED));
                    assert_eq!(row.model_alias.as_deref(), Some(MODEL));
                    assert!(row.total_tokens.is_none() && row.access_key_id.is_none());
                    let trace = row.route_trace.as_ref().unwrap();
                    assert_eq!(trace["realCredentialRef"], "exact");
                    assert_eq!(
                        trace["chataibotImage"],
                        json!({"verified":true,"count":1,"supplierCharge":null})
                    );
                    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                    let calls = server.calls();
                    assert_eq!(calls.len(), 3);
                    assert_eq!(calls[0].path, path);
                    assert_eq!(calls[1].path, "/api/jobs/synthetic-probe-job");
                    for call in &calls[..2] {
                        assert!(call.headers["cookie"].to_str().unwrap().contains(TOKEN));
                        assert!(!call.headers.contains_key("authorization"));
                    }
                    assert!(!calls[2].headers.contains_key("cookie"));
                    let audit = serde_json::to_string(&row).unwrap();
                    for secret in [TOKEN, "synthetic-private", "aGVsbG8=", "合成图片测试"] {
                        assert!(!audit.contains(secret), "{audit}");
                    }
                }),
            )
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn chataibot_probe_default_and_text_never_generate_or_read_quota() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(5)).await;
        let target = fixture
            .state
            .route_config
            .snapshot()
            .select_credential_probe_target("exact")
            .unwrap();
        let text = parsed(json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}));
        within(
            &server.origin,
            Box::pin(async {
                for input in [None, Some(&text)] {
                    let (_, report) = Box::pin(probe_console_target_with_request(
                        &fixture.state,
                        &target,
                        input,
                    ))
                    .await;
                    assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
                }
                assert!(server.calls().is_empty());
            }),
        )
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
