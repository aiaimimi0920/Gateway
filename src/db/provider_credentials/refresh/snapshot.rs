//! Bind one credential's material to its database incarnation and authorization generation.
use crate::{
    console::refresh_grants::{GrantMaterial, GrantRequest},
    error::GatewayError,
    protocol::accio_oauth::{import_access, ImportMaterial, GATEWAY},
    routing::candidate::deserialize_provider_payload,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub(crate) struct Snapshot {
    pub request: GrantRequest,
    pub payload: Value,
    pub storage_mode: String,
}

pub(super) fn from_rows(
    account: &Value,
    credential: &Value,
    account_payload: Value,
    credential_payload: Value,
) -> Result<Snapshot, GatewayError> {
    if !cfg!(feature = "line-accio-web-reverse-api") {
        return Err(invalid());
    }
    let provider = text(account, "id")?;
    let id = text(credential, "id")?;
    if account["status"] != "active"
        || account["adapter"] != "accio_compatible"
        || credential["status"] != "active"
        || !credential["archived_at"].is_null()
        || text(credential, "provider_account_id")? != provider
        || !matches!(credential["storage_mode"].as_str(), Some("inline" | "r2"))
    {
        return Err(invalid());
    }
    let ap = account_payload.as_object().ok_or_else(invalid)?;
    let cp = credential_payload.as_object().ok_or_else(invalid)?;
    let grant_names = [
        "apiKey",
        "api_key",
        "accessToken",
        "access_token",
        "refreshToken",
        "refresh_token",
        "accioSgcookie",
        "accio_sgcookie",
        "sgcookie",
        "refreshClientId",
        "refresh_client_id",
        "refreshEndpoint",
        "refresh_endpoint",
        "grok_oauth",
        "grokOauth",
        "kiro_oauth",
        "kiroOauth",
        "quick_oauth",
        "quickOauth",
        "vertex_oauth",
        "vertexOauth",
        "dim_oauth",
        "dimOauth",
        "nous_portal",
        "nousPortal",
    ];
    // Account defaults cannot provide any portion of a credential's grant.
    if std::iter::once(ap).chain(extra_maps(ap)).any(|map| {
        grant_names.iter().any(|key| {
            map.get(*key)
                .is_some_and(|value| !value.is_null() && value != "")
        })
    }) || std::iter::once(cp).chain(extra_maps(cp)).any(|map| {
        [
            "grok_oauth",
            "grokOauth",
            "kiro_oauth",
            "kiroOauth",
            "quick_oauth",
            "quickOauth",
            "vertex_oauth",
            "vertexOauth",
            "dim_oauth",
            "dimOauth",
            "nous_portal",
            "nousPortal",
        ]
        .iter()
        .any(|key| map.get(*key).is_some_and(|value| !value.is_null()))
    }) {
        return Err(invalid());
    }
    reject_clear_conflicts(cp)?;
    let own = deserialize_provider_payload(&credential_payload, Some("accio_compatible"))
        .map_err(|_| invalid())?;
    if import_access(cp).map_err(|_| invalid())?.as_deref() != Some(own.api_key.as_str()) {
        return Err(invalid());
    }
    let merged = super::super::payload::merge_provider_account_and_credential_payloads(
        &account_payload,
        &credential_payload,
    );
    let effective =
        deserialize_provider_payload(&merged, Some("accio_compatible")).map_err(|_| invalid())?;
    if effective.adapter != "accio_compatible"
        || url::Url::parse(&effective.base_url).ok() != url::Url::parse(GATEWAY).ok()
    {
        return Err(invalid());
    }
    let mut canonical = Map::new();
    ImportMaterial::parse_refresh(cp, &effective.base_url)
        .map_err(|_| invalid())?
        .apply(&mut canonical);
    let required = |key: &str| {
        canonical
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(invalid)
    };
    let material = GrantMaterial {
        endpoint: required("refreshEndpoint")?,
        client: required("refreshClientId")?,
        access: own.api_key,
        refresh: required("refreshToken")?,
        expires_at: Some(required("expiresAt")?),
        accio_sgcookie: canonical
            .get("accioSgcookie")
            .and_then(Value::as_str)
            .map(str::to_owned),
        context: None,
        kiro_oauth: None,
        quick_oauth: None,
        vertex_oauth: None,
        dim_oauth: None,
        nous_portal: None,
        lobsterai: None,
        luma_oauth: None,
    };
    let mut authority = json!([
        "gateway-db-refresh-authority-v1",
        provider,
        id,
        identity(account)?,
        identity(credential)?,
        account_payload,
        credential_payload
    ]);
    authority.sort_all_objects();
    let digest = hex::encode(Sha256::digest(authority.to_string().as_bytes()));
    let request = GrantRequest::from_database_snapshot(provider, id, &digest, material)
        .map_err(|_| invalid())?;
    Ok(Snapshot {
        request,
        payload: credential_payload,
        storage_mode: text(credential, "storage_mode")?.into(),
    })
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, GatewayError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 512)
        .ok_or_else(invalid)
}
fn identity(row: &Value) -> Result<Value, GatewayError> {
    let id = text(row, "refresh_authority_id")?;
    uuid::Uuid::parse_str(id).map_err(|_| invalid())?;
    let version = row["refresh_authority_version"]
        .as_i64()
        .filter(|v| *v >= 0)
        .ok_or_else(invalid)?;
    Ok(json!([id, version]))
}
fn extra_maps(raw: &Map<String, Value>) -> impl Iterator<Item = &Map<String, Value>> {
    ["extraBody", "extra_body"]
        .into_iter()
        .filter_map(move |key| raw.get(key).and_then(Value::as_object))
}
fn reject_clear_conflicts(raw: &Map<String, Value>) -> Result<(), GatewayError> {
    // The import parser may derive a known absent endpoint, but an explicit DB clear
    // must not silently re-enable automatic refresh from the retained client/token.
    if std::iter::once(raw).chain(extra_maps(raw)).any(|map| {
        ["refreshEndpoint", "refresh_endpoint"]
            .iter()
            .any(|key| map.get(*key).is_some_and(Value::is_null))
    }) {
        return Err(invalid());
    }
    for aliases in [
        &["refreshToken", "refresh_token"][..],
        &["accioSgcookie", "accio_sgcookie", "sgcookie"][..],
        &["expiresAt", "expires_at"][..],
        &["refreshEndpoint", "refresh_endpoint"][..],
        &[
            "refreshClientId",
            "refresh_client_id",
            "clientId",
            "client_id",
        ][..],
    ] {
        let values: Vec<_> = std::iter::once(raw)
            .chain(extra_maps(raw))
            .flat_map(|map| aliases.iter().filter_map(move |key| map.get(*key)))
            .collect();
        if values.iter().any(|v| v.is_null()) && values.iter().any(|v| !v.is_null()) {
            return Err(invalid());
        }
    }
    Ok(())
}

pub(super) fn invalid() -> GatewayError {
    GatewayError::conflict("Accio database refresh authority or material is invalid")
        .with_code("accio_db_refresh_invalid_authority")
}
