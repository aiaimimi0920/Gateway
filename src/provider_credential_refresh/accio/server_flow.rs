//! Normal management writes create the DB source; normal public requests consume it and finalize audit.
use super::{server_fixture::Fixture, server_peer::Peer, server_runtime::Server};
use serde_json::{json, Value};
pub(super) async fn create(server: &Server, expiry: &str) -> (String, String) {
    let account = server.manage("POST", "/v1/internal/gateway/provider-accounts", Some(json!({
        "label":"Accio DB server","adapter":"accio_compatible","protocolFamily":"openai",
        "protocolProfile":"accio","payload":{"adapter":"accio_compatible","baseUrl":"https://phoenix-gw.alibaba.com",
        "supportedModels":["claude-sonnet-4-6"],"callFlow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"accio"}}
    }))).await;
    let account = account["providerAccount"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let credential = server.manage("POST", &format!("/v1/internal/gateway/provider-accounts/{account}/credentials"), Some(json!({
        "label":"Retained Accio credential","sourceKind":"manual","syncMode":"manual","payload":{
        "apiKey":"server-original-access","refreshToken":"server-original-refresh","refreshClientId":"explicit-client",
        "refreshEndpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token","accioSgcookie":"server-secret-sg",
        "expiresAt":expiry,"headers":{"utdid":"server-device"},"supportedModels":["claude-sonnet-4-6"]}
    }))).await;
    (
        account,
        credential["providerCredential"]["id"]
            .as_str()
            .unwrap()
            .to_owned(),
    )
}
pub(super) async fn public(server: &Server, kind: &str, stream: bool, prompt: &str) {
    let path = match kind {
        "chat" => "/v1/chat/completions",
        "responses" => "/v1/responses",
        _ => "/v1/messages",
    };
    let mut body = json!({"model":"claude-sonnet-4-6","stream":stream,"token":"caller-forbidden","utdid":"caller-device"});
    if kind == "responses" {
        body["input"] = json!(prompt);
    } else {
        body["messages"] = json!([{"role":"user","content":prompt}]);
        body["max_tokens"] = json!(64);
    }
    let (status, text) = server
        .request("POST", path, Some("server-public-synthetic"), Some(body))
        .await;
    assert_eq!(status, 200, "{path}: {text}");
    assert!(text.contains("OK 你好"), "{path}: {text}");
    if stream {
        assert!(
            text.contains(match kind {
                "responses" => "response.completed",
                "chat" => "[DONE]",
                _ => "message_stop",
            }),
            "{text}"
        );
    } else {
        let _: Value = serde_json::from_str(&text).unwrap();
    }
    assert!(!text.contains("server-secret-sg") && !text.contains("server-original-refresh"));
}
pub(super) fn wire(peer: &Peer, tokens: &[&str]) {
    let calls = peer.inference.lock().unwrap();
    assert_eq!(calls.len(), tokens.len());
    for ((headers, body), token) in calls.iter().zip(tokens) {
        assert_eq!(body["token"], *token);
        assert_eq!(body["utdid"], "server-device");
        assert_eq!(headers["utdid"], "server-device");
        assert!(
            !body.to_string().contains("caller-forbidden")
                && !body.to_string().contains("server-secret-sg")
                && !body.to_string().contains("server-original-refresh")
        );
    }
}
pub(super) async fn audit(f: &Fixture, account: &str, count: i64) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let completed: i64 = sqlx::query_scalar("select count(*) from gateway_request_audits where provider_account_id=$1 and status='completed' and total_tokens=9 and route_attempt_count=1 and completed_at is not null")
                .bind(account).fetch_one(&f.pg).await.unwrap();
            if completed == count { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.unwrap();
}
