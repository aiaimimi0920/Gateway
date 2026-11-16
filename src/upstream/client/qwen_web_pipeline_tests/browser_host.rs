//! Opt-in normal management UI: replace synthetic session material, then explicitly probe it.
use super::*;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};

const KEY: &str = "synthetic-ui-exact";
const COOKIE: &str = "token=synthetic-ui-exact; cf_clearance=synthetic-ui-clearance";
const UA: &str = "synthetic-ui-UA";
#[path = "browser_host/local_owner.rs"]
mod local_owner;

#[test]
#[ignore = "Requires a new fixture root and a dedicated browser session"]
fn qwen_web_browser_material_save_and_exact_probe_host() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(1800), serve())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_QWEN_PROBE_UI_ROOT").expect("Explicit new root required"),
    );
    let resumed = std::env::var("GATEWAY_QWEN_PROBE_UI_RESUME").as_deref() == Ok("1");
    let (runtime, revision) = local_owner::open(&root, resumed).await;
    let peer = wire::Server::start(Default::default()).await;
    // Reuse only the AppState fixture; all configuration authority is production file-backed.
    let mut console =
        support::ConsoleStateFixture::new(serde_json::from_value(document()).unwrap(), false);
    let local = LocalRuntime::open(&root.join("local")).await.unwrap();
    let state = Arc::get_mut(&mut console.state).unwrap();
    state.config.storage_mode = crate::config::GatewayStorageMode::Local;
    state.route_config = runtime.route_config().clone();
    state.route_config_runtime = Some(runtime);
    state.local_runtime = Some(local.clone());
    state.upstream_client = super::super::UpstreamClient::new(5);
    state.upstream_client.http = peer.client();
    let fixture = Fixture { console, local };
    let original = crate::routing::config::RouteConfigStore::from_document(
        serde_json::from_value(document()).unwrap(),
    )
    .unwrap();
    let mut expected = serde_json::to_value(original.snapshot().document()).unwrap();
    expected["providers"][0]["credentials"][1]["api_key"] = json!(KEY);
    expected["providers"][0]["credentials"][1]["headers"]["Cookie"] = json!(COOKIE);
    expected["providers"][0]["credentials"][1]["headers"]["User-Agent"] = json!(UA);
    expected["providers"][0]["credentials"][1]["session_auth"] =
        json!({"transport":"bearer","headerName":"authorization"});
    // The normal editor also materializes enabled/model ownership, then reconciles
    // the provider union. These existing transformations are not session mutations.
    expected["providers"][0]["credentials"][1]["enabled"] = json!(true);
    expected["providers"][0]["credentials"][1]["model_selection"] =
        json!({"fetched":[],"manual":["public-alias"]});
    expected["providers"][0]["supported_models"] = json!(["other-model", "public-alias"]);
    // Compare typed documents, including serde defaults introduced by normal material import.
    expected = serde_json::to_value(
        serde_json::from_value::<crate::routing::config::RouteConfigYaml>(expected).unwrap(),
    )
    .unwrap();
    if resumed {
        assert_eq!(
            serde_json::to_value(fixture.console.state.route_config.snapshot().document()).unwrap(),
            expected
        );
    }
    let origin = peer.origin.clone();
    // Test-only task binding follows axum's connection task; never enables real-network fallback.
    let app = build_router(fixture.console.state.clone()).layer(axum::middleware::from_fn(
        move |request: axum::extract::Request, next: axum::middleware::Next| {
            let origin = origin.clone();
            async move { wire::within(&origin, next.run(request)).await }
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async {
            let _ = stopped.await;
        })
        .await
        .unwrap();
    });
    write(
        &root,
        "ready.json",
        json!({"origin":format!("http://{address}"),
        "pid":std::process::id(),"provider":"qwen-pipeline","credential":"exact",
        "model":"public-alias","syntheticOnly":true,"resumed":resumed}),
    );
    let completed = loop {
        let current = fixture.console.state.route_config.snapshot();
        write(
            &root,
            "observed.json",
            json!({"upstreamCalls":peer.calls().len(),
            "materialSaved":serde_json::to_value(current.document()).unwrap() == expected,
            "revisionChanged":current.revision().id() != revision}),
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join(if resumed {
            "resume-command.json"
        } else {
            "command.json"
        })) {
            Ok(bytes) => {
                let command: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(
                    command == json!({"action":"finish"})
                        || command == json!({"action":"stop"})
                        || (!resumed && command == json!({"action":"save-exit"}))
                );
                break command["action"].as_str().unwrap().to_owned();
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => panic!("Fixture command read failed: {error}"),
        }
    };
    shutdown.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    if completed == "stop" {
        fixture.finish().await;
        peer.finish().await;
        write(&root, "stopped.json", json!({"status":"not_verified"}));
        panic!("UI verification stopped before completion");
    }
    let current = fixture.console.state.route_config.snapshot();
    assert_eq!(
        serde_json::to_value(current.document()).unwrap(),
        expected,
        "Only selected material and the normal editor model metadata may change"
    );
    assert_ne!(current.revision().id(), revision);
    if completed == "save-exit" {
        assert!(peer.calls().is_empty(), "Saving must not generate");
        assert!(fixture
            .local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap()
            .is_empty());
        let receipt = json!({"status":"saved","revision":current.revision().id(),"pid":std::process::id(),"upstreamCalls":0});
        drop(current);
        fixture.finish().await;
        peer.finish().await;
        write(&root, "saved.json", receipt);
        return;
    }
    let calls = peer.calls();
    assert_eq!(calls.len(), 2, "one create and one completion, no replay");
    for call in &calls {
        assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
        assert_eq!(call.headers["cookie"], COOKIE);
        assert_eq!(call.headers["user-agent"], UA);
    }
    assert_eq!(calls[1].body["model"], "qwen3.7-plus");
    let rows = fixture.audits().await;
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.status, "completed");
    assert_eq!(row.total_tokens, Some(5));
    assert_eq!(row.resolved_model.as_deref(), Some("qwen3.7-plus"));
    assert!(row.access_key_id.is_none());
    assert_eq!(
        row.route_trace.as_ref().unwrap()["realCredentialRef"],
        "exact"
    );
    assert_eq!(row.route_trace.as_ref().unwrap()["consoleModelProbe"], true);
    let serialized = serde_json::to_string(&rows).unwrap();
    for secret in [KEY, COOKIE, UA, "synthetic private probe prompt"] {
        assert!(!serialized.contains(secret));
    }
    let receipt = json!({"status":"passed","materialSaved":true,"revisionChanged":true,
        "otherAccountsUnchanged":true,"upstreamCalls":2,"audit":row,
        "resumed":resumed,"pid":std::process::id(),
        "scope":"normal main UI, real local authority, management TCP and synthetic Qwen peer; no real platform"});
    drop(current);
    fixture.finish().await;
    peer.finish().await;
    write(&root, "completed.json", receipt);
}

fn write(root: &Path, name: &str, value: Value) {
    let staging = root.join(format!("{name}.staging"));
    std::fs::write(&staging, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(staging, root.join(name)).unwrap();
}
