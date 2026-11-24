//! Real loopback HTTP exercises fixed recipes, auth isolation, group semantics and request budgets.
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use neuro_gateway::site_health::{allows, discovery::discover, ProbePolicy};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

struct Server {
    base: String,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[derive(Clone)]
struct Fixture {
    requests: Arc<Mutex<Vec<(String, String)>>>,
    kind: &'static str,
}
impl Server {
    async fn start(kind: &'static str) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().fallback(any(respond)).with_state(Fixture {
            requests: requests.clone(),
            kind,
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base,
            requests,
            task,
        }
    }
}
async fn respond(State(f): State<Fixture>, req: Request) -> Response {
    let auth = req
        .headers()
        .get("authorization")
        .or_else(|| req.headers().get("x-api-key"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_owned();
    assert!(req.headers().get("cookie").is_none());
    f.requests
        .lock()
        .unwrap()
        .push((req.uri().to_string(), auth));
    if f.kind == "denied" {
        return StatusCode::FORBIDDEN.into_response();
    }
    if f.kind == "redirect" {
        return (
            StatusCode::FOUND,
            [("location", "/unexpected-token-target")],
        )
            .into_response();
    }
    if f.kind == "sub2api" {
        let rows = json!([
            {"group_id":1,"model":"m","metrics":{"request_count":0,"success_rate":0.91},"health":{"overall":"healthy"}},
            {"group_id":2,"model":"m","metrics":{"request_count":0,"success_rate":1.0},"health":{"overall":"healthy"}},
            {"group_id":1,"model":"unknown","metrics":{"success_rate":0.0},"health":{"overall":"unknown"}},
            {"group_id":2,"model":"unknown","metrics":{"success_rate":1.0},"health":{"overall":"healthy"}},
            {"group_id":1,"model":"missing-group","metrics":{"success_rate":1.0},"health":{"overall":"healthy"}},
            {"group_id":9,"model":"not-authorized-scope","metrics":{"success_rate":1.0},"health":{"overall":"healthy"}}
        ]);
        return Json(json!({"code":0,"data":{"group_by":"platform_group_model","items":rows,
            "coverage":{"coverage_complete":false,"aggregation_lag_seconds":60,"computed_at":"2026-10-23T00:00:00Z"}}})).into_response();
    }
    match req.uri().path() {
        "/api/perf-metrics/summary" => {
            let rows: Vec<_> = (0..20)
                .map(|i| json!({"model_name":format!("m{i:02}"),"success_rate":100}))
                .collect();
            Json(json!({"success":true,"data":{"models":rows}})).into_response()
        }
        "/api/perf-metrics" => {
            let query: url::Url = format!("http://fixture.invalid{}", req.uri())
                .parse()
                .unwrap();
            let params: std::collections::HashMap<_, _> =
                query.query_pairs().into_owned().collect();
            let group = if f.kind == "mismatch" {
                "wrong"
            } else {
                params["group"].as_str()
            };
            Json(json!({"success":true,"data":{"model_name":params["model"],"groups":[{"group":group,"success_rate":40}]}})).into_response()
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
fn policy(adapter: &str, groups: Value) -> ProbePolicy {
    serde_json::from_value(
        json!({"adapter":adapter,"mode":"restrict","allow_private_network":true,"monitor_groups":groups,
        "auth":{"kind":"bearer","token":"monitor-only"}}),
    )
    .unwrap()
}

#[tokio::test]
async fn bindings_are_stable_across_model_map_hash_iteration_order() {
    let server = Server::start("new-api").await;
    let mut config = policy("new-api-perf-summary", json!([]));
    config.model_map = (0..32)
        .map(|i| (format!("m{i}"), format!("up{i}")))
        .collect();
    let first = discover("site", &server.base, &config).await.unwrap();
    let reloaded: ProbePolicy =
        serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();
    let second = discover("site", &server.base, &reloaded).await.unwrap();
    assert_eq!(first.binding, second.binding);
}

#[tokio::test]
async fn manual_new_api_queries_exact_groups_with_a_finite_budget_and_no_fallback() {
    let server = Server::start("new-api").await;
    let config = policy("new-api-perf-summary", json!(["plus", "pro"]));
    let report = discover("site", &server.base, &config).await.unwrap();
    assert_eq!(report.probes.len(), 1);
    assert_eq!(report.probes[0].status, "partial_coverage");
    assert_eq!(report.models.len(), 20);
    assert_eq!(report.models.values().filter(|r| r.is_some()).count(), 8);
    assert_eq!(report.models["m00"], Some(40.0));
    assert_eq!(report.models["m19"], None);
    let requests = server.requests.lock().unwrap();
    assert_eq!(
        requests.len(),
        17,
        "one summary plus at most sixteen details"
    );
    assert!(requests
        .iter()
        .all(|(_, auth)| auth == "Bearer monitor-only"));
    assert!(requests
        .iter()
        .all(|(path, _)| path.starts_with("/api/perf-metrics")));
}

#[tokio::test]
async fn sub2api_fractional_rates_redacted_counts_missing_groups_and_coverage_are_preserved() {
    let server = Server::start("sub2api").await;
    let config = policy("sub2api-v2", json!(["1", "2"]));
    let report = discover("site", &server.base, &config).await.unwrap();
    assert_eq!(report.models["m"], Some(91.0));
    assert_eq!(report.models["unknown"], None);
    assert_eq!(report.models["missing-group"], None);
    assert!(!report.models.contains_key("not-authorized-scope"));
    assert_eq!(
        report.probes[0]
            .coverage
            .as_ref()
            .unwrap()
            .aggregation_lag_secs,
        Some(60)
    );
    assert_eq!(report.probes[0].status, "partial_coverage");
    let requests = server.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].0.contains("group_id=1&group_id=2"));
    assert_eq!(requests[0].1, "Bearer monitor-only");
}

#[tokio::test]
async fn monitoring_auth_errors_remain_unknown_and_redirects_never_forward_tokens() {
    for (kind, status) in [
        ("denied", "authentication_required"),
        ("redirect", "http_error"),
        ("mismatch", "supported"),
    ] {
        let server = Server::start(kind).await;
        let config = policy(
            "new-api-perf-summary",
            if kind == "mismatch" {
                json!(["plus"])
            } else {
                json!([])
            },
        );
        let report = discover("site", &server.base, &config).await.unwrap();
        if kind == "mismatch" {
            assert_eq!(report.probes[0].status, "partial_coverage");
            assert!(report.models.values().all(Option::is_none));
        } else {
            assert_eq!(report.probes[0].status, status);
            assert!(report.models.is_empty());
            assert_eq!(server.requests.lock().unwrap().len(), 1);
        }
        assert!(allows(
            &config,
            Some(&report),
            &report.binding,
            "m00",
            report.checked_at
        ));
    }
    let server = Server::start("new-api").await;
    assert!(discover("site", &server.base, &policy("auto", json!([])))
        .await
        .is_err());
    assert!(server.requests.lock().unwrap().is_empty());
}
