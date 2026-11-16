//! Synthetic authority snapshots do not confer OAuth client permission or account identity.
use super::snapshot::from_rows;
use serde_json::{json, Value};

pub(super) fn rows() -> (Value, Value, Value, Value) {
    (
        json!({"id":"account-a","status":"active","adapter":"accio_compatible",
            "refresh_authority_id":"c53c6a8e-420a-4be5-9870-fc5b8b738fc6","refresh_authority_version":3}),
        json!({"id":"credential-a","provider_account_id":"account-a","status":"active",
            "archived_at":null,"storage_mode":"inline","source_kind":"manual","sync_mode":"manual",
            "refresh_authority_id":"86c750d8-7a6c-41f0-b0ca-4ad9c8ff9bfb","refresh_authority_version":5}),
        json!({"baseUrl":"https://phoenix-gw.alibaba.com","supportedModels":["explicit-model"]}),
        json!({"apiKey":"synthetic-access","refreshToken":"synthetic-refresh",
            "refreshClientId":"explicit-client","refreshEndpoint":"https://phoenix-gw.alibaba.com/api/auth/refresh_token",
            "accioSgcookie":"synthetic opaque sg","expiresAt":"2026-11-09T00:00:00.123456789Z",
            "headers":{"utdid":"synthetic-device"},"extraBody":{"empid":"retained-context"}}),
    )
}

#[test]
fn accio_db_refresh_snapshot_owns_exact_credential_material_and_absolute_expiry() {
    let (a, c, ap, cp) = rows();
    let result = from_rows(&a, &c, ap, cp.clone());
    assert!(
        result.is_ok(),
        "a complete database-owned grant must be admitted"
    );
    let snapshot = result.ok().unwrap();
    assert_eq!(snapshot.request.provider, "account-a");
    assert_eq!(snapshot.request.credential, "credential-a");
    assert_eq!(snapshot.payload, cp);
    assert_eq!(snapshot.storage_mode, "inline");
    assert_eq!(snapshot.request.revision.len(), 64);
}

#[test]
fn accio_db_refresh_snapshot_generation_and_incarnation_fence_restored_material() {
    let (a, mut c, ap, cp) = rows();
    let first = from_rows(&a, &c, ap.clone(), cp.clone())
        .ok()
        .expect("original authority");
    c["refresh_authority_version"] = json!(6);
    let next = from_rows(&a, &c, ap.clone(), cp.clone())
        .ok()
        .expect("updated authority");
    assert_eq!(first.request.family_id(), next.request.family_id());
    assert_ne!(first.request.revision, next.request.revision);
    c["refresh_authority_id"] = json!("93205b32-6c68-4729-a41d-9f95b7605e2b");
    let recreated = from_rows(&a, &c, ap, cp).ok().expect("recreated authority");
    assert_ne!(next.request.revision, recreated.request.revision);
}

#[test]
fn accio_db_refresh_snapshot_rejects_inherited_grant_and_disabled_authority() {
    let (mut a, c, mut ap, mut cp) = rows();
    ap["refreshToken"] = cp["refreshToken"].take();
    assert!(from_rows(&a, &c, ap.clone(), cp.clone()).is_err());
    let (_, _, ap, cp) = rows();
    a["status"] = json!("disabled");
    assert!(from_rows(&a, &c, ap, cp).is_err());
}

#[test]
fn accio_db_refresh_snapshot_rejects_other_audience_or_ambiguous_aliases() {
    let (a, c, ap, mut cp) = rows();
    cp["baseUrl"] = json!("https://example.invalid");
    assert!(from_rows(&a, &c, ap.clone(), cp).is_err());
    let (_, _, _, mut cp) = rows();
    cp["api_key"] = json!("different-access");
    assert!(from_rows(&a, &c, ap, cp).is_err());
}

#[test]
fn accio_db_refresh_snapshot_rejects_partial_clear_and_foreign_context_without_restoration() {
    let (a, c, ap, mut cp) = rows();
    cp["refreshToken"] = Value::Null;
    cp["extraBody"]["refreshToken"] = json!("synthetic-refresh");
    assert!(from_rows(&a, &c, ap.clone(), cp).is_err());
    let (_, _, _, mut cp) = rows();
    cp["grok_oauth"] = json!({});
    assert!(from_rows(&a, &c, ap.clone(), cp).is_err());
    let (_, _, _, mut cp) = rows();
    cp["refreshEndpoint"] = Value::Null;
    assert!(from_rows(&a, &c, ap, cp).is_err());
}

#[test]
fn accio_db_refresh_snapshot_uses_protocol_token_bounds_not_database_id_bounds() {
    let (a, c, ap, mut cp) = rows();
    cp["refreshToken"] = json!("r".repeat(1000));
    assert!(from_rows(&a, &c, ap, cp).is_ok());
}
