use super::*;

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_rechecks_identity_after_startup_and_before_publication() {
    for (name, control, method) in [
        ("revoke-startup", "hold-connect", "connect"),
        ("revoke-publish", "hold-text", "session.sendAndWait"),
    ] {
        let f = fixture("ok").await;
        let identity =
            CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model")
                .unwrap();
        let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
        let slot = Semaphore::new(1);
        let settings = settings(name);
        let root = settings.workspace_root.clone();
        let control = root.join(control);
        std::fs::write(&control, "synthetic fence control").unwrap();
        let execution = run(&identity, &gate, settings, &slot);
        let revoke = async {
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let sent = std::fs::read_dir(&root)
                        .unwrap()
                        .filter_map(Result::ok)
                        .any(|entry| {
                            std::fs::read_to_string(entry.path().join("rpc.jsonl"))
                                .is_ok_and(|log| log.contains(method))
                        });
                    if sent {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            f.replace(|doc| doc.providers[0].credentials[0].enabled = Some(false));
            std::fs::remove_file(control).unwrap();
        };
        let (result, ()) = tokio::join!(execution, revoke);
        assert!(result.is_err());
        assert_eq!(slot.available_permits(), 1);
        assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
        drop(identity);
        f.close().await;
    }
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_PROBE_ROOT and GATEWAY_SYNTHETIC_COPILOT fixture EXE"]
async fn copilot_dispatch_drop_during_startup_quarantines_before_owner_exists() {
    let f = fixture("ok").await;
    let identity =
        CopilotIdentity::bind(&f.console.state, &f.ctx, &f.candidate, "synthetic-model").unwrap();
    let gate = provider_attempt_gate(&f.ctx, &f.console.state, "copilot").unwrap();
    let slot = Semaphore::new(1);
    let settings = settings("startup-cancel");
    let root = settings.workspace_root.clone();
    std::fs::write(root.join("hold-connect"), "synthetic startup control").unwrap();
    let mut execution = Box::pin(run(&identity, &gate, settings.clone(), &slot));
    tokio::select! {
        result = &mut execution => panic!("unexpected completion: {result:?}"),
        _ = async {
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    let connected = std::fs::read_dir(&root).unwrap().filter_map(Result::ok).any(|entry| {
                        std::fs::read_to_string(entry.path().join("rpc.jsonl"))
                            .is_ok_and(|log| log.contains("connect"))
                    });
                    if connected { break; }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }).await.unwrap();
        } => {}
    }
    drop(execution);
    assert_eq!(slot.available_permits(), 0);
    assert!(run(&identity, &gate, settings, &slot).await.is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 2);
    drop(identity);
    f.close().await;
}
