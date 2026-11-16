//! 管理授权、四种计划来源及持久结果通过实际 router 和产品 sender 验证。
use super::super::test_support::{Peer, Reply};
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json};
const PATH: &str = "/v1/internal/gateway/console/providers/lobsterai-public/probe";
fn policy() -> Value {
    json!({"automaticEnabled":false,"intervalMinutes":15,"modelSelection":"selected","models":[MODEL],
        "cases":[{"id":"text","name":"Text","prompt":"hello","expectedAnswer":"合成答案","difficulty":1,"enabled":true}]})
}
async fn post(f: &Fixture, path: &str, body: Value, grant: Option<&str>) -> (StatusCode, Value) {
    let response = Box::pin(send_console_json(
        &f.state,
        Method::POST,
        path,
        body,
        grant,
        None,
    ))
    .await;
    (response.status(), parse_json(response).await)
}
#[tokio::test]
async fn lobsterai_management_probe_grant_plans_and_persisted_results() {
    Box::pin(async {
        let peer = Peer::start((0..4).map(|_| Reply::raw(200, native::reply("chat", false, false, true))).collect()).await;
        let f = Box::pin(Fixture::with_probe_policy(
            peer.builder().retry(rquest::retry::Policy::never()).build().unwrap(),
            peer.origin.clone(), UPSTREAM_MODEL,
            json!([{"modelId":UPSTREAM_MODEL,"apiFormat":"openai","supportsToolCalling":true}]),
            false, Some(policy()),
        )).await;
        let revision = f.state.route_config.snapshot().revision().id().to_owned();
        let grant = grant_console_secret_access(&f.state).await;
        for (body, grant, expected) in [
            (json!({"model":MODEL,"prompt":"hello","credentialIds":["exact"]}), None, StatusCode::FORBIDDEN),
            (json!({"model":MODEL,"prompt":"hello","credentialIds":["foreign"]}), Some(grant.as_str()), StatusCode::BAD_REQUEST),
        ] {
            let (status, body) = post(&f, PATH, body, grant).await;
            assert_eq!(status, expected, "{body}");
        }
        peer.inspect(|calls| assert!(calls.is_empty()));
        super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), Box::pin(async {
            for body in [
                json!({"model":MODEL,"prompt":"hello","credentialIds":["exact"]}),
                json!({"testPlan":policy(),"scope":{"kind":"account","id":"exact"},"credentialIds":["exact"]}),
                json!({"planId":"named","credentialIds":["exact"]}),
            ] {
                let (status, body) = post(&f, PATH, body, Some(&grant)).await;
                assert_eq!(status, StatusCode::OK, "{body}");
                assert_eq!(body["result"]["results"][0]["status"], "passed", "{body}");
                assert!(!body.to_string().contains(KEY));
            }
            let response = Box::pin(support::probe_credential(&f.state, &grant, "exact")).await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = parse_json(response).await;
            assert_eq!(body["result"]["status"], "passed", "{body}");
            assert_eq!(body["result"]["assessment"]["policySource"], "account", "{body}");
        })).await;
        let (status, results) = post(&f, &format!("{PATH}/results"), json!({"includePlans":true}), Some(&grant)).await;
        assert_eq!(status, StatusCode::OK, "{results}");
        assert!(results.to_string().contains("named"), "{results}");
        assert!(!results.to_string().contains(KEY));
        let (status, _) = post(&f, &format!("{PATH}/results"), json!({}), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        peer.inspect(|calls| {
            assert_eq!(calls.len(), 4);
            for call in calls {
                assert_eq!(call.path, "/api/proxy/v1/chat/completions");
                assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
                assert_eq!(call.body["model"], UPSTREAM_MODEL);
                assert_eq!(call.body["max_tokens"], 256);
            }
        });
        let audits = f.audits(4).await;
        assert!(audits.iter().all(|r| r.status == "completed" && r.total_tokens == Some(5)));
        assert!(audits.iter().all(|r| r.route_trace.as_ref().unwrap()["realCredentialRef"] == "exact"));
        assert_eq!(f.remaining().await, 2);
        assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
        f.finish().await;
    }).await;
}
#[tokio::test]
async fn lobsterai_management_probe_without_policy_or_with_wrong_model_never_sends() {
    Box::pin(async {
        let peer = Peer::start(vec![]).await;
        let f = Box::pin(Fixture::with_http(
            peer.builder()
                .retry(rquest::retry::Policy::never())
                .build()
                .unwrap(),
            peer.origin.clone(),
        ))
        .await;
        let grant = grant_console_secret_access(&f.state).await;
        super::super::transport::TEST_ORIGIN
            .scope(
                peer.origin.clone(),
                Box::pin(async {
                    let response =
                        Box::pin(support::probe_credential(&f.state, &grant, "exact")).await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let body = parse_json(response).await;
                    assert_eq!(body["result"]["status"], "unsupported", "{body}");
                    let (status, body) = post(
                        &f,
                        PATH,
                        json!({"model":"other","prompt":"hello","credentialIds":["exact"]}),
                        Some(&grant),
                    )
                    .await;
                    assert_eq!(status, StatusCode::OK);
                    assert_eq!(
                        body["result"]["results"][0]["status"], "unsupported",
                        "{body}"
                    );
                }),
            )
            .await;
        peer.inspect(|calls| assert!(calls.is_empty()));
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}
