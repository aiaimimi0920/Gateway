//! Opt-in normal UI host: real management TCP/Grant and one synthetic Udio task executor.
use super::*;
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[tokio::test]
#[ignore = "Requires an explicit new fixture root and a dedicated browser session"]
async fn udio_probe_browser_normal_main_tcp_host() {
    tokio::time::timeout(Duration::from_secs(1800), Box::pin(serve()))
        .await
        .unwrap();
}

async fn serve() {
    let root = PathBuf::from(
        std::env::var_os("GATEWAY_UDIO_PROBE_UI_ROOT").expect("Explicit new root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture root"
    );
    std::fs::create_dir(&root).unwrap();
    let peer = Peer::start(complete(), 200, false).await;
    let fixture = Box::pin(Fixture::new(&peer)).await;
    let document = serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
    let revision = fixture
        .state
        .route_config
        .snapshot()
        .revision()
        .id()
        .to_owned();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let app = crate::http::router::build_router(fixture.state.clone());
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
        "pid":std::process::id(),"storageMode":"local","provider":"udio-pipeline",
        "credential":"exact","model":MODEL,"initialBalance":2,"syntheticOnly":true}),
    );
    let completed = loop {
        // Complete snapshots let the UI cancellation observation prove zero task submissions.
        write(
            &root,
            "observed.json",
            json!({"upstreamCalls":peer.calls().len()}),
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        match std::fs::read(root.join("command.json")) {
            Ok(bytes) => {
                let command: Value = serde_json::from_slice(&bytes).unwrap();
                assert!(
                    command == json!({"action":"finish"}) || command == json!({"action":"stop"})
                );
                break command["action"] == "finish";
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
    // A blocked UI can stop its own host cleanly without publishing a success receipt.
    if !completed {
        fixture.finish().await;
        peer.stop().await;
        write(&root, "stopped.json", json!({"status":"not_verified"}));
        panic!("UI verification stopped before completion");
    }
    let row = fixture.audit().await;
    assert_eq!(row.status, "completed");
    closed_audit(&row, true);
    fixture::wire(&peer);
    assert_eq!(peer.calls()[0].1["input"]["waitAudio"], true);
    assert_eq!(fixture.balance().await.remaining_messages, Some(2));
    assert!(fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(Default::default())
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap(),
        document
    );
    assert_eq!(
        fixture.state.route_config.snapshot().revision().id(),
        revision
    );
    let receipt = json!({"status":"passed","ordinaryKeyBalance":2,"audit":row,
        "configurationUnchanged":true,"revisionUnchanged":true,"upstreamCalls":1,
        "audioVerified":false,"modelHealthUnchanged":true,
        "scope":"normal main UI to management TCP and synthetic bound executor; no real Udio"});
    fixture.finish().await;
    peer.stop().await;
    // Publish only after TCP, SQLite and owned executor shutdown have completed.
    write(&root, "completed.json", receipt);
}

fn write(root: &std::path::Path, name: &str, value: Value) {
    let staging = root.join(format!("{name}.staging"));
    std::fs::write(&staging, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    std::fs::rename(staging, root.join(name)).unwrap();
}
