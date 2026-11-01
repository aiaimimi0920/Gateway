//! Managed music sessions: read private material and persist Supabase rotation before calling FlowMusic.
use super::producer_session_helpers::{
    producer_bearer_needs_refresh, refresh_producer_supabase_access_token,
};
use crate::{
    error::GatewayError, object_storage::gateway_object_storage,
    routing::candidate::ProviderAccountPayload,
};
use rquest::{
    header::{HeaderMap, HeaderValue, AUTHORIZATION},
    Client,
};
use serde::{Deserialize, Serialize};
use std::{
    hash::{Hash, Hasher},
    sync::LazyLock,
};
use tokio::sync::Mutex;

// Bounded striped locks serialize account refreshes without an ever-growing per-account map.
static REFRESH_LOCKS: LazyLock<[Mutex<()>; 32]> =
    LazyLock::new(|| std::array::from_fn(|_| Mutex::new(())));
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Material {
    schema_version: u8,
    provider: String,
    access_token: String,
    refresh_token: Option<String>,
}

fn failure(code: &str) -> GatewayError {
    GatewayError::service_unavailable("FlowMusic 登录材料不可用或需要重新登录；私人详情已省略。")
        .with_code(code)
        .with_provider("producer_compatible")
}

fn parse(bytes: &[u8]) -> Result<Material, GatewayError> {
    let material: Material =
        serde_json::from_slice(bytes).map_err(|_| failure("flowmusic_session_invalid"))?;
    let valid = |s: &str, max| {
        !s.is_empty()
            && s.len() <= max
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    };
    if bytes.len() > 24 * 1024
        || material.schema_version != 1
        || material.provider != "flowmusic-web"
        || !valid(&material.access_token, 16384)
        || material
            .refresh_token
            .as_ref()
            .is_some_and(|s| !valid(s, 4096))
    {
        return Err(failure("flowmusic_session_invalid"));
    }
    Ok(material)
}

pub(crate) async fn headers(
    http: &Client,
    payload: &ProviderAccountPayload,
    headers: &HeaderMap,
) -> Result<HeaderMap, GatewayError> {
    let Some(key) = payload
        .runtime_state_object_key
        .as_deref()
        .filter(|key| key.starts_with("credential-runtime/flowmusic/"))
    else {
        return Ok(
            super::producer_session_helpers::maybe_refresh_producer_headers(http, headers).await,
        );
    };
    // Never send a managed FlowMusic session to a user-edited or redirected provider origin.
    if payload.base_url.trim_end_matches('/') != "https://www.flowmusic.app" {
        return Err(failure("flowmusic_session_origin_mismatch"));
    }
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hash);
    let _guard = REFRESH_LOCKS[hash.finish() as usize % REFRESH_LOCKS.len()]
        .lock()
        .await;
    let storage = gateway_object_storage().map_err(|_| failure("flowmusic_session_unavailable"))?;
    let bytes = storage
        .read_bytes_with_limit(key, 24 * 1024)
        .await
        .map_err(|_| failure("flowmusic_session_unavailable"))?;
    let mut material = parse(&bytes)?;
    if producer_bearer_needs_refresh(Some(&material.access_token)) {
        let refresh = material
            .refresh_token
            .as_deref()
            .ok_or_else(|| failure("flowmusic_login_required"))?;
        let renewed = refresh_producer_supabase_access_token(http, refresh)
            .await
            .ok_or_else(|| failure("flowmusic_login_required"))?;
        material.access_token = renewed.access_token;
        if let Some(refresh) = renewed.refresh_token {
            material.refresh_token = Some(refresh);
        }
        let bytes =
            serde_json::to_vec(&material).map_err(|_| failure("flowmusic_session_invalid"))?;
        parse(&bytes)?;
        storage
            .put_bytes(key, bytes, "application/json")
            .await
            .map_err(|_| failure("flowmusic_session_rotation_failed"))?;
    }
    let mut effective = headers.clone();
    effective.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", material.access_token))
            .map_err(|_| failure("flowmusic_session_invalid"))?,
    );
    Ok(effective)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_material_is_bounded_provider_specific_and_not_a_browser_state() {
        assert!(parse(br#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"fixture.access.token"}"#).is_ok());
        for source in [br#"{"schemaVersion":1,"provider":"other","accessToken":"fixture"}"#.as_slice(),
            br#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"a\r\nb"}"#,
            br#"{"schemaVersion":1,"provider":"flowmusic-web","accessToken":"fixture","cookies":[]}"#] {
            assert!(parse(source).is_err());
        }
    }
}
