use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues, RouteConfigRuntime};
use neuro_gateway::routing::config::{RouteConfigStore, RouteConfigYaml};
use redis::AsyncCommands;

fn document(provider_id: &str, model: &str) -> RouteConfigYaml {
    serde_yaml::from_str(&format!(
        r#"
providers:
  - id: {provider_id}
    base_url: https://example.com/v1
    api_key: test-secret
    supported_models: [{model}]
model_routes:
  - pattern: {model}
    provider_ids: [{provider_id}]
aliases:
  answer: {model}
"#
    ))
    .unwrap()
}

#[tokio::test]
async fn real_redis_runtime_commit_and_replica_reconcile() {
    let Ok(redis_url) = std::env::var("GATEWAY_CONSOLE_REDIS_TEST_URL") else {
        return;
    };
    let namespace = std::env::var("GATEWAY_CONSOLE_REDIS_TEST_NAMESPACE")
        .unwrap_or_else(|_| format!("console_live_{}", uuid::Uuid::new_v4().simple()));
    let pool = deadpool_redis::Config::from_url(redis_url.clone())
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    let _: () = redis::cmd("FLUSHDB")
        .query_async(&mut connection)
        .await
        .unwrap();
    drop(connection);

    let old = document("old-provider", "old-model");
    let new = document("new-provider", "new-model");
    let temp = TestDirectory::new("console-redis-live");
    let routes = temp.path().join("routes.yaml");
    fs::write(
        &routes,
        neuro_gateway::console::document::canonicalize_route_document(&old)
            .unwrap()
            .canonical_yaml(),
    )
    .unwrap();
    let console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(temp.path().join("state")),
        routes_file: Some(routes.clone()),
        redis_namespace: Some(namespace.clone()),
        ..ConsoleConfigValues::default()
    })
    .unwrap();
    let store = Arc::new(RouteConfigStore::from_document(old.clone()).unwrap());
    let runtime =
        RouteConfigRuntime::new(Arc::clone(&store), &console, pool.clone(), true).unwrap();

    let snapshot = runtime
        .commit_document(
            store.snapshot().revision().id(),
            new.clone(),
            Some("live redis commit".to_string()),
        )
        .await
        .unwrap();

    let mut connection = pool.get().await.unwrap();
    let active_revision_key = format!("gw:console:route-config:{}:active_revision", namespace);
    let active_document_key = format!("gw:console:route-config:{}:active_document", namespace);
    let revision_key = format!(
        "gw:console:route-config:{}:revisions:{}",
        namespace,
        snapshot.revision().id()
    );
    let active_revision: String = connection.get(&active_revision_key).await.unwrap();
    let active_document: String = connection.get(&active_document_key).await.unwrap();
    let revision_payload: String = connection.get(&revision_key).await.unwrap();
    drop(connection);

    assert_eq!(active_revision, snapshot.revision().id());
    let active_document_yaml: RouteConfigYaml = serde_json::from_str(&active_document).unwrap();
    assert_eq!(
        active_document_yaml
            .aliases
            .get("answer")
            .map(String::as_str),
        Some("new-model")
    );
    let revision_json: serde_json::Value = serde_json::from_str(&revision_payload).unwrap();
    assert_eq!(
        revision_json["metadata"]["id"].as_str(),
        Some(snapshot.revision().id())
    );
    assert_eq!(
        store.snapshot().source(),
        neuro_gateway::routing::config::ActiveConfigSource::Redis
    );

    let replica_routes = temp.path().join("replica-routes.yaml");
    fs::write(
        &replica_routes,
        neuro_gateway::console::document::canonicalize_route_document(&old)
            .unwrap()
            .canonical_yaml(),
    )
    .unwrap();
    let replica_console = ConsoleConfig::from_values(ConsoleConfigValues {
        state_dir: Some(temp.path().join("replica-state")),
        routes_file: Some(replica_routes),
        redis_namespace: Some(namespace.clone()),
        ..ConsoleConfigValues::default()
    })
    .unwrap();
    let replica_store = Arc::new(RouteConfigStore::from_document(old).unwrap());
    let replica_runtime = RouteConfigRuntime::new(
        Arc::clone(&replica_store),
        &replica_console,
        pool.clone(),
        false,
    )
    .unwrap();
    let reconciled = replica_runtime
        .reconcile_from_redis()
        .await
        .unwrap()
        .expect("replica should observe the committed Redis revision");

    assert_eq!(reconciled.revision().id(), snapshot.revision().id());
    assert_eq!(
        reconciled.source(),
        neuro_gateway::routing::config::ActiveConfigSource::Redis
    );
    assert_eq!(
        replica_store.snapshot().resolve_alias(Some("answer")),
        Some("new-model".to_string())
    );
}

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!("gateway-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
