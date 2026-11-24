//! Full console authorization and saved-account lifecycle using the existing local runtime fixture.
use super::{
    files_tests::metadata,
    pipeline_fixture::{support::*, *},
    test_support::*,
};
use axum::http::{Method, StatusCode};
use serde_json::{json, Value};
const PATH: &str = "/v1/internal/gateway/console/luma-agents-files";
fn input(fixture: &Fixture, operation: Value) -> Value {
    json!({"providerId":"luma-pipeline","credentialId":"exact", "expectedRevision":fixture.state.route_config.snapshot().revision().id(),"operation":operation})
}

#[tokio::test]
async fn files_management_upload_list_refresh_delete_and_missing() {
    let server = Server::start(vec![
        Reply::json(
            201,
            json!({"id":ID,"state":"pending","upload":null,"file":metadata("pending")}),
        ),
        Reply::json(
            200,
            json!({"data":[metadata("pending")],"has_more":false,"next_cursor":null}),
        ),
        Reply::json(200, metadata("ready")),
        Reply::json(204, Value::Null),
        Reply::json(404, json!({"secret":"hidden"})),
    ])
    .await;
    within(&server.origin, Box::pin(async {
        let fixture = Box::pin(Fixture::new(&server,5)).await;
        let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
        for operation in [json!({"action":"upload","filename":"参考.png","purpose":"reference","data":"AQID"}),
            json!({"action":"list"}),json!({"action":"get","id":ID}),json!({"action":"delete","id":ID})] {
            let response = Box::pin(send_console_json(&fixture.state, Method::POST, PATH, input(&fixture,operation.clone()),Some(&grant),None)).await;
            assert_eq!(response.status(),StatusCode::OK);
            assert_eq!(response.headers()["cache-control"],"no-store");
            let body = parse_json(response).await;
            assert_eq!(body["credentialId"],"exact"); assert_eq!(body["providerId"],"luma-pipeline");
            assert_eq!(body["result"]["action"],operation["action"]);
            if operation["action"] == "get" { assert_eq!(body["result"]["file"]["state"],"ready"); }
        }
        let response = Box::pin(send_console_json(&fixture.state, Method::POST, PATH,input(&fixture,json!({"action":"get","id":ID})),Some(&grant),None)).await;
        assert_eq!(response.status(),StatusCode::NOT_FOUND); assert!(!drain(response.into_body()).await.contains("hidden"));
        let calls = server.calls(); assert_eq!(calls.len(),5);
        assert_eq!(calls[3].method,"DELETE");
        assert!(calls.iter().all(|c| c.headers["authorization"] == format!("Bearer {KEY}")));
        assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await;
    })).await;
    server.stop().await;
}

#[tokio::test]
async fn files_management_denies_ungranted_foreign_disabled_stale_or_wrong_account() {
    let server = Server::start(vec![]).await;
    within(
        &server.origin,
        Box::pin(async {
            let fixture = Box::pin(Fixture::new(&server, 5)).await;
            let grant = Box::pin(grant_console_secret_access(&fixture.state)).await;
            let valid = input(&fixture, json!({"action":"list"}));
            for (field, value) in [
                ("credentialId", "disabled"),
                ("credentialId", "backup-exact"),
                ("credentialId", "exact "),
                ("providerId", "other"),
                ("expectedRevision", "stale"),
            ] {
                let mut body = valid.clone();
                body[field] = json!(value);
                let response = Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    PATH,
                    body,
                    Some(&grant),
                    None,
                ))
                .await;
                assert!(response.status().is_client_error(), "{field} {value}");
            }
            for (access, origin) in [
                (None, None),
                (Some(grant.as_str()), Some("https://foreign.test")),
            ] {
                let response = Box::pin(send_console_json(
                    &fixture.state,
                    Method::POST,
                    PATH,
                    valid.clone(),
                    access,
                    origin,
                ))
                .await;
                assert!(response.status().is_client_error());
                assert_eq!(response.headers()["cache-control"], "no-store");
            }
            assert!(server.calls().is_empty());
            fixture.finish().await;
        }),
    )
    .await;
    server.stop().await;
}
