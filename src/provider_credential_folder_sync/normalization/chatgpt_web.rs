//! Normalize ChatGPT Web login and refresh metadata.

use super::super::payload_metadata::apply_folder_sync_metadata;
use super::chatgpt_auth::{chatgpt_web_import_string, decode_jwt_claims};
use super::source::canonicalize_folder_sync_raw_source;
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

pub(super) fn normalize_chatgpt_web_import_payload(
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let Some(raw_map) = raw_payload.as_object() else {
        return Err(GatewayError::bad_request(
            "chatgpt web reverse provider credential payload 必须是 JSON object",
        ));
    };

    let access_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["accessToken"],
            &["access_token"],
            &["apiKey"],
            &["api_key"],
            &["token"],
            &["chatgptLoginDetails", "clientBootstrap", "accessToken"],
        ],
    )
    .ok_or_else(|| {
        GatewayError::bad_request(
            "chatgpt web reverse provider credential payload 缺少 access token",
        )
    })?;
    let refresh_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["refreshToken"],
            &["refresh_token"],
            &["oauthRefreshToken"],
            &["openaiRefreshToken"],
            &["oauthTokens", "refreshToken"],
            &["oauthTokens", "refresh_token"],
            &["tokens", "refreshToken"],
            &["tokens", "refresh_token"],
            &["chatgptLoginDetails", "refreshToken"],
            &["chatgptLoginDetails", "refresh_token"],
            &["chatgptLoginDetails", "oauthTokens", "refreshToken"],
            &["chatgptLoginDetails", "oauthTokens", "refresh_token"],
            &["platformAuth", "refreshToken"],
            &["platformAuth", "refresh_token"],
        ],
    );
    let id_token = chatgpt_web_import_string(
        raw_map,
        &[
            &["idToken"],
            &["id_token"],
            &["oauthIdToken"],
            &["openaiIdToken"],
            &["oauthTokens", "idToken"],
            &["oauthTokens", "id_token"],
            &["tokens", "idToken"],
            &["tokens", "id_token"],
            &["chatgptLoginDetails", "idToken"],
            &["chatgptLoginDetails", "id_token"],
            &["chatgptLoginDetails", "oauthTokens", "idToken"],
            &["chatgptLoginDetails", "oauthTokens", "id_token"],
            &["platformAuth", "idToken"],
            &["platformAuth", "id_token"],
        ],
    );
    let access_token_claims = decode_jwt_claims(access_token);

    let base_url = provider_account
        .payload
        .get("base_url")
        .and_then(Value::as_str)
        .or_else(|| {
            provider_account
                .payload
                .get("baseUrl")
                .and_then(Value::as_str)
        })
        .filter(|value| !value.trim().is_empty())
        .map(crate::protocol::chatgpt::web_reverse::normalize_site_base_url)
        .unwrap_or_else(|| "https://chatgpt.com".to_string());
    let default_model = provider_account
        .payload
        .get("default_model")
        .cloned()
        .or_else(|| provider_account.payload.get("defaultModel").cloned())
        .unwrap_or_else(|| Value::String("gpt-5.4".to_string()));
    let expires_at = raw_map
        .get("expires")
        .or_else(|| raw_map.get("expiresAt"))
        .or_else(|| raw_map.get("expires_at"))
        .cloned()
        .or_else(|| {
            access_token_claims
                .as_ref()
                .and_then(|claims| claims.get("exp"))
                .and_then(Value::as_i64)
                .and_then(|exp| OffsetDateTime::from_unix_timestamp(exp).ok())
                .and_then(|timestamp| timestamp.format(&Rfc3339).ok())
                .map(Value::String)
        })
        .unwrap_or(Value::Null);

    let mut payload = serde_json::Map::new();
    payload.insert(
        "adapter".to_string(),
        Value::String("chatgpt_web_reverse_compatible".to_string()),
    );
    payload.insert(
        "apiKey".to_string(),
        Value::String(access_token.to_string()),
    );
    payload.insert("baseUrl".to_string(), Value::String(base_url.to_string()));
    payload.insert("defaultModel".to_string(), default_model);
    payload.insert(
        "sessionAuth".to_string(),
        serde_json::json!({
            "transport": "bearer",
            "headerName": "authorization"
        }),
    );
    if !expires_at.is_null() {
        payload.insert("expiresAt".to_string(), expires_at);
    }
    payload.insert(
        "headers".to_string(),
        serde_json::json!({
            "Origin": "https://chatgpt.com",
            "Referer": "https://chatgpt.com/",
            "Accept-Language": "zh-CN,zh;q=0.9,en;q=0.8,en-US;q=0.7",
            "User-Agent": crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_USER_AGENT,
        }),
    );
    let mut extra_body = serde_json::Map::new();
    extra_body.insert(
        "clientVersion".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_VERSION.to_string(),
        ),
    );
    extra_body.insert(
        "clientBuildNumber".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_CLIENT_BUILD_NUMBER
                .to_string(),
        ),
    );
    extra_body.insert(
        "timezone".to_string(),
        Value::String(
            crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string(),
        ),
    );
    if let Some(pow_sources) = raw_map
        .get("chatgptPowSources")
        .or_else(|| raw_map.get("powSources"))
        .and_then(Value::as_array)
        .filter(|items| !items.is_empty())
    {
        extra_body.insert(
            "chatgptPowSources".to_string(),
            Value::Array(pow_sources.clone()),
        );
    }
    if let Some(pow_data_build) = raw_map
        .get("chatgptPowDataBuild")
        .or_else(|| raw_map.get("powDataBuild"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        extra_body.insert(
            "chatgptPowDataBuild".to_string(),
            Value::String(pow_data_build.to_string()),
        );
    }
    if let Some(refresh_token) = refresh_token {
        extra_body.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.to_string()),
        );
        extra_body.insert(
            "refreshStrategy".to_string(),
            Value::String("oauth_token".to_string()),
        );
        extra_body.insert(
            "oauthTokenEndpoint".to_string(),
            Value::String("https://auth.openai.com/oauth/token".to_string()),
        );
        extra_body.insert(
            "oauthClientId".to_string(),
            Value::String("app_2SKx67EdpoN0G6j64rFvigXD".to_string()),
        );
        payload.insert(
            "refreshToken".to_string(),
            Value::String(refresh_token.to_string()),
        );
    }
    if let Some(id_token) = id_token {
        extra_body.insert("idToken".to_string(), Value::String(id_token.to_string()));
        payload.insert("idToken".to_string(), Value::String(id_token.to_string()));
    }
    if let Some(device_id) = chatgpt_web_import_string(
        raw_map,
        &[
            &["deviceId"],
            &["oaiDeviceId"],
            &["chatgptLogin", "deviceId"],
            &["platformAuth", "deviceId"],
        ],
    ) {
        extra_body.insert("deviceId".to_string(), Value::String(device_id.to_string()));
    } else {
        extra_body.insert(
            "deviceId".to_string(),
            Value::String(uuid::Uuid::new_v4().to_string()),
        );
    }
    if let Some(session_id) =
        chatgpt_web_import_string(raw_map, &[&["sessionId"], &["oaiSessionId"]]).or_else(|| {
            access_token_claims
                .as_ref()
                .and_then(|claims| claims.get("session_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        })
    {
        extra_body.insert(
            "sessionId".to_string(),
            Value::String(session_id.to_string()),
        );
    } else {
        extra_body.insert(
            "sessionId".to_string(),
            Value::String(uuid::Uuid::new_v4().to_string()),
        );
    }
    if let Some(auth_url) = chatgpt_web_import_string(
        raw_map,
        &[
            &["chatgptLogin", "authUrl"],
            &["authUrl"],
            &["chatgptAuthUrl"],
        ],
    ) {
        extra_body.insert(
            "chatgptAuthUrl".to_string(),
            Value::String(auth_url.to_string()),
        );
    }
    if let Some(mailbox_ref) =
        chatgpt_web_import_string(raw_map, &[&["chatgptLogin", "mailboxRef"], &["mailboxRef"]])
    {
        extra_body.insert(
            "mailboxRef".to_string(),
            Value::String(mailbox_ref.to_string()),
        );
    }
    if let Some(mailbox_session_id) = chatgpt_web_import_string(
        raw_map,
        &[&["chatgptLogin", "mailboxSessionId"], &["mailboxSessionId"]],
    ) {
        extra_body.insert(
            "mailboxSessionId".to_string(),
            Value::String(mailbox_session_id.to_string()),
        );
    }
    if let Some(registration_ip_country) = chatgpt_web_import_string(
        raw_map,
        &[
            &[
                "platformOrganizationDetails",
                "onboardingLogin",
                "ip_country",
            ],
            &["ip_country"],
            &["ipCountry"],
        ],
    ) {
        extra_body.insert(
            "registrationIpCountry".to_string(),
            Value::String(registration_ip_country.to_string()),
        );
    }
    if let Some(proxy_url) = chatgpt_web_import_string(
        raw_map,
        &[
            &["proxyUrl"],
            &["proxy_url"],
            &["credentialProxyUrl"],
            &["credential_proxy_url"],
            &["outboundProxy"],
            &["outbound_proxy"],
        ],
    ) {
        extra_body.insert("proxyUrl".to_string(), Value::String(proxy_url.to_string()));
    }
    if let Some(proxy_bypass) = chatgpt_web_import_string(
        raw_map,
        &[
            &["proxyBypass"],
            &["proxy_bypass"],
            &["noProxy"],
            &["no_proxy"],
        ],
    ) {
        extra_body.insert(
            "proxyBypass".to_string(),
            Value::String(proxy_bypass.to_string()),
        );
    }
    let auth_seed_email = chatgpt_web_import_string(
        raw_map,
        &[
            &["email"],
            &["loginEmail"],
            &["login_email"],
            &["accountEmail"],
        ],
    );
    let auth_seed_password = chatgpt_web_import_string(
        raw_map,
        &[&["password"], &["loginPassword"], &["login_password"]],
    );
    let auth_seed_password_sha256 = chatgpt_web_import_string(
        raw_map,
        &[
            &["passwordSha256"],
            &["password_sha256"],
            &["passwordHash"],
            &["password_hash"],
        ],
    );
    if let Some(email) = auth_seed_email {
        let mut auth_seed = serde_json::Map::new();
        auth_seed.insert(
            "type".to_string(),
            Value::String("chatgpt_web_signin".to_string()),
        );
        auth_seed.insert("email".to_string(), Value::String(email.to_string()));
        if let Some(password) = auth_seed_password {
            auth_seed.insert("password".to_string(), Value::String(password.to_string()));
        }
        if let Some(password_sha256) = auth_seed_password_sha256 {
            auth_seed.insert(
                "passwordSha256".to_string(),
                Value::String(password_sha256.to_string()),
            );
        }
        extra_body.insert("authSeed".to_string(), Value::Object(auth_seed));
    }
    payload.insert("extraBody".to_string(), Value::Object(extra_body));
    payload.insert(
        "rawSource".to_string(),
        canonicalize_folder_sync_raw_source(&Value::Object(raw_map.clone())),
    );
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint.or(Some("session_auth")),
    );
    Ok(Value::Object(payload))
}
