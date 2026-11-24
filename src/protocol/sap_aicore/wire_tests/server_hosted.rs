//! Opt-in Hosted proof: separate app states, persisted identities and loopback SAP peers.
use super::{super::test_support, mtls_tests::OAuthPeer, *};

const PUBLIC: &str = "sap-server-public-synthetic";
const PROJECT: &str = "platform-default-project";
const ACCOUNTS: &str = "/v1/internal/gateway/provider-accounts";

mod bedrock_images;
mod orchestration;
mod pinned_route;
mod probe;
mod reasoning;
mod responses;
mod stored_health;
mod stored_stream;

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_x509_public_json_and_sse() {
    run_hosted(hosted);
}

fn run_hosted<F, Fut>(run: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()>,
{
    // Keep Windows Server bootstrap on the same bounded stack as the other Hosted fixtures.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(Box::pin(run()))
        })
        .unwrap()
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

async fn hosted() {
    let scope = format!("sap-hosted-{}", uuid::Uuid::new_v4().simple());
    let inference = peer::Peer::chat_with_unique_ids(scope.clone()).await;
    let writer = build(&scope, "writer", &inference).await;
    let account = manage(&writer, ACCOUNTS, json!({
        "label":scope,"adapter":"openai_compatible","protocolFamily":"openai",
        "protocolProfile":super::super::PROFILE,"status":"active","executionMode":"direct_http",
        "payload":{"adapter":"openai_compatible","auth_mode":super::super::AUTH_MODE,
            "base_url":test_support::BASE,"supported_models":[UPSTREAM_MODEL],
            "headers":{"AI-Resource-Group":"group-a"},"extra_body":{"sap_api_version":"2024-10-21"}}
    })).await;
    let account = account["providerAccount"]["id"].as_str().unwrap();
    let credential = manage(&writer, &format!("{ACCOUNTS}/{account}/credentials"), json!({
        "label":"Isolated SAP certificate","status":"active","sourceKind":"manual","syncMode":"manual",
        "payload":{"api_key":mtls_fixtures::service_key(false).to_string(),"supported_models":[UPSTREAM_MODEL]}
    })).await;
    let credential = credential["providerCredential"]["id"].as_str().unwrap();
    let alias = manage(
        &writer,
        "/v1/internal/gateway/model-aliases",
        json!({
            "alias":scope,"projectId":PROJECT,"scopeType":"project",
            "providerAccountId":account,"upstreamModel":UPSTREAM_MODEL
        }),
    )
    .await;
    assert_eq!(alias["modelAlias"]["alias"], scope);

    let reader = build(&scope, "reader", &inference).await;
    let oauth = OAuthPeer::start().await;
    let chat = "chat/completions";
    test_support::origin(inference.origin.clone(), async {
        test_support::mtls(oauth.origin.clone(), mtls_tests::root_pem(), async {
            let (status, _) = infer(&reader, &scope, chat, false, "invalid").await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
        })
        .await;
        test_support::mtls(
            oauth.origin.replace("localhost", "127.0.0.1"),
            mtls_tests::root_pem(),
            async {
                let (status, text) = infer(&reader, &scope, chat, false, PUBLIC).await;
                assert_eq!(status, StatusCode::BAD_GATEWAY, "{text}");
                assert!(text.contains("sap_aicore_token_failed"));
            },
        )
        .await;
    })
    .await;
    oauth.assert_calls(&[]);

    assert!(inference.calls.lock().unwrap().is_empty());
    let completed = test_support::origin(
        inference.origin.clone(),
        test_support::mtls(oauth.origin.clone(), mtls_tests::root_pem(), async {
            let mut completed = 0;
            for endpoint in ENDPOINTS {
                for stream in [false, true] {
                    let (status, text) = infer(&reader, &scope, endpoint, stream, PUBLIC).await;
                    assert_eq!(status, StatusCode::OK, "{endpoint}/{stream}: {text}");
                    assert!(text.contains("合成答案"), "{text}");
                    if stream {
                        let terminal = match endpoint {
                            "messages" => "message_stop",
                            "responses" => "response.completed",
                            _ => "[DONE]",
                        };
                        assert!(text.contains(terminal), "{text}");
                    }
                    completed += 1;
                    assert_inference_wire(&inference, completed);
                }
            }
            completed
        }),
    )
    .await;
    assert!(inference.oauth.lock().unwrap().is_empty());
    oauth.assert_calls(&["synthetic-sap-client"]);
    assert_audits(
        &writer,
        account,
        credential,
        &scope,
        completed.try_into().unwrap(),
    )
    .await;
    finish(writer, reader, inference).await;
}

