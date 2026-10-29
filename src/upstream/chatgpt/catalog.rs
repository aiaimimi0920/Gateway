//! Read-only Web catalog; never use Codex endpoints, solve challenges, or follow redirects.
use super::{
    bootstrap::initialize_site, headers::build_request_headers, web_reverse::build_request_context,
};
use crate::credential_runtime::SessionAuthConfig;
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use futures::StreamExt;
use serde_json::{json, Value};
use std::{collections::HashSet, time::Duration};

const URL: &str = "https://chatgpt.com/backend-api/models?history_and_training_disabled=false";
const MAX_BYTES: usize = 1024 * 1024;

fn failure(code: &'static str, message: impl Into<String>) -> GatewayError {
    GatewayError::service_unavailable(message).with_code(code)
}

pub fn accepts_origin(base: &str) -> bool {
    matches!(base.trim_end_matches('/'), "https://chatgpt.com")
}

pub async fn fetch(
    client: &rquest::Client,
    payload: &ProviderAccountPayload,
) -> Result<Value, GatewayError> {
    if !accepts_origin(&payload.base_url)
        || payload.api_key.is_empty()
        || payload.api_key.len() > 32768
        || !payload.api_key.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(GatewayError::bad_request(
            "网页目录仅支持已保存的 ChatGPT 官网账号。",
        ));
    }
    fetch_at(client, URL, payload).await
}

async fn fetch_at(
    client: &rquest::Client,
    url: &str,
    payload: &ProviderAccountPayload,
) -> Result<Value, GatewayError> {
    let payload = session_payload(payload);
    let context = build_request_context(&payload);
    // The catalog needs the same site session, not requirements/proof-script evaluation.
    initialize_site(client, Duration::from_secs(25), &payload, &context, None).await?;
    let target =
        url::Url::parse(url).map_err(|_| GatewayError::bad_request("Invalid Web catalog URL."))?;
    let mut headers = build_request_headers(
        &context,
        &payload,
        None,
        target.path(),
        "application/json",
        None,
    )?;
    // Validate the actual fixed URL as well as the route used to construct headers.
    context.cookies.apply(url, &mut headers)?;
    let response = client
        .get(url)
        .headers(headers)
        .redirect(rquest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .send()
        .await
        .map_err(|_| {
            failure(
                "chatgpt_web_catalog_network",
                "无法读取 ChatGPT 网页模型目录，请检查网络后重试。",
            )
        })?;
    let status = response.status().as_u16();
    if status == 401 {
        return Err(failure(
            "chatgpt_web_session_invalid",
            "ChatGPT 网页会话已失效（HTTP 401），请重新登录。",
        ));
    }
    if response
        .headers()
        .get("cf-mitigated")
        .is_some_and(|v| v == "challenge")
    {
        return Err(failure(
            "chatgpt_web_browser_challenge_required",
            format!("ChatGPT 网页模型目录收到上游浏览器验证响应（HTTP {status}）；未尝试自动解决验证，当前列表未被覆盖。"),
        ));
    }
    // A forbidden response alone proves neither a challenge nor an expired login.
    if status == 403 {
        return Err(failure(
            "chatgpt_web_catalog_forbidden",
            "ChatGPT 网页模型目录被上游拒绝（HTTP 403）；尚不能据此判断会话失效或模型权限，当前列表未被覆盖。",
        ));
    }
    if !response.status().is_success() {
        return Err(failure(
            "chatgpt_web_catalog_http",
            format!("ChatGPT 网页模型目录 HTTP {status}；当前列表未被覆盖。"),
        ));
    }
    if !response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(';')
                .next()
                .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
        })
    {
        return Err(failure(
            "chatgpt_web_catalog_not_json",
            "ChatGPT 网页模型目录返回非 JSON 页面，可能需要人工验证。",
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            failure(
                "chatgpt_web_catalog_network",
                "读取 ChatGPT 网页模型目录时连接中断。",
            )
        })?;
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return Err(failure(
                "chatgpt_web_catalog_too_large",
                "ChatGPT 网页模型目录超过大小限制。",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = serde_json::from_slice(&bytes).map_err(|_| {
        failure(
            "chatgpt_web_catalog_invalid",
            "ChatGPT 网页模型目录格式无效。",
        )
    })?;
    let models = parse(&body)?;
    let checked_at = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    Ok(json!({"models":models,"checkedAt":checked_at}))
}

fn session_payload(payload: &ProviderAccountPayload) -> ProviderAccountPayload {
    let mut safe = payload.clone();
    safe.adapter = super::PROVIDER.to_string();
    safe.headers
        .retain(|name, _| matches!(name.to_ascii_lowercase().as_str(), "cookie" | "user-agent"));
    if let Some(extra) = safe.extra_body.as_mut() {
        extra.retain(|name, _| {
            matches!(
                name.as_str(),
                "deviceId"
                    | "oaiDeviceId"
                    | "languageCode"
                    | "oaiLanguage"
                    | "language"
                    | "sessionId"
                    | "oaiSessionId"
                    | "clientVersion"
                    | "clientBuildNumber"
            )
        });
    }
    // Catalog authentication is fixed, not influenced by custom transport/header overrides.
    safe.session_auth = Some(SessionAuthConfig {
        transport: "bearer".into(),
        header_name: Some("authorization".into()),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        expires_at: None,
    });
    safe
}

fn parse(body: &Value) -> Result<Vec<Value>, GatewayError> {
    let entries = body
        .get("models")
        .and_then(Value::as_array)
        .filter(|items| items.len() <= 2048)
        .ok_or_else(|| {
            failure(
                "chatgpt_web_catalog_invalid",
                "ChatGPT 网页模型目录结构无效。",
            )
        })?;
    let mut seen = HashSet::new();
    let models: Vec<_> = entries
        .iter()
        .filter_map(|entry| {
            let slug = entry.get("slug")?.as_str()?;
            if slug.is_empty()
                || slug.len() > 200
                || slug.trim() != slug
                || slug.chars().any(char::is_control)
                || slug.contains(['*', '?'])
                || !seen.insert(slug)
            {
                return None;
            }
            // Web availability is not the Codex `supported_in_api` flag or a generation attestation.
            Some(json!({"slug":slug,"visibility":null,"supported_in_api":null}))
        })
        .collect();
    if models.is_empty() {
        return Err(failure(
            "chatgpt_web_catalog_empty",
            "ChatGPT 网页目录未返回有效模型；当前列表未被覆盖。",
        ));
    }
    Ok(models)
}

#[cfg(test)]
mod tests;
