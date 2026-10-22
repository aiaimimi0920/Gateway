use super::*;
use crate::{protocol::responses::normalize_responses, upstream::client::UpstreamClient};
use serde_json::json;

fn payload(profile: &str) -> ProviderAccountPayload {
    serde_json::from_value(
        json!({"adapter":"anthropic_compatible","base_url":"https://relay.example",
        "api_key":"synthetic-cpa-key","request_client_profile":profile}),
    )
    .unwrap()
}

#[test]
fn cpa_full_mode_is_opt_in_and_requires_dispatch_context() {
    let req = normalize_responses(json!({"model":"claude-opus-5","input":"hello"})).unwrap();
    assert!(UpstreamClient::build_request_plan(
        &payload("cpa_claude_code"),
        &req,
        "claude-opus-5",
        false
    )
    .is_err());
    let plan =
        UpstreamClient::build_request_plan(&payload("cpa"), &req, "claude-opus-5", false).unwrap();
    assert!(plan.body.unwrap().pointer("/metadata/user_id").is_none());
}

#[tokio::test]
async fn cpa_full_profile_keeps_body_and_session_header_consistent() {
    context::within(
        context::Context {
            caller_scope: "authenticated-caller".into(),
            ..Default::default()
        },
        async {
            let req = normalize_responses(
                json!({"model":"claude-opus-5","input":"hello","temperature":0.3}),
            )
            .unwrap();
            for stream in [false, true] {
                let p = payload("cpa_claude_code");
                let plan =
                    UpstreamClient::build_request_plan(&p, &req, "claude-opus-5", stream).unwrap();
                let mut headers = crate::upstream::headers::build_inference_headers(
                    &p,
                    &req,
                    "claude-opus-5",
                    None,
                );
                finalize_cpa_full_headers(&p, &plan, &mut headers).unwrap();
                let body = plan.body.as_ref().unwrap();
                assert_eq!(
                    body.as_object()
                        .unwrap()
                        .keys()
                        .take(4)
                        .map(String::as_str)
                        .collect::<Vec<_>>(),
                    ["model", "max_tokens", "messages", "metadata"]
                );
                assert_eq!(body["temperature"], 0.3);
                assert_eq!(
                    headers["x-claude-code-session-id"],
                    gateway_cpa_claude_profile::session_header(body).unwrap()
                );
                assert_eq!(headers["user-agent"], "claude-cli/2.1.280 (external, cli)");
                assert!(headers["anthropic-beta"]
                    .to_str()
                    .unwrap()
                    .contains("oauth-2025-04-20"));
                assert_eq!(plan.url, "https://relay.example/v1/messages");
                assert!(plan.query.contains(&("beta".into(), "true".into())));
            }
        },
    )
    .await;
}
