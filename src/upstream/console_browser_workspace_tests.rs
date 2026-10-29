use super::*;

#[tokio::test]
async fn console_workspace_uses_unique_owned_temp_and_preserves_neighbors() {
    let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
    let neighbor = ConsoleBrowserWorkspace::create().await.unwrap();
    let root = workspace.path().to_path_buf();
    assert!(root.is_absolute());
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("gateway-console-browser-"));
    assert_ne!(root, neighbor.path());
    tokio::fs::write(root.join("private-fixture"), b"fixture")
        .await
        .unwrap();
    tokio::fs::write(neighbor.path().join("keep"), b"keep")
        .await
        .unwrap();
    let mut command = Command::new("fixture");
    workspace.configure(&mut command);
    for name in ["TEMP", "TMP", "TMPDIR"] {
        assert!(command
            .as_std()
            .get_envs()
            .any(|(key, value)| key == name && value == Some(root.as_os_str())));
    }
    assert!(workspace.cleanup().await);
    assert!(!root.exists());
    assert_eq!(
        tokio::fs::read(neighbor.path().join("keep")).await.unwrap(),
        b"keep"
    );
    assert!(neighbor.cleanup().await);
}

#[tokio::test]
async fn console_workspace_drop_reclaims_files_after_early_return() {
    let workspace = ConsoleBrowserWorkspace::create().await.unwrap();
    let root = workspace.path().to_path_buf();
    tokio::fs::write(root.join("private-fixture"), b"fixture")
        .await
        .unwrap();
    drop(workspace);
    tokio::time::timeout(Duration::from_secs(6), async {
        while root.exists() {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("drop cleanup");
}
