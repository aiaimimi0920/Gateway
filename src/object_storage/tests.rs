use super::*;
use std::time::Duration;
use uuid::Uuid;

mod s3;

#[test]
fn choose_storage_mode_uses_inline_for_small_payloads() {
    let payload = serde_json::json!({
        "adapter": "openai_compatible",
        "apiKey": "small",
    });
    assert_eq!(choose_provider_payload_storage_mode(&payload), "inline");
}

#[test]
fn choose_storage_mode_uses_object_for_large_arrays() {
    let payload = serde_json::json!({
        "adapter": "openai_compatible",
        "apiKeys": vec!["k"; 51],
    });
    assert_eq!(choose_provider_payload_storage_mode(&payload), "r2");
}

#[tokio::test]
async fn local_provider_account_object_key_round_trips_json() {
    let root = std::env::temp_dir().join(format!(
        "neuro-gateway-provider-account-object-{}",
        Uuid::new_v4()
    ));
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };
    let object_key = build_gateway_provider_account_object_key("account-fixture");
    let payload = serde_json::json!({
        "baseUrl": "https://example.invalid",
        "defaultModel": "qwen3-coder-plus"
    });

    storage
        .put_json(&object_key, &payload)
        .await
        .expect("provider account object must be writable");
    assert_eq!(storage.read_json(&object_key).await.unwrap(), payload);

    let replacement = serde_json::json!({
        "baseUrl": "https://replacement.invalid",
        "defaultModel": "gemini-2.5-flash"
    });
    storage
        .put_json(&object_key, &replacement)
        .await
        .expect("provider account object must be replaceable");
    assert_eq!(storage.read_json(&object_key).await.unwrap(), replacement);
    let entries = std::fs::read_dir(root.join("ai-gateway/provider-account"))
        .expect("read provider account object directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect provider account object entries");
    assert_eq!(
        entries.len(),
        1,
        "atomic replacement must clean temporary files"
    );

    assert!(root
        .join("ai-gateway/provider-account/account-fixture.json")
        .exists());

    std::fs::remove_dir_all(root).expect("remove provider account object root");
}

#[tokio::test]
async fn local_operations_serialize_per_root() {
    let root = std::env::temp_dir().join(format!(
        "neuro-gateway-object-operation-lock-{}",
        Uuid::new_v4()
    ));
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };
    let guard = super::local_paths::local_operation_guard(&root).await;
    let mut operation = tokio::spawn(async move {
        storage
            .put_bytes(
                "serialized/object.json",
                b"payload".to_vec(),
                "application/json",
            )
            .await
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut operation)
            .await
            .is_err(),
        "a second local operation must wait for the root owner"
    );
    drop(guard);
    operation
        .await
        .expect("local operation task must join")
        .expect("local operation must succeed");
    std::fs::remove_dir_all(root).expect("remove operation-lock root");
}

#[tokio::test]
async fn local_readiness_probe_round_trips_and_cleans_probe_file() {
    let root =
        std::env::temp_dir().join(format!("neuro-gateway-object-readiness-{}", Uuid::new_v4()));
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };

    let outcome = storage.probe_readiness(Duration::from_secs(1)).await;

    assert!(outcome.ready);
    assert!(!outcome.timed_out);
    let remaining = std::fs::read_dir(&root)
        .expect("read probe root")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect probe root entries");
    assert!(remaining.is_empty(), "readiness probe must clean its file");
    std::fs::remove_dir_all(root).expect("remove probe root");
}

#[tokio::test]
async fn local_readiness_probe_rejects_non_directory_root() {
    let root = std::env::temp_dir().join(format!(
        "neuro-gateway-object-readiness-file-{}",
        Uuid::new_v4()
    ));
    std::fs::write(&root, b"not-a-directory").expect("write invalid probe root");
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };

    let outcome = storage.probe_readiness(Duration::from_secs(1)).await;

    assert!(!outcome.ready);
    assert!(!outcome.timed_out);
    std::fs::remove_file(root).expect("remove invalid probe root");
}

