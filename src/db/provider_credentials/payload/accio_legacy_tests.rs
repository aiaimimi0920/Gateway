//! The normal hydrated reader upgrades a known legacy shape without mutating storage or authority.
use super::super::GatewayProviderCredentialRow;
use super::provider_credential_view_from_row;
use serde_json::{json, Value};
use sqlx::types::Json;
use time::OffsetDateTime;

fn legacy() -> Value {
    json!({"apiKey":"synthetic-access", "refreshToken":"synthetic-refresh",
        "baseUrl":"https://phoenix-gw.alibaba.com", "providerSurfaceKey":"accio",
        "serviceProviderKey":"accio", "credentialMaterialKind":"session_auth",
        "headers":{"utdid":"synthetic-device","x-utdid":"synthetic-device","Cookie":"cna=synthetic-cna"},
        "extraBody":{"token":"synthetic-access","accessToken":"synthetic-access","utdid":"synthetic-device","empid":"retained-account","version":"retained-version"},
        "defaultModel":"retained-model","unknownMetadata":{"retain":true},
        "rawSource":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh",
            "utdid":"synthetic-device","sgcookie":"synthetic opaque sg",
            "refreshClientId":"explicit-client","expiresAt":1767225600123u64}})
}
fn row(payload: Value) -> GatewayProviderCredentialRow {
    GatewayProviderCredentialRow {
        id: "stable-credential".into(),
        provider_account_id: "stable-account".into(),
        label: "retained-label".into(),
        status: "disabled".into(),
        payload_inline: Some(Json(payload)),
        payload_object_key: None,
        storage_mode: "inline".into(),
        source_kind: "folder_sync_import".into(),
        source_path: Some("accio/legacy.json".into()),
        source_hash: Some("unchanged-source-hash".into()),
        sync_mode: "folder_sync".into(),
        sync_state: "exported".into(),
        sync_error: Some("retained-sync-diagnostic".into()),
        cooldown_until: Some(OffsetDateTime::UNIX_EPOCH),
        last_error: Some("retained-health-diagnostic".into()),
        failure_count: 7,
        last_health_check_at: Some(OffsetDateTime::UNIX_EPOCH),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
        archived_at: None,
    }
}
async fn read(payload: Value) -> Value {
    provider_credential_view_from_row(row(payload))
        .await
        .unwrap()
        .payload
}
fn no_body_copies(payload: &Value) {
    for key in ["token", "accessToken", "utdid"] {
        assert!(payload["extraBody"].get(key).is_none(), "legacy copy {key}");
    }
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_projects_complete_material_without_reimport_or_metadata_changes() {
    let source = legacy();
    let view = provider_credential_view_from_row(row(source.clone()))
        .await
        .unwrap();
    let payload = &view.payload;
    assert_eq!(payload["accioSgcookie"], "synthetic opaque sg");
    assert_eq!(payload["refreshClientId"], "explicit-client");
    assert_eq!(
        payload["refreshEndpoint"],
        crate::protocol::accio_oauth::REFRESH_ENDPOINT
    );
    assert_eq!(payload["expiresAt"], "2026-01-01T00:00:00.123Z");
    no_body_copies(payload);
    assert_eq!(payload["rawSource"], source["rawSource"]);
    for key in [
        "apiKey",
        "refreshToken",
        "headers",
        "defaultModel",
        "unknownMetadata",
    ] {
        assert_eq!(payload[key], source[key]);
    }
    assert_eq!(payload["extraBody"]["empid"], "retained-account");
    assert_eq!(
        (
            view.id.as_str(),
            view.provider_account_id.as_str(),
            view.status.as_str()
        ),
        ("stable-credential", "stable-account", "disabled")
    );
    assert_eq!(view.label, "retained-label");
    assert_eq!(view.source_hash.as_deref(), Some("unchanged-source-hash"));
    assert_eq!(view.sync_error.as_deref(), Some("retained-sync-diagnostic"));
    assert_eq!(view.failure_count, 7);
    assert!(view.cooldown_until.is_some() && view.last_health_check_at.is_some());
    assert_eq!(
        read(source.clone()).await,
        *payload,
        "repeat read is deterministic"
    );
    assert_eq!(
        read(payload.clone()).await,
        *payload,
        "projected form is idempotent"
    );
    assert!(
        source.get("accioSgcookie").is_none(),
        "input storage bytes stay untouched"
    );
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_does_not_resurrect_material_after_access_or_refresh_rotation() {
    for (key, value) in [
        ("apiKey", json!("synthetic-rotated")),
        ("refreshToken", json!("synthetic-rotated")),
        ("apiKey", Value::Null),
        ("apiKey", json!("")),
        ("refreshToken", Value::Null),
        ("refreshToken", json!("")),
    ] {
        let mut payload = legacy();
        payload[key] = value;
        let projected = read(payload.clone()).await;
        assert_eq!(projected[key], payload[key]);
        for field in [
            "accioSgcookie",
            "refreshClientId",
            "refreshEndpoint",
            "expiresAt",
        ] {
            assert!(
                projected.get(field).is_none(),
                "old grant resurrected {field}"
            );
        }
        no_body_copies(&projected);
        assert_eq!(projected["rawSource"], payload["rawSource"]);
    }
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_cleared_access_cannot_fall_back_to_the_archived_body_token() {
    let mut payload = legacy();
    payload["apiKey"] = json!("");
    let projected = read(payload).await;
    no_body_copies(&projected);
    let mut target = projected;
    target["adapter"] = json!("accio_compatible");
    let target = serde_json::from_value(target).unwrap();
    let request = crate::protocol::openai::normalize_chat_completions(json!({
        "model":"exact-model","messages":[{"role":"user","content":"synthetic probe"}]
    }))
    .unwrap();
    let error = crate::upstream::accio::build_request_plan(&target, &request, "exact-model", false)
        .err()
        .expect("cleared canonical access must not use archived body copies");
    assert_eq!(error.code.as_deref(), Some("accio_missing_body_token"));
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_preserves_explicit_clear_and_new_material_aliases() {
    for fields in [
        json!({"accioSgcookie":null,"refreshClientId":null,"refreshEndpoint":null,"expiresAt":null}),
        json!({"accio_sgcookie":null,"refresh_client_id":null,"refresh_endpoint":null,"expires_at":null}),
        json!({"accioSgcookie":"synthetic-new-sg","refreshClientId":"new-client","refreshEndpoint":"explicit-endpoint","expiresAt":"2099-01-01T00:00:00Z"}),
    ] {
        let mut payload = legacy();
        payload
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        let projected = read(payload).await;
        for (key, value) in fields.as_object().unwrap() {
            assert_eq!(&projected[key], value, "explicit {key}");
        }
        if fields.get("accio_sgcookie").is_some() {
            for key in [
                "accioSgcookie",
                "refreshClientId",
                "refreshEndpoint",
                "expiresAt",
            ] {
                assert!(projected.get(key).is_none(), "conflicting alias {key}");
            }
        }
        no_body_copies(&projected);
    }
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_partial_clear_or_changed_context_cannot_replenish_other_fields() {
    for change in [
        json!({"accioSgcookie":null}),
        json!({"accio_sgcookie":"synthetic-new-sg"}),
        json!({"refreshClientId":null}),
        json!({"refresh_endpoint":null}),
        json!({"expiresAt":"2099-01-01T00:00:00Z"}),
    ] {
        let mut payload = legacy();
        payload
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        let projected = read(payload.clone()).await;
        for key in [
            "accioSgcookie",
            "refreshClientId",
            "refreshEndpoint",
            "expiresAt",
        ] {
            assert_eq!(
                projected.get(key),
                payload.get(key),
                "archive replenished {key}"
            );
        }
        no_body_copies(&projected);
    }
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_normalizes_only_the_unchanged_original_absolute_expiry() {
    for (input, expected) in [
        (
            "2026-01-01T08:00:00.123456789+08:00",
            "2026-01-01T00:00:00.123456789Z",
        ),
        ("1767225600.123456789", "2026-01-01T00:00:00.123456789Z"),
        ("2000-01-01T00:00:00Z", "2000-01-01T00:00:00Z"),
    ] {
        let mut payload = legacy();
        payload["rawSource"]["expiresAt"] = json!(input);
        payload["expiresAt"] = json!(input);
        assert_eq!(read(payload).await["expiresAt"], expected);
    }
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_requires_folder_provenance_and_exact_known_legacy_shape() {
    for changed in [
        json!({"providerSurfaceKey":"other-product"}),
        json!({"baseUrl":"https://foreign.invalid"}),
        json!({"extraBody":{"token":"different-body-token"}}),
        json!({"headers":{"utdid":"different-device"}}),
        json!({"rawSource":null}),
    ] {
        let mut payload = legacy();
        payload
            .as_object_mut()
            .unwrap()
            .extend(changed.as_object().unwrap().clone());
        assert_eq!(read(payload.clone()).await, payload);
    }
    for (source, mode) in [("manual", "folder_sync"), ("folder_sync_import", "manual")] {
        let payload = legacy();
        let mut input = row(payload.clone());
        input.source_kind = source.into();
        input.sync_mode = mode.into();
        assert_eq!(
            provider_credential_view_from_row(input)
                .await
                .unwrap()
                .payload,
            payload
        );
    }
    let mut unknown = legacy();
    unknown["rawSource"]["access_token"] = json!(false);
    unknown["extraBody"] = json!({"custom":"not-the-old-normalizer"});
    assert_eq!(read(unknown.clone()).await, unknown);
}

#[cfg(feature = "line-accio-web-reverse-api")]
#[tokio::test]
async fn accio_legacy_db_reader_rejects_malformed_original_material_without_secret_diagnostics() {
    let mut payload = legacy();
    payload["rawSource"]["sgcookie"] = json!("synthetic-private\r\nsg");
    let error = provider_credential_view_from_row(row(payload))
        .await
        .unwrap_err();
    assert_eq!(error.code.as_deref(), Some("accio_import_invalid_material"));
    assert!(!error.to_string().contains("synthetic-private"));
}

#[cfg(not(feature = "line-accio-web-reverse-api"))]
#[tokio::test]
async fn compiled_out_accio_legacy_db_reader_preserves_existing_payload() {
    let payload = legacy();
    assert_eq!(read(payload.clone()).await, payload);
}
