use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use deadpool_redis::Pool as RedisPool;
use serde_json::Value;
use sqlx::PgPool;

use crate::db;
use crate::error::GatewayError;
use crate::protocol::gemini_canvas;
use crate::redis::credential_cache;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::gemini::canvas_program_web_reverse as gemini_canvas_program_web_reverse_modular;

fn read_json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string)
}

pub(crate) fn gemini_canvas_explicit_cookie_header_from_payload(
    payload: &ProviderAccountPayload,
    storage_state: Option<&Value>,
) -> Option<String> {
    payload
        .extra_body
        .as_ref()
        .and_then(|extra| {
            extra
                .get("canvasProgramInvokeContract")
                .and_then(Value::as_object)
                .and_then(|contract| contract.get("cookieHeader"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("cookieHeader"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| {
                    extra
                        .get("canvasProgramInvokeContract")
                        .and_then(Value::as_object)
                        .and_then(|contract| contract.get("cookie_header"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .extra_body
                .as_ref()
                .and_then(|extra| extra.get("cookie_header"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            payload
                .headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("cookie"))
                .map(|(_, value)| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| storage_state.and_then(|value| read_json_string(value, "cookieHeader")))
}

pub(crate) fn gemini_canvas_pure_http_session_from_payload_or_storage(
    payload: &ProviderAccountPayload,
    storage_state: &Value,
    target_url: &str,
    base_url: &str,
    auth_user: &str,
) -> Result<gemini_canvas::GeminiCanvasPureHttpSession, GatewayError> {
    if let Some(cookie_header) =
        gemini_canvas_explicit_cookie_header_from_payload(payload, Some(storage_state))
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    {
        gemini_canvas::pure_http_session_from_cookie_header(cookie_header, auth_user)
    } else {
        gemini_canvas::storage_state_to_pure_http_session(
            storage_state,
            target_url,
            base_url,
            auth_user,
        )
    }
}

pub(crate) fn gemini_canvas_http_origin(payload: &ProviderAccountPayload) -> String {
    origin_from_url(payload.base_url.trim_end_matches('/'))
        .unwrap_or_else(|| "https://gemini.google.com".to_string())
}

pub(crate) fn gemini_canvas_page_base_url(payload: &ProviderAccountPayload) -> String {
    if payload.adapter == "gemini_canvas_program_web_reverse_compatible" {
        if let Some(page_url) =
            gemini_canvas_program_web_reverse_modular::gemini_canvas_program_payload_page_url(
                payload,
                "https://gemini.google.com",
            )
        {
            if let Some(origin) = origin_from_url(&page_url) {
                return origin;
            }
        }
    }
    let base_origin = gemini_canvas_http_origin(payload);
    if base_origin.contains("gemini.google.com") {
        base_origin
    } else {
        "https://gemini.google.com".to_string()
    }
}

pub(crate) fn origin_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (scheme, rest) = if let Some(rest) = trimmed.strip_prefix("https://") {
        ("https", rest)
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        ("http", rest)
    } else {
        ("https", trimmed)
    };
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .split('@')
        .last()
        .unwrap_or_default()
        .trim();
    if host.is_empty() {
        None
    } else {
        Some(format!("{scheme}://{host}"))
    }
}

pub(crate) fn current_unix_timestamp_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub(crate) fn gemini_canvas_signaler_zx_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(crate) fn gemini_canvas_runtime_api_missing_google_api_key_error(
    page_harvest_probes: &[String],
) -> GatewayError {
    let mut error = GatewayError::unauthorized(
        "Gemini Canvas runtime API media lane requires a Google API key harvested from the Canvas session state.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("gemini_canvas_runtime_api_missing_google_api_key");
    if !page_harvest_probes.is_empty() {
        error.message = format!(
            "{}; page_harvest={}",
            error.message,
            page_harvest_probes.join(" | ")
        );
    }
    error
}

pub(crate) fn build_gemini_canvas_runtime_persistence_payload(
    existing_payload: &Value,
    payload: &ProviderAccountPayload,
    extra_body_patch: &HashMap<String, Value>,
    provider_account_base_url: Option<&str>,
) -> Value {
    let mut updated_payload = existing_payload.clone();
    if let Some(map) = updated_payload.as_object_mut() {
        let extra_body = map
            .entry("extraBody".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Some(extra_body_map) = extra_body.as_object_mut() {
            for (key, value) in extra_body_patch {
                extra_body_map.insert(key.clone(), value.clone());
            }
        }
        if let Some(runtime_state_object_key) = payload.runtime_state_object_key.as_ref() {
            map.insert(
                "runtimeStateObjectKey".to_string(),
                Value::String(runtime_state_object_key.clone()),
            );
        }
        if map
            .get("adapter")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            map.insert(
                "adapter".to_string(),
                Value::String(payload.adapter.clone()),
            );
        }
        if map
            .get("baseUrl")
            .or_else(|| map.get("base_url"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            let fallback_base_url = if payload.base_url.trim().is_empty() {
                provider_account_base_url.map(str::to_string)
            } else {
                Some(payload.base_url.clone())
            };
            if let Some(base_url) = fallback_base_url {
                map.insert("baseUrl".to_string(), Value::String(base_url));
            }
        }
    }
    updated_payload
}

pub(crate) async fn persist_gemini_canvas_program_runtime_material(
    redis_pool: Option<&RedisPool>,
    pg_pool: Option<&PgPool>,
    payload: &ProviderAccountPayload,
    extra_body_patch: Option<HashMap<String, Value>>,
) {
    let Some(credential_id) = payload.credential_id.as_deref() else {
        return;
    };
    let Some(extra_body_patch) = extra_body_patch else {
        return;
    };

    if let Some(redis_pool) = redis_pool {
        let _ = credential_cache::write_back_runtime_material(
            redis_pool,
            credential_id,
            None,
            None,
            Some(&extra_body_patch),
            payload.session_auth.as_ref(),
            payload.keepalive.as_ref(),
            payload.expires_at.as_deref(),
            payload.runtime_state_object_key.as_deref(),
        )
        .await;
    }

    if let Some(pg_pool) = pg_pool {
        match db::get_provider_credential(pg_pool, credential_id).await {
            Ok(Some(existing)) => {
                let provider_account_base_url =
                    db::get_provider_account(pg_pool, existing.provider_account_id.as_str())
                        .await
                        .ok()
                        .flatten()
                        .and_then(|provider_account| {
                            provider_account
                                .payload
                                .get("baseUrl")
                                .or_else(|| provider_account.payload.get("base_url"))
                                .and_then(Value::as_str)
                                .map(str::trim)
                                .filter(|value| !value.is_empty())
                                .map(str::to_string)
                        });
                let updated_payload = build_gemini_canvas_runtime_persistence_payload(
                    &existing.payload,
                    payload,
                    &extra_body_patch,
                    provider_account_base_url.as_deref(),
                );

                let update_input = db::UpsertProviderCredentialInput {
                    provider_account_id: existing.provider_account_id.clone(),
                    label: existing.label.clone(),
                    status: Some(existing.status.clone()),
                    payload: updated_payload,
                    source_kind: Some(existing.source_kind.clone()),
                    source_path: existing.source_path.clone(),
                    source_hash: existing.source_hash.clone(),
                    sync_mode: Some(existing.sync_mode.clone()),
                    sync_state: Some(existing.sync_state.clone()),
                    sync_error: existing.sync_error.clone(),
                };
                let _ = db::update_provider_credential(pg_pool, credential_id, update_input).await;
            }
            Ok(None) => {}
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        serde_json::from_value(json!({
            "adapter": adapter,
            "baseUrl": base_url,
            "apiKey": "sk-test"
        }))
        .expect("payload")
    }

    #[test]
    fn runtime_api_missing_google_api_key_error_matches_contract() {
        let probes = vec![
            "page1 len=10 contains_aiza=false api_key_count=0".to_string(),
            "page2 fetch_error=timeout".to_string(),
        ];
        let error = gemini_canvas_runtime_api_missing_google_api_key_error(&probes);
        assert_eq!(error.http_status, Some(401));
        assert_eq!(
            error.provider_name.as_deref(),
            Some("gemini_canvas_compatible")
        );
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_canvas_runtime_api_missing_google_api_key")
        );
        assert_eq!(
            error.message.as_str(),
            "Gemini Canvas runtime API media lane requires a Google API key harvested from the Canvas session state.; page_harvest=page1 len=10 contains_aiza=false api_key_count=0 | page2 fetch_error=timeout"
        );
    }

    #[test]
    fn origin_from_url_normalizes_scheme_and_strips_path() {
        assert_eq!(
            origin_from_url(" gemini.google.com/app/4abc4e7577b6149f?hl=zh-CN "),
            Some("https://gemini.google.com".to_string())
        );
        assert_eq!(
            origin_from_url("http://example.com/nested/path"),
            Some("http://example.com".to_string())
        );
        assert_eq!(origin_from_url("   "), None);
    }

    #[test]
    fn gemini_canvas_explicit_cookie_header_from_payload_prefers_contract_then_headers_then_storage(
    ) {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.headers.insert(
            "cookie".to_string(),
            "__Secure-1PAPISID=from-header".to_string(),
        );
        payload.extra_body = Some(HashMap::from([(
            "canvasProgramInvokeContract".to_string(),
            json!({
                "cookieHeader": "__Secure-1PAPISID=from-contract; SID=contract"
            }),
        )]));
        let storage_state = json!({
            "cookieHeader": "__Secure-1PAPISID=from-storage"
        });

        assert_eq!(
            gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
                .as_deref(),
            Some("__Secure-1PAPISID=from-contract; SID=contract")
        );

        payload.extra_body = None;
        assert_eq!(
            gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
                .as_deref(),
            Some("__Secure-1PAPISID=from-header")
        );

        payload.headers.clear();
        assert_eq!(
            gemini_canvas_explicit_cookie_header_from_payload(&payload, Some(&storage_state))
                .as_deref(),
            Some("__Secure-1PAPISID=from-storage")
        );
    }

    #[test]
    fn gemini_canvas_pure_http_session_from_payload_or_storage_prefers_explicit_cookie_header() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );
        payload.extra_body = Some(HashMap::from([(
            "cookieHeader".to_string(),
            json!("__Secure-1PAPISID=session-token; SID=abc"),
        )]));

        let session = gemini_canvas_pure_http_session_from_payload_or_storage(
            &payload,
            &json!({}),
            "https://gemini.google.com/app/4abc4e7577b6149f",
            "https://gemini.google.com",
            "0",
        )
        .expect("explicit cookie header session");

        assert_eq!(
            session,
            gemini_canvas::GeminiCanvasPureHttpSession {
                cookie_header: "__Secure-1PAPISID=session-token; SID=abc".to_string(),
                sapisid: "session-token".to_string(),
                auth_user: "0".to_string(),
            }
        );
    }

    #[test]
    fn current_unix_timestamp_i64_stays_within_wall_clock_bounds() {
        let before = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let observed = current_unix_timestamp_i64();
        let after = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        assert!(observed >= before);
        assert!(observed <= after);
    }

    #[test]
    fn gemini_canvas_signaler_zx_token_is_32_hex_chars() {
        let token = gemini_canvas_signaler_zx_token();

        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn gemini_canvas_runtime_persistence_payload_merges_material_patch() {
        let mut payload = make_payload("gemini_canvas_program_web_reverse_compatible", "   ");
        payload.runtime_state_object_key =
            Some("credential-runtime/gemini-canvas/program/storage-state.json".to_string());
        let existing_payload = json!({
            "extraBody": {
                "existing": "keep",
                "shareId": "old-share"
            },
            "adapter": "   ",
            "base_url": "   "
        });
        let patch = HashMap::from([
            ("shareId".to_string(), json!("new-share")),
            ("appPath".to_string(), json!("/app/4abc4e7577b6149f")),
        ]);

        let updated = build_gemini_canvas_runtime_persistence_payload(
            &existing_payload,
            &payload,
            &patch,
            Some("https://gemini.google.com"),
        );
        let updated_map = updated.as_object().expect("updated payload object");
        let extra_body = updated_map
            .get("extraBody")
            .and_then(Value::as_object)
            .expect("merged extraBody object");

        assert_eq!(extra_body.get("existing"), Some(&json!("keep")));
        assert_eq!(extra_body.get("shareId"), Some(&json!("new-share")));
        assert_eq!(
            extra_body.get("appPath"),
            Some(&json!("/app/4abc4e7577b6149f"))
        );
        assert_eq!(
            updated_map.get("runtimeStateObjectKey"),
            Some(&json!(
                "credential-runtime/gemini-canvas/program/storage-state.json"
            ))
        );
        assert_eq!(
            updated_map.get("adapter"),
            Some(&json!("gemini_canvas_program_web_reverse_compatible"))
        );
        assert_eq!(
            updated_map.get("baseUrl"),
            Some(&json!("https://gemini.google.com"))
        );
    }

    #[test]
    fn gemini_canvas_runtime_persistence_payload_preserves_existing_identity_fields() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://candidate.example",
        );
        payload.runtime_state_object_key = Some("runtime-state/new.json".to_string());
        let existing_payload = json!({
            "extraBody": {},
            "adapter": "existing_adapter",
            "baseUrl": "https://existing.example"
        });
        let patch = HashMap::from([("shareId".to_string(), json!("new-share"))]);

        let updated = build_gemini_canvas_runtime_persistence_payload(
            &existing_payload,
            &payload,
            &patch,
            Some("https://provider-account.example"),
        );
        let updated_map = updated.as_object().expect("updated payload object");
        let extra_body = updated_map
            .get("extraBody")
            .and_then(Value::as_object)
            .expect("merged extraBody object");

        assert_eq!(extra_body.get("shareId"), Some(&json!("new-share")));
        assert_eq!(updated_map.get("adapter"), Some(&json!("existing_adapter")));
        assert_eq!(
            updated_map.get("baseUrl"),
            Some(&json!("https://existing.example"))
        );
        assert_eq!(
            updated_map.get("runtimeStateObjectKey"),
            Some(&json!("runtime-state/new.json"))
        );
    }

    #[tokio::test]
    async fn persist_gemini_canvas_program_runtime_material_noops_without_required_inputs() {
        let mut payload = make_payload(
            "gemini_canvas_program_web_reverse_compatible",
            "https://gemini.google.com",
        );

        persist_gemini_canvas_program_runtime_material(
            None,
            None,
            &payload,
            Some(HashMap::from([("shareId".to_string(), json!("new-share"))])),
        )
        .await;

        payload.credential_id = Some("credential-123".to_string());
        persist_gemini_canvas_program_runtime_material(None, None, &payload, None).await;
    }
}
