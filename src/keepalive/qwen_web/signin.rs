//! Qwen direct sign-in: credential attempts, headers and HTTP authentication.
use super::super::{
    collect_set_cookie_header, decode_jwt_expiry_iso, external_gateway_headers,
    read_extra_body_string, read_json_field_string, read_nested_json_string,
    request_builder_with_headers,
};
use super::types::QwenWebRefreshedRuntime;
use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use rquest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tracing::warn;

#[derive(Debug, Clone)]
pub(in crate::keepalive) struct QwenWebSigninSeed {
    pub(in crate::keepalive) email: String,
    pub(in crate::keepalive) password: Option<String>,
    pub(in crate::keepalive) password_sha256: Option<String>,
}

pub(in crate::keepalive) fn qwen_web_signin_seed(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<QwenWebSigninSeed> {
    let extra_body = extra_body?;
    let auth_seed = extra_body
        .get("authSeed")
        .or_else(|| extra_body.get("auth_seed"))
        .or_else(|| extra_body.get("signinSeed"))
        .or_else(|| extra_body.get("signin_seed"));

    let email = read_nested_json_string(
        auth_seed,
        &[
            "email",
            "loginEmail",
            "login_email",
            "account",
            "accountEmail",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "email",
                "loginEmail",
                "login_email",
                "account",
                "accountEmail",
            ],
        )
    })?;

    let password =
        read_nested_json_string(auth_seed, &["password", "loginPassword", "login_password"])
            .or_else(|| {
                read_extra_body_string(
                    Some(extra_body),
                    &["password", "loginPassword", "login_password"],
                )
            });
    let password_sha256 = read_nested_json_string(
        auth_seed,
        &[
            "passwordSha256",
            "password_sha256",
            "passwordHash",
            "password_hash",
        ],
    )
    .or_else(|| {
        read_extra_body_string(
            Some(extra_body),
            &[
                "passwordSha256",
                "password_sha256",
                "passwordHash",
                "password_hash",
            ],
        )
    });

    if password.is_none() && password_sha256.is_none() {
        return None;
    }

    Some(QwenWebSigninSeed {
        email,
        password,
        password_sha256,
    })
}

pub(in crate::keepalive) fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hex::encode(hasher.finalize())
}

pub(in crate::keepalive) fn qwen_web_signin_password_attempts(
    seed: &QwenWebSigninSeed,
) -> Vec<String> {
    let mut attempts = Vec::new();
    if let Some(password) = seed
        .password
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        attempts.push(password.to_string());
        let hashed = sha256_hex(password);
        if !attempts.contains(&hashed) {
            attempts.push(hashed);
        }
    }
    if let Some(password_sha256) = seed
        .password_sha256
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !attempts.iter().any(|entry| entry == password_sha256) {
            attempts.push(password_sha256.to_string());
        }
    }
    attempts
}

pub(in crate::keepalive) fn qwen_web_signin_headers(
    payload: &ProviderAccountPayload,
) -> HashMap<String, String> {
    let base_url = payload.base_url.trim_end_matches('/').to_string();
    let mut headers = external_gateway_headers(&payload.headers);
    let defaults = [
        (
            "User-Agent".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_USER_AGENT.to_string(),
        ),
        ("Connection".to_string(), "keep-alive".to_string()),
        (
            "Accept".to_string(),
            "application/json, text/plain, */*".to_string(),
        ),
        (
            "Accept-Encoding".to_string(),
            "gzip, deflate, br, zstd".to_string(),
        ),
        (
            "Timezone".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_TIMEZONE.to_string(),
        ),
        (
            "sec-ch-ua".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_SEC_CH_UA.to_string(),
        ),
        ("source".to_string(), "web".to_string()),
        (
            "Version".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_VERSION.to_string(),
        ),
        (
            "bx-v".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_BX_VERSION.to_string(),
        ),
        ("Sec-Fetch-Site".to_string(), "same-origin".to_string()),
        ("Sec-Fetch-Mode".to_string(), "cors".to_string()),
        ("Sec-Fetch-Dest".to_string(), "empty".to_string()),
        (
            "Accept-Language".to_string(),
            crate::protocol::qwen_web::QWEN_WEB_DEFAULT_ACCEPT_LANGUAGE.to_string(),
        ),
        ("Origin".to_string(), base_url.clone()),
        ("Referer".to_string(), format!("{base_url}/auth")),
        ("Content-Type".to_string(), "application/json".to_string()),
    ];
    for (key, value) in defaults {
        if !headers.contains_key(&key) {
            headers.insert(key, value);
        }
    }
    headers.remove("Authorization");
    headers.remove("authorization");
    headers
}

