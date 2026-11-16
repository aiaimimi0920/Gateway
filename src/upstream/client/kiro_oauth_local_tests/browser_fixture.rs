//! Opt-in real browser host; a new explicit root and file commands own this test's local runtimes.
use super::*;
use std::path::Path;

#[test]
#[ignore = "Requires a new explicit disposable root and a dedicated normal UI browser session"]
fn kiro_oauth_local_ui_fixture() {
    run(1800, serve);
}

fn ready(root: &Path, server: &LocalServer, generation: u64) {
    std::fs::write(root.join("ready.next.json"),serde_json::to_vec_pretty(&json!({"origin":server.origin,
        "port":server.port,"generation":generation,"pid":std::process::id(),"revision":server.revision(),"storageMode":"local"})).unwrap()).unwrap();
    std::fs::rename(root.join("ready.next.json"), root.join("ready.json")).unwrap();
}
async fn serve() {
    let root = std::path::PathBuf::from(
        std::env::var_os("GATEWAY_KIRO_OAUTH_UI_ROOT").expect("Explicit fixture root required"),
    );
    assert!(
        root.is_absolute() && !root.exists(),
        "Use a new absolute fixture directory"
    );
    let config = config(&root);
    let oidc = issuer::Mock::new(vec![
        issuer::registration(),
        issuer::device(),
        issuer::tokens(true, true),
    ])
    .await;
    let upstream = wire::Server::stream(wire::event_frame("local-answer"), false).await;
    let mut server =
        LocalServer::start_with_oidc(config.clone(), &upstream, Some(oidc.origin.clone()), None)
            .await;
    let mut generation = 0;
    let mut sequence = 0;
    let mut boots = Vec::new();
    let mut identity = None;
    ready(&root, &server, generation);
    loop {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let command = match std::fs::read(root.join("command.json")) {
            Ok(bytes) => serde_json::from_slice::<Value>(&bytes).unwrap(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => panic!("Fixture command read failed: {error}"),
        };
        let next = command["sequence"].as_u64().unwrap();
        if next <= sequence {
            continue;
        }
        sequence = next;
        match command["action"].as_str().unwrap() {
            "restart" => {
                assert!(generation < 2);
                let snapshot = server.state.route_config.snapshot();
                let providers = &snapshot.document().providers;
                assert_eq!(providers.len(), 1);
                assert_eq!(providers[0].id, PROVIDER);
                assert_eq!(providers[0].credentials.len(), 1);
                let credential = &providers[0].credentials[0];
                let id = credential.id.clone().unwrap();
                if let Some(expected) = &identity {
                    assert_eq!(&id, expected);
                } else {
                    identity = Some(id.clone());
                }
                let context = credential.kiro_oauth.as_ref().unwrap();
                assert_eq!(context.runtime_target().unwrap().profile_arn(), PROFILE);
                let has_material =
                    credential.api_key.as_deref() == Some("synthetic-console-access");
                assert_eq!(has_material, generation == 0);
                let revision = server.revision();
                let port = server.port;
                let material = credential.clone();
                drop(snapshot);
                server.stop().await;
                server = LocalServer::start_with_oidc(
                    config.clone(),
                    &upstream,
                    Some(oidc.origin.clone()),
                    Some(port),
                )
                .await;
                assert_eq!(server.revision(), revision);
                assert_eq!(
                    serde_json::to_value(
                        &server.state.route_config.snapshot().document().providers[0].credentials
                            [0]
                    )
                    .unwrap(),
                    serde_json::to_value(material).unwrap()
                );
                generation += 1;
                boots.push(json!({"generation":generation,"revision":revision,"credential":id,"materialPresent":has_material}));
                ready(&root, &server, generation);
            }
            "finish" => break,
            action => panic!("Unknown fixture command: {action}"),
        }
    }
    assert_eq!(generation, 2);
    assert_eq!(oidc.count(), 3);
    assert_wire(&upstream, 2);
    server.stop().await;
    oidc.stop().await;
    upstream.stop().await;
    std::fs::write(
        root.join("completed.json"),
        serde_json::to_vec_pretty(&json!({"boots":boots,"publicCalls":2,
        "oidcCalls":3,"materialCleared":true,"provider":PROVIDER,"credential":identity}))
        .unwrap(),
    )
    .unwrap();
}
