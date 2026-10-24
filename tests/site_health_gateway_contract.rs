//! Gateway families share bounded readers; legacy plans retain their exact secret audience.
use axum::{
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::any,
    Json, Router,
};
use neuro_gateway::site_health::{discovery::discover, ProbePolicy};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Fixture {
    kind: &'static str,
    calls: Arc<Mutex<Vec<(String, String)>>>,
}
struct Server {
    base: String,
    fixture: Fixture,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Server {
    async fn start(kind: &'static str) -> Self {
        let fixture = Fixture {
            kind,
            calls: Default::default(),
        };
        let app = Router::new()
            .fallback(any(respond))
            .with_state(fixture.clone());
        let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", socket.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(socket, app).await.unwrap();
        });
        Self {
            base,
            fixture,
            task,
        }
    }
}
async fn respond(State(f): State<Fixture>, req: Request) -> Response {
    assert!(req.headers().get("cookie").is_none());
    let auth = ["authorization", "x-api-key", "api-key"]
        .into_iter()
        .filter_map(|h| req.headers().get(h))
        .map(|h| h.to_str().unwrap())
        .collect::<Vec<_>>()
        .join(",");
    f.calls
        .lock()
        .unwrap()
        .push((req.uri().to_string(), auth.clone()));
    let path = req.uri().path();
    if f.kind == "denied" {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    if f.kind == "redirect" {
        return (StatusCode::FOUND, [("location", "/unexpected")]).into_response();
    }
    if f.kind == "liveness" {
        return Json(json!({"success":true,"data":{"system_name":"One API","status":"ok"}}))
            .into_response();
    }
    if f.kind == "failed-auth" && !auth.is_empty() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let status_row = json!({"model_name":"m","success_rate":97,"total_requests":10});
    let body = match (f.kind, path) {
        ("slot", "/api/model_health/slot_status") => {
            let group = if req.uri().query().unwrap_or("").contains("group=second") {
                "second"
            } else {
                "first"
            };
            let rows = if group == "second" {
                json!([])
            } else {
                json!([status_row])
            };
            json!({"success":true,"data":{"enabled":true,"group":group,"models":rows}})
        }
        ("matrix", "/api/v1/channel-monitor-v2/matrix") => {
            json!({"code":0,"data":{"group_by":"platform_group_model","items":[
                {"group_id":1,"model":"m","metrics":{"success_rate":0.98},"health":{"overall":"healthy"}}
            ]}})
        }
        ("multi" | "failed-auth" | "budget", "/api/perf-metrics/summary") => {
            let count = if f.kind == "budget" { 20 } else { 1 };
            json!({"success":true,"data":{"models":(0..count).map(|i|json!({"model_name":format!("m{i}"),"success_rate":99})).collect::<Vec<_>>()}})
        }
        ("budget", "/api/perf-metrics") => {
            let url = url::Url::parse(&format!("http://local{}", req.uri())).unwrap();
            let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
            json!({"success":true,"data":{"model_name":q["model"],"groups":[{"group":q["group"],"success_rate":60}]}})
        }
        ("multi" | "failed-auth", "/api/enhancements/model-status/embed/status/all") => {
            json!({"success":true,"data":[status_row]})
        }
        ("channel" | "multi" | "failed-auth", "/api/channel/health/models") => {
            json!({"success":true,"data":[
                {"model_name":"a","test_count":10,"success_count":9},
                {"model_name":"b","test_count":10,"success_count":8}
            ]})
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    Json(body).into_response()
}
fn policy(adapter: &str, auth: bool) -> ProbePolicy {
    serde_json::from_value(json!({"adapter":adapter,"allow_private_network":true,
        "auth":if auth {json!({"kind":"bearer","token":"monitor-only"})} else {json!({"kind":"none"})}})).unwrap()
}

#[test]
fn families_and_all_legacy_wire_values_roundtrip_without_rewriting() {
    for id in [
        "new-api",
        "sub2api",
        "one-api",
        "compatible-gateway",
        "new-api-perf-summary",
        "model-status-all",
        "channel-health",
        "model-status-embed",
        "model-health-slot",
        "sub2api-v2",
    ] {
        let config = policy(id, false);
        assert_eq!(serde_json::to_value(&config).unwrap()["adapter"], id);
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert_eq!(serde_yaml::from_str::<ProbePolicy>(&yaml).unwrap(), config);
    }
    let mut sub = policy("sub2api", false);
    sub.monitor_groups = vec!["free".into()];
    assert!(sub.validate("https://example.com").is_err());
}

#[tokio::test]
async fn multiple_deployments_reuse_family_and_select_different_internal_readers() {
    for kind in ["multi", "channel"] {
        let server = Server::start(kind).await;
        let result = discover("site", &server.base, &policy("new-api", false))
            .await
            .unwrap();
        assert_eq!(result.probes.len(), 5);
        assert_eq!(result.selected_template.as_deref(), Some("channel-health"));
        assert_eq!(result.models.len(), 2);
        assert!(server
            .fixture
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(path, auth)| auth.is_empty()
                && !path.contains("/api/v1/")
                && !path.contains("/user/")
                && !path.contains("/completions")));
    }
    let server = Server::start("matrix").await;
    let result = discover("site", &server.base, &policy("sub2api", false))
        .await
        .unwrap();
    assert_eq!(result.probes.len(), 1);
    assert_eq!(result.selected_template.as_deref(), Some("sub2api-v2"));
}

#[tokio::test]
async fn public_discovery_never_receives_secrets_and_auth_failure_never_falls_back() {
    for kind in ["multi", "failed-auth"] {
        for auth_kind in ["bearer", "api_key"] {
            let server = Server::start(kind).await;
            let config: ProbePolicy =
                serde_json::from_value(json!({"adapter":"new-api","allow_private_network":true,
                "auth":{"kind":auth_kind,"header":"api-key","token":"monitor-only"}}))
                .unwrap();
            let result = discover("site", &server.base, &config).await.unwrap();
            let calls = server.fixture.calls.lock().unwrap();
            assert_eq!(calls.iter().filter(|(_, a)| a.is_empty()).count(), 4);
            let authenticated: Vec<_> = calls.iter().filter(|(_, a)| !a.is_empty()).collect();
            assert_eq!(authenticated.len(), 1);
            assert!(authenticated[0].0.starts_with("/api/channel/health/models"));
            assert_eq!(
                authenticated[0].1,
                if auth_kind == "bearer" {
                    "Bearer monitor-only"
                } else {
                    "monitor-only"
                }
            );
            if kind == "failed-auth" {
                assert!(result.selected_template.is_none() && result.models.is_empty());
                assert_eq!(result.probes[2].http_status, Some(403));
            } else {
                assert_eq!(result.models.len(), 2);
            }
            assert_eq!(result.probes[0].status, "capability_detected");
        }
    }
}

#[tokio::test]
async fn unrecognized_auth_uses_only_project_owned_default_and_never_guesses_extensions() {
    for kind in ["denied", "redirect"] {
        for adapter in ["new-api", "sub2api", "one-api", "compatible-gateway"] {
            let server = Server::start(kind).await;
            let result = discover("site", &server.base, &policy(adapter, true))
                .await
                .unwrap();
            assert!(result.selected_template.is_none() && result.models.is_empty());
            let calls = server.fixture.calls.lock().unwrap();
            assert!(calls.iter().all(|(p, _)| !p.contains("unexpected")));
            let authenticated: Vec<_> = calls.iter().filter(|(_, a)| !a.is_empty()).collect();
            match adapter {
                "new-api" => {
                    assert_eq!(authenticated.len(), 1);
                    assert!(authenticated[0].0.starts_with("/api/perf-metrics/summary"));
                }
                "sub2api" => {
                    assert_eq!(authenticated.len(), 1);
                    assert!(authenticated[0]
                        .0
                        .starts_with("/api/v1/channel-monitor-v2/matrix"));
                }
                _ => {
                    assert!(authenticated.is_empty());
                    assert_eq!(
                        result.probes.last().unwrap().status,
                        "authentication_target_unconfirmed"
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn one_api_requires_real_health_capability_not_a_liveness_page() {
    let server = Server::start("liveness").await;
    let result = discover("site", &server.base, &policy("one-api", false))
        .await
        .unwrap();
    assert!(result.models.is_empty() && result.selected_template.is_none());
    let server = Server::start("channel").await;
    let result = discover("site", &server.base, &policy("one-api", false))
        .await
        .unwrap();
    assert_eq!(result.selected_template.as_deref(), Some("channel-health"));
}

#[tokio::test]
async fn shallow_discovery_never_publishes_unscoped_or_missing_group_evidence() {
    let server = Server::start("slot").await;
    let mut config = policy("new-api", true);
    config.monitor_groups = vec!["first".into(), "second".into()];
    let result = discover("site", &server.base, &config).await.unwrap();
    assert_eq!(
        result.selected_template.as_deref(),
        Some("model-health-slot")
    );
    assert_eq!(result.models["m"], None);
    let calls = server.fixture.calls.lock().unwrap();
    assert_eq!(
        calls
            .iter()
            .filter(|(p, a)| p.contains("slot_status") && a.is_empty())
            .count(),
        1
    );
    assert_eq!(calls.iter().filter(|(_, a)| !a.is_empty()).count(), 2);
    drop(calls);

    let server = Server::start("budget").await;
    config.monitor_groups = vec!["paid".into()];
    let result = discover("site", &server.base, &config).await.unwrap();
    assert_eq!(
        result.models.values().filter(|v| **v == Some(60.0)).count(),
        16
    );
    assert_eq!(result.models.values().filter(|v| v.is_none()).count(), 4);
    let calls = server.fixture.calls.lock().unwrap();
    // Three shallow public requests + authenticated summary and sixteen details.
    // Channel/embed reject requested groups locally without sending HTTP requests.
    assert_eq!(calls.len(), 20);
    assert!(calls
        .iter()
        .filter(|(p, _)| p.starts_with("/api/perf-metrics?"))
        .all(|(_, a)| !a.is_empty()));
}

#[tokio::test]
async fn legacy_explicit_reader_stays_pinned_even_when_other_extensions_are_available() {
    let server = Server::start("multi").await;
    let result = discover("site", &server.base, &policy("new-api-perf-summary", true))
        .await
        .unwrap();
    assert_eq!(
        result.selected_template.as_deref(),
        Some("new-api-perf-summary")
    );
    let calls = server.fixture.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].1, "Bearer monitor-only");
}
