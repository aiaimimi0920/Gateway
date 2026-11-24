//! Exercise the real management router against a non-forwarding SAP peer.
use super::*;

mod orchestration;
use axum::{extract::State, http::Method, response::Response, Router};
use std::sync::Mutex;
use support::{grant_console_secret_access, parse_json, send_console_json, ConsoleStateFixture};

const PATH: &str = "/v1/internal/gateway/console/sap-aicore-deployments";
const ORIGIN: &str = "https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com";
const GROUP: &str = "group-a";
const ACCESS: &str = "synthetic-sap-list-token";

#[derive(Default)]
struct PeerState {
    calls: Mutex<Vec<String>>,
    mode: &'static str,
}

struct DiscoveryPeer {
    origin: String,
    state: Arc<PeerState>,
    task: tokio::task::JoinHandle<()>,
}
impl DiscoveryPeer {
    async fn new(mode: &'static str) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let state = Arc::new(PeerState {
            mode,
            ..Default::default()
        });
        let app = Router::new().fallback(handle).with_state(state.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            origin,
            state,
            task,
        }
    }
    fn fixture(&self) -> ConsoleStateFixture {
        let mut fixture = ConsoleStateFixture::new(
            serde_json::from_value(json!({"providers":[],"model_routes":[]})).unwrap(),
            true,
        );
        Arc::get_mut(&mut fixture.state)
            .unwrap()
            .upstream_client
            .http = rquest::Client::builder()
            .no_proxy()
            .proxy(rquest::Proxy::all(&self.origin).unwrap())
            .default_headers(rquest::header::HeaderMap::from_iter([(
                rquest::header::COOKIE,
                "synthetic-cookie=not-for-sap".parse().unwrap(),
            )]))
            .redirect(rquest::redirect::Policy::limited(3))
            .build()
            .unwrap();
        fixture
    }
}
impl Drop for DiscoveryPeer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn deployment(id: &str, model: &str) -> Value {
    json!({"id":id,"deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/{id}"),"status":"RUNNING",
      "scenarioId":"foundation-models","executableId":"azure-openai",
      "details":{"resources":{"backendDetails":{"model":{"name":model,"version":"2024-11-20"}}}},
      "configurationName":"synthetic-do-not-reflect"})
}

fn bedrock_deployment(id: &str) -> Value {
    json!({"id":id,"deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/{id}"),"status":"RUNNING",
      "scenarioId":"foundation-models","executableId":"aws-bedrock", "configurationName":"bedrock-config"})
}

