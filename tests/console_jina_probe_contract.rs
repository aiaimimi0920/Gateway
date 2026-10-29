//! One Jina credential must prove search and reader independently, with no content/secret echo.
use axum::{
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Json, Router,
};
use neuro_gateway::{db, http::router::build_router, local_runtime::LocalRuntime};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
#[path = "console_contract_support/mod.rs"]
mod support;
use support::*;

type Calls = Arc<Mutex<Vec<(String, String)>>>;
async fn upstream(
    State(calls): State<Calls>,
    OriginalUri(uri): OriginalUri,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, String) {
    let reader = uri.path() == "/";
    let expected = if reader {
        json!({"url":"https://example.com/", "retainImages":"none", "withGeneratedAlt":false,
            "timeout":30, "tokenBudget":10000})
    } else {
        assert!(matches!(
            body["q"].as_str(),
            Some("https://example.com/" | "Rust programming language official documentation")
        ));
        json!({"q":body["q"], "num":3, "fallback":false,
            "retainImages":"none", "withGeneratedAlt":false})
    };
    assert_eq!(body, expected);
    assert_eq!(headers["accept"], "application/json");
    assert!(headers.get("x-api-key").is_none());
    let key = headers["authorization"]
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap()
        .to_string();
    calls.lock().unwrap().push((key.clone(), uri.path().into()));
    let page = json!({"title":"Example good-secret", "url":"https://example.com/", "content":"Never echo good-secret"});
    let data = if reader { page } else { json!([page]) };
    match key.as_str() {
        "good-secret" => (
            StatusCode::OK,
            json!({"code":200,"status":20000,"data":data}).to_string(),
        ),
        "bad-secret" => (StatusCode::UNAUTHORIZED, "bad-secret".into()),
        "payment-secret" => (StatusCode::PAYMENT_REQUIRED, "payment-secret".into()),
        "forbidden-secret" => (StatusCode::FORBIDDEN, "forbidden-secret".into()),
        "limited-secret" => (StatusCode::TOO_MANY_REQUESTS, "limited-secret".into()),
        "server-secret" => (StatusCode::INTERNAL_SERVER_ERROR, "server-secret".into()),
        "empty-secret" => (
            StatusCode::OK,
            json!({"code":200,"status":20000,"data":null}).to_string(),
        ),
        "invalid-secret" => (StatusCode::OK, "not JSON invalid-secret".into()),
        "huge-secret" => (StatusCode::OK, "x".repeat(1_048_577)),
        "error-secret" => (
            StatusCode::OK,
            json!({"code":200,"status":20000,"error":"error-secret","data":data}).to_string(),
        ),
        "code-secret" => (
            StatusCode::OK,
            json!({"code":401,"status":40100,"data":data}).to_string(),
        ),
        "malformed-secret" => {
            let page = json!({"title":"Invalid", "url":"javascript:alert(1)", "content":"malformed-secret"});
            (
                StatusCode::OK,
                json!({"code":200,"status":20000,"data":if reader {page} else {json!([page])}})
                    .to_string(),
            )
        }
        "shape-secret" => (
            StatusCode::OK,
            json!({"code":200,"status":20000,"results":data}).to_string(),
        ),
        "content-secret" => (
            StatusCode::OK,
            json!({"code":200,"status":20000,
            "data":{"title":"Example", "url":"https://example.com/", "content":" "}})
            .to_string(),
        ),
        _ => panic!("unexpected synthetic credential"),
    }
}

