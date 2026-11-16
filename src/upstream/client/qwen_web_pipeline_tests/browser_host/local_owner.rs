//! Persistent synthetic fixture only: explicit second process resumes the saved local authority.
use super::*;
use crate::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime};

pub(super) async fn open(root: &Path, resumed: bool) -> (Arc<RouteConfigRuntime>, String) {
    assert!(
        root.is_absolute(),
        "Explicit absolute fixture root required"
    );
    if resumed {
        assert!(
            root.join("saved.json").is_file(),
            "Resume requires successful save/exit receipt"
        );
        assert!(
            !root.join("completed.json").exists(),
            "Completed fixtures cannot be replayed"
        );
    } else {
        assert!(!root.exists(), "Initial fixture requires a new root");
        std::fs::create_dir(root).unwrap();
        let document = serde_json::from_value(document()).unwrap();
        let canonical = crate::console::document::canonicalize_route_document(&document).unwrap();
        std::fs::write(root.join("routes.yaml"), canonical.canonical_yaml()).unwrap();
    }
    let config = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(root.join("state")),
        routes_file: Some(root.join("routes.yaml")),
        ..Default::default()
    })
    .unwrap();
    let runtime = Arc::new(RouteConfigRuntime::new_local(&config).unwrap());
    runtime.recover_startup().await.unwrap();
    let revision = if resumed {
        let saved: Value =
            serde_json::from_slice(&std::fs::read(root.join("saved.json")).unwrap()).unwrap();
        assert_eq!(saved["status"], "saved");
        assert_ne!(
            saved["pid"],
            std::process::id(),
            "Must resume in another process"
        );
        assert_eq!(
            saved["revision"],
            runtime.route_config().snapshot().revision().id()
        );
        let initial: Value =
            serde_json::from_slice(&std::fs::read(root.join("initial.json")).unwrap()).unwrap();
        initial["revision"].as_str().unwrap().to_owned()
    } else {
        let revision = runtime.route_config().snapshot().revision().id().to_owned();
        write(root, "initial.json", json!({"revision":revision}));
        revision
    };
    (runtime, revision)
}