async fn handle(State(state): State<Arc<PeerState>>, request: Request<Body>) -> Response {
    let path = request.uri().path().to_owned();
    state.calls.lock().unwrap().push(path.clone());
    assert!(request
        .uri()
        .scheme_str()
        .is_none_or(|scheme| scheme == "http"));
    assert!(!request.headers().contains_key("cookie"));
    assert!(!request.headers().contains_key("x-management-token"));
    let payload = if path == "/oauth/token" {
        use base64::{engine::general_purpose::STANDARD, Engine};
        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.headers()["authorization"],
            format!(
                "Basic {}",
                STANDARD.encode("synthetic-sap-client:synthetic-sap-secret")
            )
        );
        assert!(!request.headers().contains_key("AI-Resource-Group"));
        assert_eq!(
            drain(request.into_body()).await,
            "grant_type=client_credentials"
        );
        json!({"access_token":ACCESS,"token_type":"bearer","expires_in":3600})
    } else {
        assert_eq!(request.method(), Method::GET);
        assert_eq!(path, "/v2/lm/deployments");
        assert_eq!(
            request.headers()["authorization"],
            if state.mode == "x509" {
                "Bearer synthetic-sap-access".to_owned()
            } else {
                format!("Bearer {ACCESS}")
            }
        );
        assert_eq!(request.headers()["AI-Resource-Group"], GROUP);
        let query: std::collections::BTreeMap<String, String> =
            url::form_urlencoded::parse(request.uri().query().unwrap().as_bytes())
                .into_owned()
                .collect();
        if state.mode == "orchestration" {
            assert_eq!(query["scenarioId"], "orchestration");
            assert!(!query.contains_key("executableIds"));
        } else {
            assert_eq!(query["scenarioId"], "foundation-models");
            assert_eq!(
                query["executableIds"],
                if state.mode == "bedrock" {
                    "aws-bedrock"
                } else {
                    "azure-openai"
                }
            );
        }
        assert_eq!(query["status"], "RUNNING");
        assert_eq!(query["$top"], "50");
        match state.mode {
            "timeout" => return std::future::pending().await,
            "redirect" => {
                return Response::builder()
                    .status(302)
                    .header("location", "/never-follow")
                    .body(Body::from("private-error"))
                    .unwrap()
            }
            "large" => {
                return Response::builder()
                    .status(200)
                    .body(Body::from(vec![b'x'; 512 * 1024 + 1]))
                    .unwrap()
            }
            "azure" => {
                let mut local = deployment("azure-local", "gpt-4o");
                local["deploymentUrl"] = json!(format!(
                    "{}/v2/inference/deployments/azure-local",
                    super::super::test_support::AZURE_ORIGIN
                ));
                json!({"count":2,"resources":[local, deployment("foreign-aws", "gpt-4o")]})
            }
            "orchestration" => orchestration::catalog(),
            "bedrock" => {
                let mut labeled = bedrock_deployment("labeled");
                labeled["details"] = json!({"resources":{"backendDetails":{"model":{"name":"anthropic--claude-sonnet-4","version":"1.0"}}}});
                let mut invalid_configuration = bedrock_deployment("id-only");
                invalid_configuration["configurationName"] = json!("bad\nlabel");
                invalid_configuration["details"] = json!({"unrelated":"shape"});
                let mut other_origin = bedrock_deployment("other-origin");
                other_origin["deploymentUrl"] = json!(format!(
                    "{}/v2/inference/deployments/other-origin",
                    super::super::test_support::AZURE_ORIGIN
                ));
                let mut other_id = bedrock_deployment("other-id");
                other_id["deploymentUrl"] = json!(super::super::test_support::BASE);
                let mut wrong_scenario = bedrock_deployment("other-scenario");
                wrong_scenario["scenarioId"] = json!("orchestration");
                let mut stopped = bedrock_deployment("stopped-bedrock");
                stopped["status"] = json!("STOPPED");
                json!({"resources":[labeled, invalid_configuration, bedrock_deployment("bare"),
                    other_origin, other_id, wrong_scenario, stopped, deployment("azure", "gpt-4o")]})
            }
            "full" => json!({"resources":vec![deployment("one", "gpt-4o"); 50]}),
            _ if query["$skip"] == "0" => {
                let mut wrong_origin = deployment("foreign", "gpt-4o");
                wrong_origin["deploymentUrl"] = json!("https://api.ai.prod.us-east-1.aws.ml.hana.ondemand.com/v2/inference/deployments/foreign");
                let mut wrong_id = deployment("wrong-id", "gpt-4o");
                wrong_id["deploymentUrl"] = json!(super::super::test_support::BASE);
                let mut stopped = deployment("stopped", "gpt-4o");
                stopped["status"] = json!("STOPPED");
                let mut other = deployment("other", "gpt-4o");
                other["executableId"] = json!("orchestration");
                let mut missing = deployment("missing", "gpt-4o");
                missing.as_object_mut().unwrap().remove("details");
                let mut rows = vec![
                    deployment("z", "gpt-4o"),
                    wrong_origin,
                    wrong_id,
                    stopped,
                    other,
                    missing,
                    deployment("embed", "text-embedding-3-small"),
                    deployment("unknown", "gpt-synthetic"),
                    {
                        let mut cross = deployment("bedrock-with-azure-label", "gpt-4o");
                        cross["executableId"] = json!("aws-bedrock");
                        cross
                    },
                ];
                rows.resize(50, deployment("embed", "text-embedding-3-small"));
                json!({"count":50,"resources":rows})
            }
            _ => {
                assert_eq!(query["$skip"], "50");
                json!({"count":2,"resources":[deployment("a", "gpt-4.1"),deployment("z", "gpt-4o")]})
            }
        }
    };
    axum::response::IntoResponse::into_response(axum::Json(payload))
}

