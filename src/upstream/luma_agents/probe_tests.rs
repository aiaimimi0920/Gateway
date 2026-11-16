//! Explicit paid media tests must retain exact credentials without changing ordinary Key balances.
use super::{pipeline_fixture, test_support};
use crate::provider_runtime::*;
use pipeline_fixture::{Fixture, IMAGE, VIDEO};
use serde_json::{json, Value};
use test_support::{job, within, Reply, Server, ID};

pub(super) fn options(operation: &str, model: &str) -> Value {
    json!({"prompt":"合成媒体测试","model":model,"credentialIds":["exact"],
        "media":{"operation":operation,"confirmCharge":true,"parameters":{}}})
}
#[test]
fn luma_agents_probe_request_requires_explicit_cost_operation_and_one_exact_account() {
    let valid = options("image", IMAGE);
    assert!(ConsoleProbeRequest::parse(valid.to_string().as_bytes()).is_ok());
    for change in [
        json!({"media":{"operation":"image","confirmCharge":false}}),
        json!({"media":{"operation":"image"}}),
        json!({"media":{"operation":"chat","confirmCharge":true}}),
        json!({"model":null}),
        json!({"credentialIds":null}),
        json!({"credentialIds":["exact","unrelated"]}),
        json!({"scope":{"kind":"pool"}}),
        json!({"planId":"saved"}),
        json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"model":"other"}}}),
    ] {
        let mut invalid = valid.clone();
        invalid
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        assert!(
            ConsoleProbeRequest::parse(invalid.to_string().as_bytes()).is_err(),
            "{invalid}"
        );
    }
}
#[tokio::test]
async fn luma_agents_probe_media_operations_use_one_exact_sender_and_native_audit_without_key_charge(
) {
    Box::pin(async {
        for (kind, model, resolved, credits) in [
            ("image", IMAGE, "uni-1", Value::Null),
            ("image_edit", IMAGE, "uni-1", json!(0)),
            ("video", VIDEO, "ray-3.2", json!(0.25)),
        ] {
            let mut queued = job("queued");
            queued["type"] = json!(kind);
            queued["model"] = json!(resolved);
            let mut completed = queued.clone();
            completed["state"] = json!("completed");
            completed["usage"]["credits_charged"] = credits.clone();
            completed["output"] =
                json!([{"type":if kind=="video"{"video"}else{"image"},"url":test_support::ASSET}]);
            let server = Server::start(vec![
                Reply::json(201, queued),
                Reply::json(200, completed),
                if kind == "video" {
                    Reply::mp4()
                } else {
                    Reply::png()
                },
            ])
            .await;
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let target = fixture
                .state
                .route_config
                .snapshot()
                .select_credential_probe_target("exact")
                .unwrap();
            let mut input = options(kind, model);
            if kind == "image_edit" {
                input["media"]["parameters"]["source"] = json!({"generation_id":ID});
            }
            let options = ConsoleProbeRequest::parse(input.to_string().as_bytes())
                .unwrap()
                .unwrap();
            within(&server.origin, async {
                let (_, report) =
                    probe_console_target_with_request(&fixture.state, &target, Some(&options))
                        .await;
                assert_eq!(
                    report.status,
                    ProviderPayloadProbeStatus::Passed,
                    "{}",
                    report.message
                );
                assert!(report.answer.is_none(), "media must not be scored as text");
                let row = fixture.audit().await;
                assert_eq!(row.status, "completed");
                assert_eq!(row.response_id, ID);
                assert_eq!(row.resolved_model.as_deref(), Some(resolved));
                assert!(row.total_tokens.is_none());
                let trace = row.route_trace.as_ref().unwrap();
                assert_eq!(trace["realCredentialRef"], "exact");
                assert_eq!(
                    trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
                    credits
                );
                assert_eq!(fixture.balance().await.remaining_messages, Some(2));
                let calls = server.calls();
                assert_eq!(calls.len(), 3);
                assert_eq!(calls[0].body["type"], kind);
                assert_eq!(calls[0].body["model"], resolved);
                assert_eq!(
                    calls[0].headers["authorization"],
                    format!("Bearer {}", pipeline_fixture::KEY)
                );
                assert_eq!(calls[1].path, format!("/v1/generations/{ID}"));
                assert!(!calls[2].headers.contains_key("authorization"));
                let persisted = serde_json::to_string(&row).unwrap();
                assert!(
                    !persisted.contains(pipeline_fixture::KEY)
                        && !persisted.contains("signature=private")
                );
            })
            .await;
            fixture.finish().await;
            server.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn luma_agents_probe_default_and_text_tests_never_implicitly_generate_media() {
    Box::pin(async {
        let server = Server::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        let target = fixture
            .state
            .route_config
            .snapshot()
            .select_credential_probe_target("exact")
            .unwrap();
        within(&server.origin, async {
            let text = ConsoleProbeRequest::parse(
                br#"{"prompt":"OK","model":"public-luma-image","credentialIds":["exact"]}"#,
            )
            .unwrap()
            .unwrap();
            for request in [None, Some(&text)] {
                let (_, report) =
                    probe_console_target_with_request(&fixture.state, &target, request).await;
                assert_eq!(report.status, ProviderPayloadProbeStatus::Unsupported);
            }
            assert!(server.calls().is_empty());
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
