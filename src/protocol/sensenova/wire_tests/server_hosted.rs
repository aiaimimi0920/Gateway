//! Server public requests use persisted Provider accounts, credentials and model aliases, with no route-file fallback.
use super::{
    server_fixture::{build, close, run, PUBLIC},
    *,
};

#[test]
#[ignore = "Requires isolated PostgreSQL 001–007 / Redis and retains uniquely scoped fixture data"]
fn sensenova_server_hosted_account_native_public_json_and_sse() {
    run(hosted);
}

async fn hosted() {
    let scope = format!("sensenova-hosted-{}", uuid::Uuid::new_v4().simple());
    let first_peer = peer::Peer::new("chat", false, false, 200).await;
    let blank = json!({"providers":[],"model_routes":[]});
    let first = build(&scope, "configure", &first_peer, blank.clone()).await;
    let account = call(
        &first,
        "/v1/internal/gateway/provider-accounts",
        support::MANAGEMENT_TOKEN,
        json!({
            "label":scope,"adapter":"openai_compatible","protocolFamily":"openai",
            "protocolProfile":super::super::PROFILE,"status":"active","executionMode":"direct_http",
            "payload":{"adapter":"openai_compatible","auth_mode":super::super::AUTH_MODE,
                "base_url":super::super::BASE,"supported_models":[UPSTREAM_MODEL]}
        }),
    )
    .await;
    let account = account["providerAccount"]["id"].as_str().unwrap();
    let credential = call(&first, &format!("/v1/internal/gateway/provider-accounts/{account}/credentials"), support::MANAGEMENT_TOKEN, json!({
        "label":"Isolated SenseNova account","status":"active","sourceKind":"manual","syncMode":"manual",
        "payload":{"api_key":KEY,"supported_models":[UPSTREAM_MODEL]}
    })).await;
    let credential = credential["providerCredential"]["id"].as_str().unwrap();
    let alias = call(
        &first,
        "/v1/internal/gateway/model-aliases",
        support::MANAGEMENT_TOKEN,
        json!({
            "alias":scope,"projectId":first.config.default_project_id,"scopeType":"project",
            "providerAccountId":account,"upstreamModel":UPSTREAM_MODEL
        }),
    )
    .await;
    assert_eq!(alias["modelAlias"]["alias"], scope);
    assert!(first_peer.calls.lock().unwrap().is_empty());

    let mut sent = 0;
    for endpoint in ENDPOINTS {
        for stream in [false, true] {
            let family = family(endpoint);
            let fixture_id = match family {
                "messages" => "native-msg",
                "responses" => "native-resp",
                _ => "native-chat",
            };
            let raw = String::from_utf8(native::reply(family, false, stream, true))
                .unwrap()
                .replace(fixture_id, &format!("{fixture_id}-{scope}-{sent}"));
            // Use separate app states to prove the account can be read from PostgreSQL by another instance.
            let peer = peer::Peer::raw(raw.into_bytes()).await;
            let app = build(&scope, &format!("send-{sent}"), &peer, blank.clone()).await;
            let mut request = body(endpoint, stream);
            request["model"] = json!(scope);
            let response = super::super::test_support::origin(peer.origin.clone(), async {
                open(&app, &format!("/v1/{endpoint}"), PUBLIC, request).await
            })
            .await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
            assert!(text.contains("合成答案"), "{text}");
            if stream {
                assert!(
                    text.contains(match family {
                        "messages" => "message_stop",
                        "responses" => "response.completed",
                        _ => "[DONE]",
                    }),
                    "{text}"
                );
            }
            assert_wire(&peer, 1, endpoint);
            sent += 1;
            assert_audits(&first, account, credential, &scope, sent).await;
            close(app).await;
            peer.stop().await;
        }
    }
    close(first).await;
    first_peer.stop().await;
}

async fn open(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    body: Value,
) -> axum::response::Response {
    let request = Request::post(path)
        .header("authorization", format!("Bearer {token}"))
        .header(
            "x-management-token",
            if token == PUBLIC {
                "not-the-caller"
            } else {
                token
            },
        )
        .header("cookie", "foreign=caller-secret")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    Box::pin(build_router(state.clone()).oneshot(request))
        .await
        .unwrap()
}

async fn call(state: &Arc<AppState>, path: &str, token: &str, body: Value) -> Value {
    let response = open(state, path, token, body).await;
    let status = response.status();
    let text = drain(response.into_body()).await;
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    assert!(!text.contains(KEY));
    serde_json::from_str(&text).unwrap()
}

async fn assert_audits(
    observer: &AppState,
    account: &str,
    credential: &str,
    alias: &str,
    expected: i64,
) {
    let pg = observer.pg_pool.as_ref().unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let (all, completed): (i64, i64) = sqlx::query_as(
                "SELECT count(*), count(*) FILTER (WHERE status = 'completed' AND
                prompt_tokens = 2 AND completion_tokens = 3 AND total_tokens = 5 AND
                route_attempt_count = 1 AND requested_model = $2 AND resolved_model = $3 AND
                route_trace->>'realCredentialRef' = $4 AND
                (api_key_id IS NOT NULL OR user_credential_id IS NOT NULL OR access_key_id IS NOT NULL))
                FROM gateway_request_audits WHERE provider_account_id = $1",
            ).bind(account).bind(alias).bind(UPSTREAM_MODEL).bind(credential).fetch_one(pg).await.unwrap();
            assert!(all <= expected, "unexpected duplicate public audit");
            if (all, completed) == (expected, expected) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("public audit did not finalize with exact model, caller, credential and usage");
    let internal: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM gateway_internal_model_probe_audits WHERE provider_id = $1",
    )
    .bind(account)
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(
        internal, 0,
        "public calls must not be labeled internal probes"
    );
}