#[tokio::test]
async fn sap_aicore_azure_eu20_discovery_only_returns_matching_origin() {
    let peer = DiscoveryPeer::new("azure").await;
    let fixture = peer.fixture();
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = super::super::test_support::origin(
        peer.origin.clone(),
        send_console_json(
            &fixture.state,
            Method::POST,
            PATH,
            json!({"serviceKey":super::super::test_support::azure_key(),"resourceGroup":GROUP}),
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let result = parse_json(response).await;
    assert_eq!(result["deployments"].as_array().unwrap().len(), 1);
    assert_eq!(result["deployments"][0]["id"], "azure-local");
    assert_eq!(
        *peer.state.calls.lock().unwrap(),
        ["/oauth/token", "/v2/lm/deployments"]
    );
}

#[tokio::test]
async fn sap_aicore_discovery_protected_read_only_and_filtered() {
    let peer = DiscoveryPeer::new("listed").await;
    let fixture = peer.fixture();
    let request = json!({"serviceKey":KEY,"resourceGroup":GROUP});
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        PATH,
        request.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let grant = grant_console_secret_access(&fixture.state).await;
    for invalid in [
        json!({"serviceKey":KEY,"resourceGroup":"bad group"}),
        json!({"serviceKey":"{}","resourceGroup":GROUP}),
        json!({"serviceKey":KEY,"resourceGroup":GROUP,"baseUrl":"https://example.invalid"}),
        json!({"serviceKey":KEY,"resourceGroup":GROUP,"protocol":"aws-bedrock"}),
        json!({"serviceKey":KEY,"resourceGroup":GROUP,"protocol":"bedrock","executableIds":"orchestration"}),
    ] {
        let response = send_console_json(
            &fixture.state,
            Method::POST,
            PATH,
            invalid,
            Some(&grant),
            None,
        )
        .await;
        assert!(response.status().is_client_error());
    }
    assert!(peer.state.calls.lock().unwrap().is_empty());
    let response = super::super::test_support::origin(
        peer.origin.clone(),
        send_console_json(
            &fixture.state,
            Method::POST,
            PATH,
            request,
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result = parse_json(response).await;
    assert_eq!(
        result,
        json!({"deployments":[
        {"id":"a","deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/a"),"modelName":"gpt-4.1","modelVersion":"2024-11-20"},
        {"id":"z","deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/z"),"modelName":"gpt-4o","modelVersion":"2024-11-20"}
    ],"truncated":false})
    );
    assert_eq!(
        *peer.state.calls.lock().unwrap(),
        ["/oauth/token", "/v2/lm/deployments", "/v2/lm/deployments"]
    );
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    assert!(fixture
        .state
        .route_config
        .snapshot()
        .document()
        .providers
        .is_empty());
}

#[tokio::test]
async fn sap_aicore_bedrock_discovery_is_read_only_and_does_not_require_model_metadata() {
    let peer = DiscoveryPeer::new("bedrock").await;
    let fixture = peer.fixture();
    let request = json!({"serviceKey":KEY,"resourceGroup":GROUP,"protocol":"bedrock"});
    let before = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = send_console_json(
        &fixture.state,
        Method::POST,
        PATH,
        request.clone(),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(peer.state.calls.lock().unwrap().is_empty());
    let grant = grant_console_secret_access(&fixture.state).await;
    let response = super::super::test_support::origin(
        peer.origin.clone(),
        send_console_json(
            &fixture.state,
            Method::POST,
            PATH,
            request,
            Some(&grant),
            None,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(
        parse_json(response).await,
        json!({"deployments":[
        {"id":"bare","deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/bare"),"configurationName":"bedrock-config"},
        {"id":"id-only","deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/id-only")},
        {"id":"labeled","deploymentUrl":format!("{ORIGIN}/v2/inference/deployments/labeled"),"configurationName":"bedrock-config","modelName":"anthropic--claude-sonnet-4","modelVersion":"1.0"}
    ],"truncated":false})
    );
    assert_eq!(
        *peer.state.calls.lock().unwrap(),
        ["/oauth/token", "/v2/lm/deployments"]
    );
    let after = fixture.state.route_config.snapshot();
    assert_eq!(after.revision().id(), before);
    assert!(after.document().providers.is_empty());
}

#[tokio::test]
async fn sap_aicore_discovery_bounds_and_redacts_upstream_failures() {
    for mode in ["redirect", "large", "full", "timeout"] {
        let peer = DiscoveryPeer::new(mode).await;
        let fixture = peer.fixture();
        let grant = grant_console_secret_access(&fixture.state).await;
        let response = super::super::test_support::origin(
            peer.origin.clone(),
            send_console_json(
                &fixture.state,
                Method::POST,
                PATH,
                json!({"serviceKey":KEY,"resourceGroup":GROUP}),
                Some(&grant),
                None,
            ),
        )
        .await;
        if mode == "full" {
            assert_eq!(response.status(), StatusCode::OK);
            let result = parse_json(response).await;
            assert_eq!(result["truncated"], true);
            assert_eq!(result["deployments"].as_array().unwrap().len(), 1);
            assert_eq!(peer.state.calls.lock().unwrap().len(), 6);
        } else {
            assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
            let result = drain(response.into_body()).await;
            let expected = if mode == "timeout" {
                "sap_aicore_deployments_timeout"
            } else {
                "sap_aicore_deployments_failed"
            };
            assert!(result.contains(expected), "{result}");
            for secret in ["private-error", "synthetic-sap-secret", ACCESS] {
                assert!(!result.contains(secret));
            }
            assert_eq!(peer.state.calls.lock().unwrap().len(), 2);
        }
    }
}

#[tokio::test]
async fn sap_aicore_x509_deployment_lookup_uses_bearer_and_does_not_save_key() {
    use super::super::test_support;
    use super::mtls_tests::{root_pem, OAuthPeer};
    let oauth = OAuthPeer::start().await;
    let peer = DiscoveryPeer::new("x509").await;
    let fixture = peer.fixture();
    let grant = grant_console_secret_access(&fixture.state).await;
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let response = test_support::origin(
        peer.origin.clone(),
        test_support::mtls(
            oauth.origin.clone(),
            root_pem(),
            send_console_json(
                &fixture.state,
                Method::POST,
                PATH,
                json!({"serviceKey": mtls_fixtures::service_key(false).to_string(), "resourceGroup": GROUP}),
                Some(&grant),
                None,
            ),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result = parse_json(response).await;
    assert_eq!(result["deployments"][0]["id"], "a");
    assert!(!result.to_string().contains("PRIVATE KEY"));
    oauth.assert_calls(&["synthetic-sap-client"]);
    assert_eq!(
        *peer.state.calls.lock().unwrap(),
        ["/v2/lm/deployments", "/v2/lm/deployments"]
    );
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    assert!(fixture
        .state
        .route_config
        .snapshot()
        .document()
        .providers
        .is_empty());
}
