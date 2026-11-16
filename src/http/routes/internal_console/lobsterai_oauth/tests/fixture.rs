use super::*;
pub(super) use crate::{
    console::lobsterai_oauth::{Session, SessionStatus},
    protocol::lobsterai::{
        self as protocol,
        test_support::{tokens, Peer, Reply},
    },
};
use axum::http::HeaderMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub(super) struct Fixture {
    pub state: Arc<AppState>,
    pub console: crate::console::ConsoleConfig,
}
impl Fixture {
    pub async fn new(doc: RouteConfigYaml) -> Self {
        let root =
            std::env::temp_dir().join(format!("lobster-management-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        std::fs::write(&routes, serde_yaml::to_string(&doc).unwrap()).unwrap();
        let console =
            crate::console::ConsoleConfig::from_values(crate::console::ConsoleConfigValues {
                state_dir: Some(root.join("state")),
                routes_file: Some(routes),
                ..Default::default()
            })
            .unwrap();
        let runtime = Arc::new(crate::console::RouteConfigRuntime::new_local(&console).unwrap());
        runtime.recover_startup().await.unwrap();
        let mut state = support::build_state(runtime.route_config().clone(), Some(runtime));
        Arc::get_mut(&mut state).unwrap().config.storage_mode =
            crate::config::GatewayStorageMode::Local;
        state.redis_pool.close();
        Self { state, console }
    }
}
pub(super) fn catalog() -> Value {
    json!({"code":0,"data":[{"modelId":"opaque/model","modelName":"Synthetic Model","apiFormat":"openai"},{"modelId":"blocked","accessible":false}]})
}
pub(super) fn body(state: &AppState) -> Value {
    json!({"providerId":"lobster","expectedRevision":state.route_config.snapshot().revision().id()})
}
pub(super) async fn create(state: &Arc<AppState>, grant: &str, input: Value, peer: &Peer) -> Value {
    let response = protocol::transport::TEST_ORIGIN
        .scope(peer.origin.clone(), call(state, ROOT, input, Some(grant)))
        .await;
    assert_eq!(
        response.status(),
        StatusCode::OK,
        "{}",
        support::parse_json(response).await
    );
    assert_eq!(response.headers()["cache-control"], "no-store");
    let view = support::parse_json(response).await["session"].clone();
    assert_redacted(&view);
    view
}
pub(super) fn session(state: &Arc<AppState>, view: &Value) -> Arc<Session> {
    let (owner, _) = super::super::super::chatgpt_oauth::authorize(
        state,
        &HeaderMap::new(),
        Some(support::MANAGEMENT_TOKEN),
        None,
        false,
    )
    .unwrap();
    state
        .lifecycle
        .lobsterai_oauth
        .get_owned(view["id"].as_str().unwrap(), &owner)
        .unwrap()
}
pub(super) async fn action(
    state: &Arc<AppState>,
    view: &Value,
    grant: Option<&str>,
    name: &str,
    peer: &Peer,
) -> axum::response::Response {
    protocol::transport::TEST_ORIGIN
        .scope(
            peer.origin.clone(),
            call(
                state,
                &format!("{ROOT}/{}/{name}", view["id"].as_str().unwrap()),
                json!({"expectedRevision":view["expectedRevision"]}),
                grant,
            ),
        )
        .await
}
pub(super) async fn wait_status(session: &Session, status: SessionStatus) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while session.view().status != status {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "expected {status:?}, got {:?} {:?}",
            session.view().status,
            session.view().error_code
        )
    });
}
pub(super) async fn ready(state: &Arc<AppState>, view: &Value) {
    let callback = protocol::test_support::callback(
        view["authorizationUrl"].as_str().unwrap(),
        "synthetic-code",
    );
    assert!(deliver(&callback).await.starts_with("HTTP/1.1 200"));
    wait_status(&session(state, view), SessionStatus::Ready).await;
}
pub(super) async fn deliver(callback: &str) -> String {
    let url = url::Url::parse(callback).unwrap();
    let mut stream =
        tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, url.port().unwrap()))
            .await
            .unwrap();
    stream
        .write_all(
            format!(
                "GET {}?{} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n",
                url.path(),
                url.query().unwrap(),
                url.port().unwrap()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut reply = String::new();
    tokio::time::timeout(Duration::from_secs(3), stream.read_to_string(&mut reply))
        .await
        .unwrap()
        .unwrap();
    reply
}
pub(super) fn assert_redacted(value: &Value) {
    for secret in [
        "synthetic-access",
        "synthetic-refresh",
        "synthetic-code",
        "synthetic-unrelated-secret",
    ] {
        assert!(!value.to_string().contains(secret));
    }
}
pub(super) async fn changed(state: &AppState) {
    let snapshot = state.route_config.snapshot();
    let mut doc = snapshot.document().clone();
    doc.providers[0].label = Some("Concurrent edit".into());
    state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(snapshot.revision().id(), doc, None)
        .await
        .unwrap();
}
