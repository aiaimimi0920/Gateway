//! Synthetic pre-SG revisions retain their JSON/YAML identity and immutable archives on upgrade.
use neuro_gateway::console::{
    document::validate_route_document,
    refresh_grants::GrantRequest,
    revision::{RevisionActor, RevisionMetadata},
    ConsoleConfig, ConsoleConfigValues, RouteConfigRedisRevision, RouteConfigRuntime,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use time::OffsetDateTime;

fn old_wire() -> Value {
    let document = serde_json::from_value(json!({"providers":[{"id":"oauth", "preset":"accio",
        "base_url":"https://phoenix-gw.alibaba.com", "explicit_credentials_only":true,
        "credentials":[{"id":"account", "api_key":"old-access", "refresh_token":"old-refresh",
        "refresh_endpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token",
        "refresh_client_id":"synthetic-client", "expires_at":"2020-01-01T00:00:00Z",
        "headers":{"utdid":"synthetic-device"}}]}]}))
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
    let mut wire = json!({"metadata":metadata, "document":validated.document()});
    // The old writer could not emit this field. Recompute both legacy digests independently.
    wire["document"]["providers"][0]["credentials"][0]
        .as_object_mut()
        .unwrap()
        .remove("accio_sgcookie");
    wire.sort_all_objects();
    let document_digest = hex::encode(Sha256::digest(
        serde_json::to_vec(&wire["document"]).unwrap(),
    ));
    let yaml_digest = hex::encode(Sha256::digest(
        serde_yaml::to_string(&wire["document"]).unwrap(),
    ));
    wire["metadata"]["id"] = json!(format!("r7-{}", &document_digest[..12]));
    wire["metadata"]["documentDigest"] = json!(document_digest);
    wire["metadata"]["yamlDigest"] = json!(yaml_digest);
    wire.sort_all_objects();
    wire
}

#[test]
fn accio_pre_sg_revision_keeps_exact_wire_and_legacy_material_id() {
    let wire = old_wire();
    let restored = RouteConfigRedisRevision::from_json(&wire.to_string()).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&restored.to_json().unwrap()).unwrap(),
        wire
    );
    let request = GrantRequest::from_revision(&restored, "oauth", "account").unwrap();
    assert_eq!(
        request.material_id(),
        "f6c29a449f19067a2a35629ccf9f8170948a9dea4dacaa057fe8889393104230"
    );
    let mut tampered = wire;
    tampered["document"]["providers"][0]["credentials"][0]["accio_sgcookie"] =
        json!("tampered-cookie");
    assert!(RouteConfigRedisRevision::from_json(&tampered.to_string()).is_err());
}

#[tokio::test]
async fn accio_pre_sg_local_archive_restarts_twice_without_rewriting_secrets_or_digests() {
    let root = std::env::temp_dir().join(format!("gateway-accio-legacy-{}", uuid::Uuid::new_v4()));
    let wire = old_wire();
    let id = wire["metadata"]["id"].as_str().unwrap();
    let state = root.join("state");
    let console = state.join("console");
    let archive = console.join("revisions").join(id);
    fs::create_dir_all(&archive).unwrap();
    let yaml = serde_yaml::to_string(&wire["document"]).unwrap();
    let routes = root.join("routes.yaml");
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
        assert_eq!(
            serde_json::to_value(archived.document()).unwrap(),
            wire["document"]
        );
        assert_eq!(
            archived.document().providers[0].credentials[0]
                .id
                .as_deref(),
            Some("account")
        );
        assert_eq!(
            archived.document().providers[0].credentials[0]
                .api_key
                .as_deref(),
            Some("old-access")
        );
        assert!(archived.document().providers[0].credentials[0]
            .accio_sgcookie
            .is_none());
        for (name, bytes) in &records {
            assert_eq!(fs::read(archive.join(name)).unwrap(), *bytes);
        }
        assert_eq!(fs::read(&routes).unwrap(), records[1].1);
    }
}
