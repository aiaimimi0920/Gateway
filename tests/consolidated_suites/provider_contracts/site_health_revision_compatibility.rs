//! Historical health plans must keep their immutable revision identity across upgrades.
use neuro_gateway::console::{
    document::validate_route_document,
    revision::{RevisionActor, RevisionMetadata},
    ConsoleConfig, ConsoleConfigValues, RouteConfigRedisRevision, RouteConfigRuntime,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use time::OffsetDateTime;

struct TestDirectory(std::path::PathBuf);
impl TestDirectory {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("gateway-revision-{}", uuid::Uuid::new_v4())))
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn revision(enabled: Option<bool>) -> Value {
    let document = serde_json::from_value(json!({"providers":[{
        "id":"fixture", "base_url":"https://fixture.invalid/v1", "api_key":"fixture-secret",
        "site_health":{"profiles":[{"id":"health-plan","name":"Health", "policy":{
            "adapter":"sub2api-v2", "auth":{"kind":"bearer","token":"monitor-secret"}
        }}]}
    }]}))
    .unwrap();
    let validated = validate_route_document(document).unwrap();
    let metadata = RevisionMetadata::from_validated(
        7,
        None,
        RevisionActor::ManagementToken,
        OffsetDateTime::UNIX_EPOCH,
        None,
        &validated,
    );
    let mut wire = json!({"metadata":metadata,"document":validated.document()});
    let health = &mut wire["document"]["providers"][0]["site_health"];
    for pointer in ["", "/profiles/0/policy"] {
        let policy = health
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap();
        // Older packages wrote every policy field except the subsequently added enabled flag.
        policy.remove("enabled");
        if let Some(enabled) = enabled {
            policy.insert("enabled".into(), json!(enabled));
        }
    }
    wire.sort_all_objects();
    let document_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&wire["document"]).unwrap(),
    ));
    let yaml_digest = hex::encode(Sha256::digest(
        serde_yaml::to_string(&wire["document"]).unwrap(),
    ));
    wire["metadata"]["documentDigest"] = json!(document_digest);
    wire["metadata"]["yamlDigest"] = json!(yaml_digest);
    wire["metadata"]["id"] = json!(format!("r7-{}", &document_digest[..12]));
    wire.sort_all_objects();
    wire
}

#[test]
fn historical_and_explicit_activation_revisions_keep_exact_digests() {
    for enabled in [None, Some(true), Some(false)] {
        let wire = revision(enabled);
        let restored = RouteConfigRedisRevision::from_json(&wire.to_string()).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&restored.to_json().unwrap()).unwrap(),
            wire
        );
        restored
            .into_store(neuro_gateway::routing::config::ActiveConfigSource::Recovered)
            .unwrap();
    }
}

#[test]
fn tampered_historical_plan_still_fails_digest_verification() {
    let mut wire = revision(None);
    wire["document"]["providers"][0]["site_health"]["profiles"][0]["policy"]["auth"]["token"] =
        json!("tampered");
    let error = RouteConfigRedisRevision::from_json(&wire.to_string()).unwrap_err();
    assert!(error.to_string().contains("does not match"));
}

#[tokio::test]
async fn local_upgrade_restarts_without_rewriting_archives_or_secrets() {
    for enabled in [None, Some(true), Some(false)] {
        let directory = TestDirectory::new();
        let temp = &directory.0;
        let wire = revision(enabled);
        let id = wire["metadata"]["id"].as_str().unwrap();
        let state = temp.join("state");
        let console = state.join("console");
        let archive = console.join("revisions").join(id);
        fs::create_dir_all(&archive).unwrap();
        let yaml = serde_yaml::to_string(&wire["document"]).unwrap();
        let routes = temp.join("routes.yaml");
        fs::write(&routes, &yaml).unwrap();
        let records = [
            (
                "document.json",
                serde_json::to_vec(&wire["document"]).unwrap(),
            ),
            ("routes.yaml", yaml.into_bytes()),
            (
                "metadata.json",
                serde_json::to_vec(
                    &serde_json::from_value::<RevisionMetadata>(wire["metadata"].clone()).unwrap(),
                )
                .unwrap(),
            ),
        ];
        for (name, bytes) in &records {
            fs::write(archive.join(name), bytes).unwrap();
        }
        fs::write(console.join("local-active.json"), json!(id).to_string()).unwrap();
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(state),
            routes_file: Some(routes.clone()),
            ..Default::default()
        })
        .unwrap();
        for _ in 0..2 {
            let runtime = RouteConfigRuntime::new_local(&config).unwrap();
            runtime.recover_startup().await.unwrap();
            let snapshot = runtime.route_config().snapshot();
            assert_eq!(snapshot.revision().id(), id);
            let archived = runtime
                .coordinator()
                .unwrap()
                .load_revision(id)
                .unwrap()
                .unwrap();
            let restored = serde_json::to_value(archived.document()).unwrap();
            assert_eq!(restored, wire["document"]);
            for (name, bytes) in &records {
                assert_eq!(fs::read(archive.join(name)).unwrap(), *bytes);
            }
            assert_eq!(fs::read(&routes).unwrap(), records[1].1);
        }
    }
}
