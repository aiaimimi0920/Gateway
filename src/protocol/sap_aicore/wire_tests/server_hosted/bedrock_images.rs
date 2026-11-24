//! Hosted routing must persist Bedrock image capability at deployment scope.
use super::*;
use crate::protocol::sap_aicore::wire_tests::bedrock::{events, media::PNG, reply};

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_bedrock_inline_images() {
    run_hosted(hosted);
}

async fn hosted() {
    let scope = format!("sap-bedrock-images-{}", uuid::Uuid::new_v4().simple());
    let inference = peer::Peer::raw_sequence(vec![reply(), events()]).await;
    let writer = build(&scope, "writer", &inference).await;
    let (account, credential) = provision(
        &writer,
        &scope,
        json!({"sap_bedrock_converse":true,"sap_bedrock_image_input":true}),
        json!({"api_key":KEY,"extra_body":{"sap_bedrock_image_input":false}}),
    )
    .await;
    let refused_alias = format!("{scope}-credential-only");
    let (refused_account, _) = provision(
        &writer,
        &refused_alias,
        json!({"sap_bedrock_converse":true}),
        json!({"api_key":KEY,"extra_body":{"sap_bedrock_image_input":true}}),
    )
    .await;
    let reader = build(&scope, "reader", &inference).await;
    test_support::origin(inference.origin.clone(), async {
        let (status, text) = infer_image(&reader, &refused_alias, false).await;
        assert!(!status.is_success(), "{text}");
        assert!(!text.contains(PNG));
        assert!(inference.calls.lock().unwrap().is_empty());
        assert!(inference.oauth.lock().unwrap().is_empty());
        for stream in [false, true] {
            let (status, text) = infer_image(&reader, &scope, stream).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            assert!(text.contains(&scope) && !text.contains(PNG), "{text}");
            if stream {
                assert!(text.contains("data: [DONE]"), "{text}");
            }
        }
    })
    .await;
    assert_eq!(inference.oauth.lock().unwrap().len(), 1);
    {
        let calls = inference.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for ((path, headers, raw), op) in calls.iter().zip(["converse", "converse-stream"]) {
            assert_eq!(path, &format!("/v2/inference/deployments/deployment1/{op}"));
            assert_eq!(headers["authorization"], "Bearer synthetic-sap-access");
            assert_eq!(headers["AI-Resource-Group"], "group-a");
            assert!(!headers.contains_key("cookie") && !headers.contains_key("x-management-token"));
            assert_eq!(
                raw["messages"],
                json!([{"role":"user","content":[
                    {"text":"inspect"},{"image":{"format":"png","source":{"bytes":PNG}}}
                ]}])
            );
            assert!(raw.get("sap_bedrock_image_input").is_none());
        }
    }
    assert_audits(&writer, &account, &credential, &scope, 2).await;
    let completed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM gateway_request_audits WHERE provider_account_id = $1 AND status = 'completed'",
    )
    .bind(refused_account)
    .fetch_one(writer.pg_pool.as_ref().unwrap())
    .await
    .unwrap();
    assert_eq!(completed, 0);
    finish(writer, reader, inference).await;
}

async fn infer_image(state: &Arc<AppState>, alias: &str, stream: bool) -> (StatusCode, String) {
    call(
        state,
        "/v1/chat/completions",
        PUBLIC,
        json!({"model": alias,"stream":stream,
            "messages":[{"role":"user","content":[{"type":"text","text":"inspect"},
                {"type":"image_url","image_url":{"url":format!("data:image/png;base64,{PNG}")}}
            ]}]
        }),
    )
    .await
}
