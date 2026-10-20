//! Independent site telemetry uses bounded public recipes, never inference or account secrets.
use axum::{
    extract::{Request, State},
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use neuro_gateway::site_health::{
    allows, discovery::discover, HealthMode, SiteHealthConfig, SiteSnapshot, UnknownPolicy,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Fixture {
    requests: Mutex<Vec<(String, String, Value)>>,
    embed_only: bool,
}
struct Server {
    url: String,
    fixture: Arc<Fixture>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(embed_only: bool) -> Self {
        let fixture = Arc::new(Fixture {
            embed_only,
            ..Default::default()
        });
        let app = Router::new()
            .fallback(any(respond))
            .with_state(fixture.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { url, fixture, task }
    }
}
async fn respond(State(f): State<Arc<Fixture>>, req: Request) -> Response {
    assert!(req.headers().get("authorization").is_none());
    assert!(req.headers().get("cookie").is_none());
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let uri = req.uri().to_string();
    let bytes = axum::body::to_bytes(req.into_body(), 65536).await.unwrap();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    f.requests.lock().unwrap().push((method, uri, body));
    if f.embed_only && !path.starts_with("/api/model-status/") {
        return axum::http::StatusCode::NOT_FOUND.into_response();
    }
    Json(match path.as_str() {
        "/api/perf-metrics/summary" => json!({"success":true,"data":{"models":[{"model_name":"a","success_rate":80},{"model_name":"b","success_rate":100}]}}),
        "/api/enhancements/model-status/embed/status/all" => json!({"success":true,"data":[{"model_name":"a","group_name":"paid","success_rate":99,"total_requests":10}]}),
        "/api/channel/health/models" => json!({"success":true,"data":[{"model_name":"a","test_count":10,"success_count":8},
            {"model_name":"a","test_count":10,"success_count":7,"client_error_count":2,"moderation_count":1}]}),
        "/api/model-status/embed/config/selected" => json!({"success":true,"time_window":"6h","data":["a","a"]}),
        "/api/model-status/embed/status/batch" => json!({"success":true,"data":[{"model_name":"a","success_rate":96},{"model_name":"not-selected","success_rate":100}]}),
        "/api/model_health/slot_status" => json!({"success":true,"data":{"enabled":true,"group":"paid","models":[{"model_name":"a","success_rate":98,"total_requests":10}]}}),
        _ => return axum::http::StatusCode::NOT_FOUND.into_response(),
    }).into_response()
}
fn private_config() -> SiteHealthConfig {
    SiteHealthConfig {
        allow_private_network: true,
        monitor_groups: vec!["paid".into()],
        ..Default::default()
    }
}

#[tokio::test]
async fn automatic_detection_tries_all_known_recipes_without_account_calls() {
    let server = Server::start(false).await;
    let report = discover("site", &format!("{}/v1", server.url), &private_config())
        .await
        .unwrap();
    assert_eq!(report.probes.len(), 5);
    assert!(
        report.probes.iter().all(|p| p.status == "supported"),
        "{:#?}",
        report.probes
    );
    assert_eq!(
        report.selected_template.as_deref(),
        Some("new-api-perf-summary")
    );
    let channel = report
        .probes
        .iter()
        .find(|p| p.template == "channel-health")
        .unwrap();
    assert_eq!(channel.model_count, 1);
    assert!(report.probes.iter().all(|p| p.models.is_empty()));
    let requests = server.fixture.requests.lock().unwrap();
    assert_eq!(requests.len(), 6);
    let batch = requests
        .iter()
        .find(|(_, p, _)| p.starts_with("/api/model-status/embed/status/batch"))
        .unwrap();
    assert_eq!(batch.0, "POST");
    assert_eq!(batch.2, json!(["a"]));
    assert!(batch.1.contains("window=6h"));
    assert!(!requests.iter().any(|(_, p, _)| p.contains("completions")
        || p.contains("responses")
        || p.contains("models/test")));
}

#[tokio::test]
async fn selected_models_bound_embed_evidence_and_missing_groups_are_not_guessed() {
    let server = Server::start(true).await;
    let config = SiteHealthConfig {
        allow_private_network: true,
        ..Default::default()
    };
    let report = discover("site", &server.url, &config).await.unwrap();
    assert_eq!(
        report.selected_template.as_deref(),
        Some("model-status-embed")
    );
    assert_eq!(report.models.len(), 1);
    assert_eq!(report.models["a"], Some(96.0));
    assert_eq!(
        report
            .probes
            .iter()
            .find(|p| p.template == "model-health-slot")
            .unwrap()
            .status,
        "parameters_required"
    );
    assert!(!server
        .fixture
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(_, p, _)| p.contains("slot_status")));
}

#[tokio::test]
async fn loopback_is_rejected_without_explicit_private_network_permission() {
    let server = Server::start(false).await;
    assert!(discover("site", &server.url, &SiteHealthConfig::default())
        .await
        .is_err());
    assert!(server.fixture.requests.lock().unwrap().is_empty());
}

#[test]
fn advisory_restriction_unknown_ttl_binding_and_model_mapping_are_separate() {
    let mut config = SiteHealthConfig::default();
    let mut report = SiteSnapshot {
        provider_id: "site".into(),
        checked_at: 1000,
        binding: "binding".into(),
        probes: vec![],
        selected_template: None,
        models: [("upstream".into(), Some(70.0))].into(),
    };
    config.model_map.insert("alias".into(), "upstream".into());
    assert!(allows(&config, Some(&report), "binding", "alias", 1001));
    config.mode = HealthMode::Restrict;
    assert!(!allows(&config, Some(&report), "binding", "alias", 1001));
    assert!(allows(&config, None, "binding", "alias", 1001));
    config.unknown_policy = UnknownPolicy::Block;
    assert!(!allows(&config, None, "binding", "alias", 1001));
    report.models.insert("upstream".into(), Some(99.0));
    assert!(allows(&config, Some(&report), "binding", "alias", 1001));
    for (binding, now) in [("changed", 1001), ("binding", 999), ("binding", 1900)] {
        assert!(!allows(&config, Some(&report), binding, "alias", now));
    }
    config.mode = HealthMode::Advisory;
    assert!(allows(&config, None, "binding", "alias", 1001));
}

#[test]
fn configuration_has_finite_safe_defaults_and_is_additive() {
    let config: SiteHealthConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(config.mode, HealthMode::Advisory);
    assert_eq!(config.unknown_policy, UnknownPolicy::Allow);
    for url in [
        "file:///tmp/x",
        "http://user:pass@fixture.invalid",
        "https://fixture.invalid/?key=secret",
    ] {
        assert!(config.validate(url).is_err());
    }
    let invalid = SiteHealthConfig {
        refresh_interval_secs: 0,
        ..config
    };
    assert!(invalid.validate("https://fixture.invalid/v1").is_err());
}