pub(super) async fn execute_qwen_web_http_signin_refresh(
    payload: &ProviderAccountPayload,
) -> Result<Option<QwenWebRefreshedRuntime>, GatewayError> {
    let Some(seed) = qwen_web_signin_seed(payload.extra_body.as_ref()) else {
        return Ok(None);
    };

    let provider = "qwen_web_compatible";
    let base_url = payload.base_url.trim_end_matches('/').to_string();
    let signin_url = format!("{base_url}/api/v1/auths/signin");
    let auths_url = format!("{base_url}/api/v1/auths/");
    let client = Client::new();
    let signin_headers = qwen_web_signin_headers(payload);
    let password_attempts = qwen_web_signin_password_attempts(&seed);
    if password_attempts.is_empty() {
        return Ok(None);
    }

    for password_value in password_attempts {
        let signin_response = match request_builder_with_headers(
            &client,
            rquest::Method::POST,
            &signin_url,
            &signin_headers,
        )
        .json(&json!({
            "email": seed.email,
            "password": password_value,
        }))
        .send()
        .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web HTTP signin refresh request failed"
                );
                continue;
            }
        };

        let signin_status = signin_response.status().as_u16();
        let signin_content_type = signin_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let set_cookie_header = collect_set_cookie_header(signin_response.headers());
        let signin_body = match signin_response.text().await {
            Ok(body) => body,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web HTTP signin refresh response body could not be read"
                );
                continue;
            }
        };

        if crate::protocol::qwen_web::response_indicates_browser_challenge(
            signin_status,
            signin_content_type.as_deref(),
            &signin_body,
        ) {
            warn!(
                provider = provider,
                email = %seed.email,
                status = signin_status,
                "Qwen Web HTTP signin hit an upstream challenge; browser-backed refresh is required"
            );
            return Ok(None);
        }

        if !(200..300).contains(&signin_status) {
            continue;
        }

        let signin_json: Value = match serde_json::from_str(&signin_body) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let Some(auth_token) = signin_json
            .get("token")
            .or_else(|| signin_json.get("access_token"))
            .or_else(|| signin_json.get("data").and_then(|value| value.get("token")))
            .or_else(|| {
                signin_json
                    .get("data")
                    .and_then(|value| value.get("access_token"))
            })
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
        else {
            continue;
        };

        let mut auth_headers = qwen_web_signin_headers(payload);
        auth_headers.insert("authorization".to_string(), format!("Bearer {auth_token}"));
        let auth_response = match request_builder_with_headers(
            &client,
            rquest::Method::GET,
            &auths_url,
            &auth_headers,
        )
        .send()
        .await
        {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web auth probe after HTTP signin failed"
                );
                continue;
            }
        };
        let auth_status = auth_response.status().as_u16();
        let auth_content_type = auth_response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let auth_body = match auth_response.text().await {
            Ok(body) => body,
            Err(error) => {
                warn!(
                    provider = provider,
                    email = %seed.email,
                    error = %error,
                    "Qwen Web auth probe body after HTTP signin could not be read"
                );
                continue;
            }
        };

        if crate::protocol::qwen_web::response_indicates_browser_challenge(
            auth_status,
            auth_content_type.as_deref(),
            &auth_body,
        ) || crate::protocol::qwen_web::response_indicates_session_invalid(
            auth_status,
            auth_content_type.as_deref(),
            &auth_body,
        ) || !(200..300).contains(&auth_status)
        {
            continue;
        }

        let auth_json: Value = serde_json::from_str(&auth_body).unwrap_or(Value::Null);
        let expires_at = read_json_field_string(&auth_json, "expires_at")
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "expires_at"))
            })
            .or_else(|| decode_jwt_expiry_iso(Some(&auth_token)));
        let account_name = read_json_field_string(&auth_json, "email")
            .or_else(|| read_json_field_string(&auth_json, "id"))
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "email"))
                    .or_else(|| {
                        auth_json
                            .get("data")
                            .and_then(|value| read_json_field_string(value, "id"))
                    })
            })
            .or_else(|| Some(seed.email.clone()));
        let credential_material_key = read_json_field_string(&auth_json, "id")
            .or_else(|| {
                auth_json
                    .get("data")
                    .and_then(|value| read_json_field_string(value, "id"))
            })
            .map(|value| format!("qwen-web-user:{value}"));

        return Ok(Some(QwenWebRefreshedRuntime {
            api_key: auth_token,
            expires_at,
            cookie_header: set_cookie_header,
            account_name,
            selected_display_model: None,
            credential_material_key,
        }));
    }

    Ok(None)
}
