//! Build the existing worker input without taking ownership of provider state.
use super::super::{read_extra_body_string, read_header_case_insensitive, read_nested_json_string};
use super::types::{ChatGptWebSessionWorkerAuthSeed, ChatGptWebSessionWorkerInput};
use crate::routing::candidate::ProviderAccountPayload;
use serde_json::Value;
use std::collections::HashMap;

const CHATGPT_WEB_CREDENTIAL_FAMILY_DIR: &str = "chatgpt-platform/chatgpt-web-reverse/session-auth";
#[derive(Debug, Clone)]
struct ChatGptWebSigninSeed {
    email: String,
    password: Option<String>,
    password_sha256: Option<String>,
}

fn chatgpt_web_signin_seed(
    extra_body: Option<&HashMap<String, Value>>,
) -> Option<ChatGptWebSigninSeed> {
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

    Some(ChatGptWebSigninSeed {
        email,
        password,
        password_sha256,
    })
}

fn sanitize_chatgpt_web_credential_file_name(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '-',
        })
        .collect::<String>();
    let collapsed = normalized
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        None
    } else if collapsed.ends_with(".json") {
        Some(collapsed)
    } else {
        Some(format!("{collapsed}.json"))
    }
}

pub(super) fn chatgpt_web_refresh_input(
    payload: &ProviderAccountPayload,
) -> ChatGptWebSessionWorkerInput {
    let credential_file_name = payload
        .credential_id
        .as_deref()
        .and_then(sanitize_chatgpt_web_credential_file_name)
        .or_else(|| {
            payload
                .account_name
                .as_deref()
                .and_then(sanitize_chatgpt_web_credential_file_name)
        });
    let cookie_header = read_header_case_insensitive(&payload.headers, "Cookie");
    ChatGptWebSessionWorkerInput {
        base_url: crate::protocol::chatgpt::web_reverse::normalize_site_base_url(&payload.base_url),
        models_path: payload
            .extra_body
            .as_ref()
            .and_then(|body| body.get("modelsPath"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| {
                Some(
                    crate::protocol::chatgpt::web_reverse::CHATGPT_WEB_DEFAULT_MODELS_PATH
                        .to_string(),
                )
            }),
        auth_url: payload
            .extra_body
            .as_ref()
            .and_then(|body| body.get("chatgptAuthUrl"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        auth_token: {
            let api_key = payload.api_key.trim();
            (!api_key.is_empty()).then_some(api_key.to_string())
        },
        cookie_header,
        user_agent: read_extra_body_string(payload.extra_body.as_ref(), &["userAgent", "ua"])
            .or_else(|| read_header_case_insensitive(&payload.headers, "User-Agent")),
        language: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["language", "acceptLanguage"],
        )
        .or_else(|| read_header_case_insensitive(&payload.headers, "Accept-Language")),
        language_code: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["languageCode", "oaiLanguage"],
        ),
        timezone: read_extra_body_string(payload.extra_body.as_ref(), &["timezone"]),
        chatgpt_pow_sources: payload
            .extra_body
            .as_ref()
            .and_then(|extra_body| extra_body.get("chatgptPowSources"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .filter(|items| !items.is_empty()),
        chatgpt_pow_data_build: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["chatgptPowDataBuild", "powDataBuild"],
        ),
        client_version: read_extra_body_string(payload.extra_body.as_ref(), &["clientVersion"]),
        client_build_number: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["clientBuildNumber"],
        ),
        device_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["deviceId", "oaiDeviceId"],
        ),
        session_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["sessionId", "oaiSessionId"],
        ),
        mailbox_ref: read_extra_body_string(payload.extra_body.as_ref(), &["mailboxRef"]),
        mailbox_session_id: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["mailboxSessionId"],
        ),
        proxy_url: read_extra_body_string(
            payload.extra_body.as_ref(),
            &[
                "proxyUrl",
                "proxy_url",
                "credentialProxyUrl",
                "credential_proxy_url",
                "outboundProxy",
                "outbound_proxy",
            ],
        ),
        proxy_bypass: read_extra_body_string(
            payload.extra_body.as_ref(),
            &["proxyBypass", "proxy_bypass", "noProxy", "no_proxy"],
        ),
        registration_ip_country: read_extra_body_string(
            payload.extra_body.as_ref(),
            &[
                "registrationIpCountry",
                "registration_ip_country",
                "ipCountry",
                "ip_country",
            ],
        ),
        write_credential_file: true,
        credential_file_name,
        credential_family_dir: Some(CHATGPT_WEB_CREDENTIAL_FAMILY_DIR.to_string()),
        credential_root_dir: std::env::var("NEURO_PROVIDER_CREDENTIAL_ROOT_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        browser_executable_path: std::env::var("CHATGPT_WEB_BROWSER_EXECUTABLE_PATH")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        user_data_dir: std::env::var("CHATGPT_WEB_BROWSER_USER_DATA_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        profile_directory: std::env::var("CHATGPT_WEB_BROWSER_PROFILE_DIR")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        auth_seed: chatgpt_web_signin_seed(payload.extra_body.as_ref()).map(|seed| {
            ChatGptWebSessionWorkerAuthSeed {
                email: seed.email,
                password: seed.password,
                password_sha256: seed.password_sha256,
            }
        }),
        relay_request: None,
    }
}
