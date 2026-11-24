//! Native Responses survives Hosted account persistence and cannot be enabled by a credential.
use super::*;

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_native_responses() {
    run_hosted(hosted);
}

async fn hosted() {
    let scope = format!("sap-responses-{}", uuid::Uuid::new_v4().simple());
    let inference = peer::Peer::responses_with_unique_ids(scope.clone()).await;
    let writer = build(&scope, "writer", &inference).await;
    let (account, credential) = provision(
        &writer,
        &scope,
        json!({"sap_api_version":"2024-10-21","sap_native_responses":true}),
        json!({"api_key":KEY,"extraBody":{"sap_native_responses":false}}),
    )
    .await;
    let refused_alias = format!("{scope}-credential-only");
    provision(
        &writer,
        &refused_alias,
        json!({"sap_api_version":"2024-10-21"}),
        json!({"api_key":KEY,"extra_body":{"sap_native_responses":true}}),
    )
    .await;
    let reader = build(&scope, "reader", &inference).await;
    test_support::origin(inference.origin.clone(), async {
        for (alias, passes) in [(&refused_alias, false), (&scope, true)] {
            for stream in [false, true] {
                let (status, text) = call(&reader, "/v1/responses", PUBLIC, json!({
                    "model":alias,"input":"合成问题","stream":stream,"reasoning":{"effort":"low"}
                })).await;
                if passes {
                    assert_eq!(status, StatusCode::OK, "{text}");
                    assert!(text.contains("合成答案") && text.contains(&scope), "{text}");
                    if stream {
                        assert!(text.contains("response.completed"), "{text}");
                    }
                } else {
                    assert!(!status.is_success(), "{text}");
                    assert!(inference.calls.lock().unwrap().is_empty());
                    assert!(inference.oauth.lock().unwrap().is_empty());
                }
            }
        }
    })
    .await;
    assert_eq!(inference.oauth.lock().unwrap().len(), 1);
    {
        let calls = inference.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for (path, headers, raw) in calls.iter() {
            assert_eq!(path, "/v2/inference/deployments/deployment1/responses");
            assert_eq!(headers["AI-Resource-Group"], "group-a");
            assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
            assert!(!headers.contains_key("x-management-token"));
            assert!(raw.get("model").is_none());
            assert_eq!(raw["store"], false);
        }
    }
    assert_audits(&writer, &account, &credential, &scope, 2).await;
    finish(writer, reader, inference).await;
}