async fn send(state: &Arc<neuro_gateway::state::AppState>, grant: &str, body: Value) -> Value {
    let path = if body.is_null() {
        "/v1/internal/gateway/console/credentials/good/probe"
    } else {
        "/v1/internal/gateway/console/providers/jina-custom-id/probe"
    };
    let response = build_router(state.clone())
        .oneshot(
            Request::post(path)
                .header("x-management-token", MANAGEMENT_TOKEN)
                .header("x-secret-grant", grant)
                .header("content-type", "application/json")
                .body(Body::from(if body.is_null() {
                    String::new()
                } else {
                    body.to_string()
                }))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    parse_json(response).await
}

#[tokio::test]
async fn unified_jina_probes_share_one_credential_but_separate_operations_and_audits() {
    assert!(
        neuro_gateway::preset::builtin_presets().contains_key("jina"),
        "Unified Jina preset is missing"
    );
    let calls = Calls::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = Router::new()
        .route("/search", post(upstream))
        .route("/", post(upstream))
        .with_state(calls.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let ids = [
        "good",
        "bad",
        "payment",
        "forbidden",
        "limited",
        "server",
        "empty",
        "invalid",
        "huge",
        "error",
        "code",
        "malformed",
        "shape",
        "content",
        "disabled",
    ];
    let credentials = ids.map(|id| {
        json!({"id":id,"api_key":format!("{id}-secret"),"enabled":id!="disabled",
        "supported_models":["jina-search","jina-fetch"]})
    });
    let mut fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"jina-custom-id","preset":"jina","base_url":format!("http://{address}"),
        "call_flow":{"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"},
        "supported_models":["jina-search","jina-fetch"],"model_map":{"wrong-alias":"jina-research"},"credentials":credentials
    }],"model_routes":[],"aliases":{}})).unwrap(), false);
    let directory = std::env::temp_dir().join(format!("jina-contract-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    let grant = grant_console_secret_access(&fixture.state).await;
    for model in ["jina-search", "jina-fetch"] {
        for (id, status, message) in [
            (
                "good",
                "passed",
                if model == "jina-fetch" {
                    "Reader returned a web page"
                } else {
                    "Search returned 1 web result"
                },
            ),
            ("bad", "failed", "HTTP 401"),
            ("payment", "failed", "HTTP 402"),
            ("forbidden", "failed", "HTTP 403"),
            ("limited", "failed", "HTTP 429"),
            ("server", "failed", "HTTP 500"),
            ("empty", "failed", "no usable web"),
            ("invalid", "failed", "invalid JSON"),
            ("huge", "failed", "exceeded 1 MiB"),
            ("error", "failed", "no usable web"),
            ("code", "failed", "no usable web"),
            ("malformed", "failed", "no usable web"),
            ("shape", "failed", "no usable web"),
            ("disabled", "unsupported", "disabled"),
        ] {
            let body = send(
                &fixture.state,
                &grant,
                json!({"prompt":"https://example.com/","model":model,"credentialIds":[id]}),
            )
            .await;
            let result = &body["result"]["results"][0];
            assert_eq!(result["status"], status, "{body}");
            assert!(
                result["message"].as_str().unwrap().contains(message),
                "{body}"
            );
            assert!(!body.to_string().contains("-secret"));
        }
    }
    for model in ["other", "jina-research", "wrong-alias"] {
        let body = send(
            &fixture.state,
            &grant,
            json!({"prompt":"https://example.com/","model":model,"credentialIds":["good"]}),
        )
        .await;
        assert_eq!(body["result"]["unsupportedCount"], 1);
    }
    let empty_content = send(
        &fixture.state,
        &grant,
        json!({"prompt":"https://example.com/","model":"jina-fetch","credentialIds":["content"]}),
    )
    .await;
    assert_eq!(empty_content["result"]["results"][0]["status"], "failed");
    assert!(!empty_content.to_string().contains("-secret"));
    let plan = json!({"automaticEnabled":false,"intervalMinutes":60,"modelSelection":"selected","models":["jina-search","jina-fetch"],"cases":[{
        "id":"web","name":"Web","prompt":"https://example.com/","expectedAnswer":null,"difficulty":1,"enabled":true
    }]});
    let body = send(
        &fixture.state,
        &grant,
        json!({"testPlan":plan,"credentialIds":["good"]}),
    )
    .await;
    let assessment = &body["result"]["results"][0]["assessment"];
    assert_eq!(assessment["models"].as_array().unwrap().len(), 2, "{body}");
    assert_eq!(assessment["cases"].as_array().unwrap().len(), 2, "{body}");
    for model in assessment["models"].as_array().unwrap() {
        assert_eq!(model["callable"], true, "{body}");
        assert_eq!(model["gradedCount"], 0);
        assert_eq!(model["score"], Value::Null);
    }
    for case in assessment["cases"].as_array().unwrap() {
        assert_eq!(case["expectedAnswer"], Value::Null);
        assert_eq!(case["correct"], Value::Null);
    }
    let default_probe = send(&fixture.state, &grant, Value::Null).await;
    assert_eq!(
        default_probe["result"]["status"], "passed",
        "{default_probe}"
    );
    let mut reader_fixture = ConsoleStateFixture::new(serde_json::from_value(json!({"providers":[{
        "id":"jina-custom-id","preset":"jina","base_url":format!("http://{address}"),
        "default_model":"jina-fetch","supported_models":["jina-search","jina-fetch"],
        "credentials":[{"id":"good","api_key":"good-secret","supported_models":["jina-search","jina-fetch"]}]
    }],"model_routes":[],"aliases":{}})).unwrap(), false);
    Arc::get_mut(&mut reader_fixture.state)
        .unwrap()
        .local_runtime = Some(local.clone());
    let reader_grant = grant_console_secret_access(&reader_fixture.state).await;
    let default_reader = send(&reader_fixture.state, &reader_grant, Value::Null).await;
    assert_eq!(
        default_reader["result"]["status"], "passed",
        "{default_reader}"
    );
    assert_eq!(calls.lock().unwrap().len(), 31);
    let audits = local
        .list_audits(&db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(audits.len(), 31);
    assert_eq!(
        audits.iter().filter(|a| a.endpoint_kind == "fetch").count(),
        16
    );
    assert!(audits.iter().all(|a| a.protocol_family == "search"
        && a.route_trace
            .as_ref()
            .is_some_and(|t| t["realCredentialRef"]
                .as_str()
                .is_some_and(|id| ids.contains(&id)))));
    let summary = db::request_audits::summarize_request_audit_rows(&audits);
    assert_eq!(summary.completed_count, 6);
    let states = local
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap();
    for model in ["jina-search", "jina-fetch"] {
        assert!(states
            .iter()
            .any(|s| s.provider_credential_ref.as_deref() == Some("good")
                && s.model == model
                && s.status == "active"));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    drop(fixture);
    drop(reader_fixture);
    local.close().await;
    drop(local);
    assert!(directory.starts_with(std::env::temp_dir()));
    for attempt in 0..40 {
        match std::fs::remove_dir_all(&directory) {
            Ok(()) => break,
            Err(error)
                if cfg!(windows)
                    && matches!(error.raw_os_error(), Some(32 | 33))
                    && attempt < 39 =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(error) => panic!("cannot clean isolated Jina fixture: {error}"),
        }
    }
}
