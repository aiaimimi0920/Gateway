//! Session imports use the real secret resolver and persisted route reader, never upstream I/O.
use super::document;
use neuro_gateway::console::secrets::{redact_route_document, resolve_secret_patches, SecretPatch};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde::Deserialize;
use serde_json::json;

#[test]
fn session_header_case_changes_clear_each_old_identity() {
    for keys in [
        vec!["cookie"],
        vec!["Cookie"],
        vec!["cookie", "Cookie", "COOKIE"],
    ] {
        let mut active = document("providers:\n- id: p\n  base_url: https://grok.com\n  credentials:\n  - id: a\nmodel_routes: []\naliases: {}\n");
        let credential = &mut active.providers[0].credentials[0];
        for key in &keys {
            credential
                .headers
                .insert((*key).into(), "synthetic-old".into());
        }
        let redacted = redact_route_document(&active).unwrap();
        let mut patches: Vec<_> = keys
            .iter()
            .filter(|key| **key != "Cookie")
            .map(|key| SecretPatch::clear(format!("/providers/0/credentials/0/headers/{key}")))
            .collect();
        patches.push(SecretPatch::replace(
            "/providers/0/credentials/0/headers/Cookie",
            json!("sso=synthetic-new"),
        ));
        let resolved =
            resolve_secret_patches(&active, redacted.document.clone(), &patches).unwrap();
        let headers = &resolved.providers[0].credentials[0].headers;
        assert_eq!(headers.len(), 1);
        assert_eq!(headers["Cookie"], "sso=synthetic-new");
        let reloaded = redact_route_document(&resolved).unwrap();
        assert!(!serde_json::to_string(&reloaded.document)
            .unwrap()
            .contains("synthetic-new"));
        let clears: Vec<_> = keys
            .iter()
            .map(|key| SecretPatch::clear(format!("/providers/0/credentials/0/headers/{key}")))
            .collect();
        let cleared = resolve_secret_patches(&active, redacted.document, &clears).unwrap();
        assert!(cleared.providers[0].credentials[0].headers.is_empty());
    }
}

#[test]
fn session_header_new_casing_does_not_implicitly_delete_old_identity() {
    let active = document("providers:\n- id: p\n  base_url: https://grok.com\n  credentials:\n  - id: a\n    headers:\n      cookie: synthetic-old\nmodel_routes: []\naliases: {}\n");
    let draft = redact_route_document(&active).unwrap().document;
    let error = resolve_secret_patches(
        &active,
        draft,
        &[SecretPatch::replace(
            "/providers/0/credentials/0/headers/Cookie",
            json!("sso=synthetic-new"),
        )],
    )
    .expect_err("original exact path needs clear");
    assert_eq!(error.code(), "secret_patch_missing");
}

#[test]
fn session_runtime_ids_are_metadata_not_secret_patch_targets() {
    let active = document("providers:\n- id: p\n  base_url: https://freebuff.com\n  credentials:\n  - id: a\n    extra_body:\n      realmId: board\n      freebuffAgentId: agent\nmodel_routes: []\naliases: {}\n");
    let draft = redact_route_document(&active).unwrap().document;
    assert_eq!(
        draft.providers[0].credentials[0].extra_body["realmId"],
        json!("board")
    );
    for field in ["realmId", "freebuffAgentId"] {
        let error = resolve_secret_patches(
            &active,
            draft.clone(),
            &[SecretPatch::replace(
                format!("/providers/0/credentials/0/extra_body/{field}"),
                json!("other"),
            )],
        )
        .expect_err("metadata cannot masquerade as secret");
        assert_eq!(error.code(), "secret_path_outside_schema");
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FrontendFixture {
    product: String,
    active: RouteConfigYaml,
    document: RouteConfigYaml,
    secret_patches: Vec<SecretPatch>,
}

#[test]
#[ignore = "requires explicitly exported synthetic frontend fixtures"]
fn session_material_frontend_to_secret_resolver_roundtrip() {
    let path =
        std::env::var("GATEWAY_SESSION_MATERIAL_FIXTURE_PATH").expect("synthetic fixture path");
    let fixtures: Vec<FrontendFixture> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(fixtures.len(), 8);
    for fixture in fixtures {
        let resolved =
            resolve_secret_patches(&fixture.active, fixture.document, &fixture.secret_patches)
                .unwrap_or_else(|error| panic!("{}: {error:?}", fixture.product));
        let credential = &resolved.providers[1].credentials[0];
        assert_eq!(credential.api_key.as_deref(), Some("synthetic-new-token"));
        assert_eq!(credential.refresh_token, None);
        assert!(!credential.headers.contains_key("cookie"));
        assert!(!credential.headers.contains_key("authorization"));
        if fixture.product == "lumalabs-web" {
            assert_eq!(credential.extra_body["realmId"], json!("synthetic-board"));
        }
        if fixture.product == "freebuff-web" {
            assert_eq!(
                credential.extra_body["freebuffAgentId"],
                json!("synthetic-agent")
            );
        }
        let encoded = serde_yaml::to_string(&resolved).unwrap();
        let reloaded: RouteConfigYaml = serde_yaml::from_str(&encoded).unwrap();
        let redacted = redact_route_document(&reloaded).unwrap();
        assert!(!serde_json::to_string(&redacted.document)
            .unwrap()
            .contains("synthetic-new-token"));
        let keeps: Vec<_> = redacted
            .secrets
            .iter()
            .filter(|secret| secret.configured)
            .map(|secret| SecretPatch::keep(&secret.path))
            .collect();
        let second = resolve_secret_patches(&reloaded, redacted.document, &keeps).unwrap();
        assert_eq!(
            second.providers[1].credentials[0].api_key,
            credential.api_key
        );
    }
}
