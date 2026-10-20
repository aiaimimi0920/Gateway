//! Synthetic model generation and disposable local storage; never calls a provider.
use axum::{extract::State, routing::post, Json, Router};
use neuro_gateway::{db, local_runtime::LocalRuntime, routing::candidate::RouteCandidate};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use tokio::sync::{Notify, Semaphore};

#[path = "../console_contract_support/mod.rs"]
pub mod support;
pub use support::*;

pub struct Gate {
    pub entered: Notify,
    pub release: Semaphore,
    pub status: AtomicU16,
}

async fn completion(
    State(gate): State<Arc<Gate>>,
    Json(body): Json<Value>,
) -> (axum::http::StatusCode, Json<Value>) {
    assert_eq!(body["model"], "upstream-model");
    gate.entered.notify_one();
    gate.release.acquire().await.unwrap().forget();
    (
        axum::http::StatusCode::from_u16(gate.status.load(Ordering::SeqCst)).unwrap(),
        Json(
            json!({"choices":[{"message":{"role":"assistant","content":"OK"},
        "finish_reason":"stop"}]}),
        ),
    )
}

pub struct Fixture {
    pub console: ConsoleStateFixture,
    pub local: LocalRuntime,
    pub gate: Arc<Gate>,
    pub grant: String,
    pub candidate: RouteCandidate,
    pub document: neuro_gateway::routing::config::RouteConfigYaml,
    directory: std::path::PathBuf,
    server: tokio::task::JoinHandle<()>,
}

impl Fixture {
    pub async fn new(pooled: bool) -> Self {
        let gate = Arc::new(Gate {
            entered: Notify::new(),
            release: Semaphore::new(0),
            status: AtomicU16::new(200),
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/v1/chat/completions", post(completion))
            .with_state(gate.clone());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut provider = json!({"id":"pool", "adapter":"openai_compatible",
            "protocol_profile":"openai_compatible_generic", "base_url":format!("http://{address}"),
            "api_key":"synthetic-key", "supported_models":["public-model"],
            "model_map":{"public-model":"upstream-model"}});
        if pooled {
            provider["credentials"] = json!([{"id":"one", "api_key":"synthetic-one"}]);
        }
        let document: neuro_gateway::routing::config::RouteConfigYaml =
            serde_json::from_value(json!({"providers":[provider],"model_routes":[],"aliases":{}}))
                .unwrap();
        let mut console = ConsoleStateFixture::new(document.clone(), false);
        let directory = std::env::temp_dir().join(format!(
            "gateway-local-model-recovery-{}",
            uuid::Uuid::new_v4()
        ));
        assert!(
            directory.starts_with("C:/Users/Public/nas_home/AI/GameEditor/linshi"),
            "set TEMP/TMP to linshi"
        );
        let local = LocalRuntime::open(&directory).await.unwrap();
        Arc::get_mut(&mut console.state).unwrap().local_runtime = Some(local.clone());
        let candidate = console
            .state
            .route_config
            .snapshot()
            .resolve_candidates(Some("public-model"))
            .remove(0);
        let grant = grant_console_secret_access(&console.state).await;
        Self {
            console,
            local,
            gate,
            grant,
            candidate,
            document,
            directory,
            server,
        }
    }

    pub async fn probe(&self) -> Value {
        let response = send_console_json(
            &self.console.state,
            axum::http::Method::POST,
            "/v1/internal/gateway/console/providers/pool/probe",
            json!({"prompt":"Reply OK", "model":"public-model"}),
            Some(&self.grant),
            None,
        )
        .await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let value = parse_json(response).await;
        assert!(!value.to_string().contains("synthetic-"));
        value["result"]["results"][0].clone()
    }

    pub fn failure(&self) -> db::RecordCredentialModelFailureInput {
        failure_for(&self.candidate)
    }

    pub async fn cooling(&self) {
        self.local.model_failure(self.failure()).await.unwrap();
    }

    pub async fn finish(self) {
        self.server.abort();
        let _ = self.server.await;
        self.local.close().await;
        drop(self.console);
        drop(self.local);
        for attempt in 0..30 {
            match std::fs::remove_dir_all(&self.directory) {
                Ok(()) => return,
                Err(error) if attempt == 29 => panic!("cleanup: {error}"),
                Err(_) => tokio::time::sleep(std::time::Duration::from_millis(50)).await,
            }
        }
    }
}

pub fn failure_for(candidate: &RouteCandidate) -> db::RecordCredentialModelFailureInput {
    db::RecordCredentialModelFailureInput {
        provider_account_id: candidate.provider_account_id.clone(),
        provider_credential_id: candidate.provider_credential_id.clone(),
        provider_credential_ref: candidate.payload.credential_id.clone(),
        protocol_profile: Some(candidate.protocol_profile.clone()),
        model: candidate.upstream_model.clone().unwrap(),
        upstream_status: Some(429),
        upstream_retry_after_ms: None,
        error_message: Some("rate limit".into()),
        classification: neuro_gateway::provider_failure::classify_provider_failure(
            Some(429),
            None,
            Some("rate limit"),
        ),
    }
}
