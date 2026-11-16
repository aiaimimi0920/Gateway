//! Guarded real-DB fixtures use the existing schema, loopback admission and cleanup owner.
use super::fixture::Fixture;
use neuro_gateway::db;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub const PATH: &str = "accio/legacy.json";
pub fn legacy() -> Value {
    json!({"apiKey":"synthetic-access","refreshToken":"synthetic-refresh",
        "baseUrl":"https://phoenix-gw.alibaba.com","providerSurfaceKey":"accio",
        "serviceProviderKey":"accio","credentialMaterialKind":"session_auth",
        "headers":{"utdid":"synthetic-device","x-utdid":"synthetic-device"},
        "extraBody":{"token":"synthetic-access","accessToken":"synthetic-access","utdid":"synthetic-device","empid":"retained-account"},
        "rawSource":{"accessToken":"synthetic-access","refreshToken":"synthetic-refresh",
            "utdid":"synthetic-device","sgcookie":"synthetic opaque sg",
            "refreshClientId":"explicit-client","expiresAt":1767225600123u64}})
}

pub async fn setup() -> (Fixture, String) {
    let mut fixture = Fixture::new().await;
    Arc::get_mut(&mut fixture.state)
        .unwrap()
        .config
        .provider_credential_folder_sync_import_enabled = true;
    sqlx::query("update gateway_provider_accounts set label='Accio fixture',service_provider_key='accio',service_provider_label='Accio',adapter='accio_compatible',protocol_family='openai_responses',protocol_profile='accio',payload_inline=$2 where id=$1")
        .bind(&fixture.account).bind(sqlx::types::Json(json!({"baseUrl":"https://phoenix-gw.alibaba.com"})))
        .execute(&fixture.pg).await.unwrap();
    let payload = legacy();
    let bytes = serde_json::to_vec(&payload["rawSource"]).unwrap();
    let path = fixture.root.join(PATH);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, &bytes).unwrap();
    let id = fixture.credential(PATH, "folder_sync").await;
    sqlx::query("update gateway_provider_credentials set payload_inline=$2,source_kind='folder_sync_import',source_hash=$3,sync_state='imported',sync_error='retained diagnostic',last_error='retained health',failure_count=7 where id=$1")
        .bind(&id).bind(sqlx::types::Json(payload)).bind(format!("{:x}",Sha256::digest(bytes)))
        .execute(&fixture.pg).await.unwrap();
    (fixture, id)
}

pub async fn row(fixture: &Fixture, id: &str) -> Value {
    sqlx::query_scalar::<_,sqlx::types::Json<Value>>("select to_jsonb(c) || jsonb_build_object('snapshot',xmin::text) from gateway_provider_credentials c where id=$1")
        .bind(id).fetch_one(&fixture.pg).await.unwrap().0
}
pub async fn read(fixture: &Fixture, id: &str) -> db::GatewayProviderCredentialView {
    db::get_provider_credential(&fixture.pg, id)
        .await
        .unwrap()
        .unwrap()
}
pub fn projected(payload: &Value) {
    assert_eq!(payload["accioSgcookie"], "synthetic opaque sg");
    assert_eq!(payload["refreshClientId"], "explicit-client");
    assert_eq!(payload["expiresAt"], "2026-01-01T00:00:00.123Z");
    for key in ["token", "accessToken", "utdid"] {
        assert!(payload["extraBody"].get(key).is_none());
    }
}
pub fn input(account: &str, payload: Value) -> db::UpsertProviderCredentialInput {
    db::UpsertProviderCredentialInput {
        provider_account_id: account.into(),
        label: "retained label".into(),
        status: None,
        payload,
        source_kind: None,
        source_path: None,
        source_hash: None,
        sync_mode: None,
        sync_state: None,
        sync_error: None,
    }
}