#[tokio::test]
async fn local_object_storage_rejects_unsafe_object_keys() {
    let base = std::env::temp_dir().join(format!("neuro-gateway-object-key-{}", Uuid::new_v4()));
    let root = base.join("root");
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };

    for object_key in [
        "../escape.json",
        "/rooted.json",
        "folder\\escape.json",
        "folder/./escape.json",
        "folder/\0escape.json",
        "folder/file:stream.json",
        "folder/file.",
        "folder/file ",
        "CON",
        "con.txt",
        "folder/PRN.log",
        "AUX",
        "NUL",
        "COM1",
        "LPT9.txt",
    ] {
        let error = storage
            .put_bytes(
                object_key,
                b"must-not-be-written".to_vec(),
                "application/json",
            )
            .await
            .expect_err("unsafe local object key must be rejected");
        assert_eq!(
            error.code.as_deref(),
            Some("object_storage_key_invalid"),
            "unexpected error for object key {object_key:?}: {error}"
        );
    }

    assert!(!base.join("escape.json").exists());
    if base.exists() {
        std::fs::remove_dir_all(base).expect("remove object key test root");
    }
}

#[tokio::test]
async fn local_list_objects_accepts_an_empty_prefix() {
    let root = std::env::temp_dir().join(format!("neuro-gateway-object-list-{}", Uuid::new_v4()));
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };
    storage
        .put_bytes(
            "nested/object.json",
            b"payload".to_vec(),
            "application/json",
        )
        .await
        .expect("write object for empty-prefix listing");

    let objects = storage
        .list_objects("")
        .await
        .expect("empty prefix should list local objects");

    assert_eq!(objects, vec!["nested/object.json"]);
    std::fs::remove_dir_all(root).expect("remove empty-prefix listing root");
}

#[cfg(windows)]
#[tokio::test]
async fn local_object_storage_rejects_junction_escape() {
    let base =
        std::env::temp_dir().join(format!("neuro-gateway-object-junction-{}", Uuid::new_v4()));
    let root = base.join("root");
    let outside = base.join("outside");
    let junction = root.join("linked");
    std::fs::create_dir_all(&root).expect("create local object root");
    std::fs::create_dir_all(&outside).expect("create junction target");
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&outside)
        .output()
        .expect("create test junction");
    assert!(
        output.status.success(),
        "failed to create test junction: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let storage = GatewayObjectStorage {
        driver: ObjectStorageDriver::Local { root: root.clone() },
    };

    let error = storage
        .put_bytes(
            "linked/escape.json",
            b"must-not-escape".to_vec(),
            "application/json",
        )
        .await
        .expect_err("junction escape must be rejected");

    assert_eq!(error.code.as_deref(), Some("object_storage_path_escape"));
    assert!(!outside.join("escape.json").exists());
    std::fs::remove_dir_all(base).expect("remove junction test root");
}

#[cfg(windows)]
#[test]
fn local_list_objects_terminates_on_junction_cycle() {
    const CHILD_ROOT_ENV: &str = "NEURO_GATEWAY_OBJECT_STORAGE_CYCLE_ROOT";

    if let Some(root) = std::env::var_os(CHILD_ROOT_ENV) {
        let storage = GatewayObjectStorage {
            driver: ObjectStorageDriver::Local {
                root: PathBuf::from(root),
            },
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build cycle-test runtime");
        let objects = runtime
            .block_on(storage.list_objects(""))
            .expect("junction cycle must not break object listing");
        assert_eq!(objects, vec!["object.json"]);
        return;
    }

    let base = std::env::temp_dir().join(format!("neuro-gateway-object-cycle-{}", Uuid::new_v4()));
    let root = base.join("root");
    let junction = root.join("cycle");
    std::fs::create_dir_all(&root).expect("create cycle-test object root");
    std::fs::write(root.join("object.json"), b"payload").expect("write cycle-test object");
    let junction_output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&root)
        .output()
        .expect("create cycle-test junction");
    assert!(
        junction_output.status.success(),
        "failed to create cycle-test junction: {}",
        String::from_utf8_lossy(&junction_output.stderr)
    );

    let mut child = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            "object_storage::tests::local_list_objects_terminates_on_junction_cycle",
            "--nocapture",
        ])
        .env(CHILD_ROOT_ENV, &root)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn isolated cycle-test child");

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let completed = loop {
        if child
            .try_wait()
            .expect("poll isolated cycle-test child")
            .is_some()
        {
            break true;
        }
        if std::time::Instant::now() >= deadline {
            child.kill().expect("stop hung cycle-test child");
            break false;
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output = child
        .wait_with_output()
        .expect("collect isolated cycle-test child output");

    std::fs::remove_dir(&junction).expect("remove cycle-test junction");
    std::fs::remove_dir_all(&base).expect("remove cycle-test root");

    assert!(
        completed && output.status.success(),
        "junction cycle listing did not terminate successfully\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