async fn finish(writer: Arc<AppState>, reader: Arc<AppState>, inference: peer::Peer) {
    for state in [reader, writer] {
        state.shutdown.request("sap-hosted-fixture");
        state.pg_pool.as_ref().unwrap().close().await;
        state
            .upstream_client
            .freebuff
            .shutdown(Duration::from_secs(1))
            .await
            .unwrap();
    }
    inference.stop().await;
}

async fn build(scope: &str, instance: &str, proxy: &peer::Peer) -> Arc<AppState> {
    build_with_key_secret(scope, instance, proxy, None).await
}

async fn build_with_key_secret(
    scope: &str,
    instance: &str,
    proxy: &peer::Peer,
    secret: Option<&str>,
) -> Arc<AppState> {
    // The caller may keep the isolated routes with other test evidence; no fixed path is required.
    let root = std::env::var_os("SAP_AICORE_TEST_EVIDENCE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    assert!(root.is_absolute() && root.is_dir());
    let root = root.join(scope).join(instance);
    std::fs::create_dir_all(&root).unwrap();
    let mut config = support::test_config();
    config.storage_mode = crate::config::GatewayStorageMode::Server;
    let db_url = std::env::var("SAP_AICORE_TEST_DATABASE_URL").unwrap();
    let database = url::Url::parse(&db_url).expect("invalid fixture PostgreSQL URL");
    assert_eq!(database.host_str(), Some("127.0.0.1"));
    assert_eq!(database.path(), "/gateway_fixture");
    assert!(database.port().is_some_and(|port| port != 5432));
    config.database_url = Some(db_url);
    config.redis_url = std::env::var("SAP_AICORE_TEST_REDIS_URL").unwrap();
    let redis = url::Url::parse(&config.redis_url).expect("invalid fixture Redis URL");
    assert_eq!(redis.host_str(), Some("127.0.0.1"));
    assert!(redis.port().is_some_and(|port| port != 6379));
    config.gateway_api_key = Some(PUBLIC.into());
    config.gateway_api_key_secret = secret.map(str::to_owned);
    config.default_project_id = PROJECT.into();
    config.upstream_timeout_secs = 5;
    config.console.routes_file = root.join("routes.yaml");
    config.console.state_dir = root.join("state");
    config.console.redis_namespace = scope.into();
    std::fs::write(
        &config.console.routes_file,
        "providers: []\nmodel_routes: []\n",
    )
    .unwrap();
    let mut state = Box::pin(crate::runtime::build_app_state(config))
        .await
        .unwrap();
    assert!(
        state.local_runtime.is_none() && state.pg_pool.is_some() && !state.redis_pool.is_closed()
    );
    Arc::get_mut(&mut state).unwrap().upstream_client.http = proxy.client();
    state
}

async fn provision(
    writer: &Arc<AppState>,
    alias: &str,
    settings: Value,
    mut payload: Value,
) -> (String, String) {
    let account = manage(writer, ACCOUNTS, json!({
        "label":alias,"adapter":"openai_compatible","protocolFamily":"openai",
        "protocolProfile":crate::protocol::sap_aicore::PROFILE,"status":"active","executionMode":"direct_http",
        "payload":{"adapter":"openai_compatible","auth_mode":crate::protocol::sap_aicore::AUTH_MODE,
            "base_url":test_support::BASE,"supported_models":[UPSTREAM_MODEL],
            "headers":{"AI-Resource-Group":"group-a"},
            "extra_body":settings}
    })).await;
    let account = account["providerAccount"]["id"].as_str().unwrap();
    payload["supported_models"] = json!([UPSTREAM_MODEL]);
    let credential = manage(writer, &format!("{ACCOUNTS}/{account}/credentials"), json!({
        "label":"Synthetic SAP Service Key","status":"active","sourceKind":"manual","syncMode":"manual",
        "payload":payload
    })).await;
    let credential = credential["providerCredential"]["id"].as_str().unwrap();
    let result = manage(
        writer,
        "/v1/internal/gateway/model-aliases",
        json!({
            "alias":alias,"projectId":PROJECT,"scopeType":"project",
            "providerAccountId":account,"upstreamModel":UPSTREAM_MODEL
        }),
    )
    .await;
    assert_eq!(result["modelAlias"]["alias"], alias);
    (account.into(), credential.into())
}

async fn infer(
    state: &Arc<AppState>,
    alias: &str,
    endpoint: &str,
    stream: bool,
    key: &str,
) -> (StatusCode, String) {
    let mut payload = body(endpoint, stream);
    payload["model"] = json!(alias);
    call(state, &format!("/v1/{endpoint}"), key, payload).await
}

async fn manage(state: &Arc<AppState>, path: &str, payload: Value) -> Value {
    let (status, text) = call(state, path, support::MANAGEMENT_TOKEN, payload).await;
    assert_eq!(status, StatusCode::OK, "{path}: {text}");
    serde_json::from_str(&text).unwrap()
}

async fn call(
    state: &Arc<AppState>,
    path: &str,
    token: &str,
    payload: Value,
) -> (StatusCode, String) {
    let mut request = Request::post(path)
        .header("authorization", format!("Bearer {token}"))
        .header("cookie", "foreign=caller-secret")
        .header("content-type", "application/json");
    if token == support::MANAGEMENT_TOKEN {
        request = request.header("x-management-token", token);
    }
    let request = request.body(Body::from(payload.to_string())).unwrap();
    let response = tokio::time::timeout(
        Duration::from_secs(20),
        Box::pin(build_router(state.clone()).oneshot(request)),
    )
    .await
    .unwrap()
    .unwrap();
    let status = response.status();
    let text = drain(response.into_body()).await;
    assert_redacted(&text);
    (status, text)
}

fn assert_redacted(text: &str) {
    for secret in [
        "PRIVATE KEY",
        "BEGIN CERTIFICATE",
        "synthetic-sap-client",
        "synthetic-sap-secret",
        "synthetic-sap-access",
        PUBLIC,
        "caller-secret",
    ] {
        assert!(
            !text.contains(secret),
            "fixture material appeared in output"
        );
    }
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
            let (completed, exact): (i64, i64) = sqlx::query_as(
                "SELECT count(*), count(*) FILTER (WHERE prompt_tokens = 2 AND completion_tokens = 3 AND
                 total_tokens = 5 AND route_attempt_count = 1 AND requested_model = $2 AND resolved_model = $3 AND
                 route_trace->>'realCredentialRef' = $4 AND api_key_id = $5 AND project_id = $6)
                 FROM gateway_request_audits WHERE provider_account_id = $1 AND status = 'completed'",
            ).bind(account).bind(alias).bind(UPSTREAM_MODEL).bind(credential)
                .bind(crate::auth::session::SHARED_SECRET_API_KEY_ID).bind(PROJECT).fetch_one(pg).await.unwrap();
            assert!(completed <= expected, "unexpected duplicate completed audit");
            if completed == expected {
                assert_eq!(exact, expected, "incorrect caller, model, credential, attempt or usage");
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("Hosted audit did not finalize");
    let traces: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT route_trace::text FROM gateway_request_audits WHERE provider_account_id = $1",
    )
    .bind(account)
    .fetch_all(pg)
    .await
    .unwrap();
    for trace in traces.iter().flatten() {
        assert_redacted(trace);
    }
}
