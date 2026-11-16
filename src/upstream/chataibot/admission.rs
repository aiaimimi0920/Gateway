//! All local capability checks precede settings/submission; current typed Web has a fixed audience.
use super::no_replay;
use crate::{
    error::GatewayError,
    protocol::{
        canonical::{CanonicalRelayRequest, EndpointKind},
        chataibot,
    },
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, HeaderValue};

pub(super) enum Body {
    Json(serde_json::Value),
    Multipart(chataibot::MultipartBody),
}
pub(super) struct Prepared {
    pub path: &'static str,
    pub body: Body,
    pub prompt: String,
    pub url_response: bool,
}
pub(super) fn typed(payload: &ProviderAccountPayload) -> bool {
    payload.call_flow.is_some_and(|flow| {
        flow.special_adapter == crate::protocol::call_flow::SpecialAdapter::ChataibotWeb
    })
}
fn input(code: &str, message: &str) -> GatewayError {
    no_replay(
        GatewayError::bad_request(message)
            .with_code(code)
            .with_provider("chataibot_compatible"),
    )
}
impl Prepared {
    pub fn new(
        payload: &ProviderAccountPayload,
        req: &CanonicalRelayRequest,
        model: &str,
    ) -> Result<Self, GatewayError> {
        if !matches!(
            req.endpoint_kind,
            EndpointKind::ImagesGenerations | EndpointKind::ImagesEdits
        ) {
            return Err(super::unsupported_request_plan_error());
        }
        if req.raw_body.get("mask").is_some() {
            return Err(input(
                "unsupported_chataibot_mask",
                "ChatAIBot 不支持 mask 编辑。",
            ));
        }
        if req.stream
            || req.raw_body.get("stream").is_some_and(|v| v != false)
            || req.raw_body.get("n").is_some_and(|v| v.as_u64() != Some(1))
        {
            return Err(input(
                "chataibot_image_count",
                "ChatAIBot 每次只提交一项图片操作，不支持 stream 或 n!=1。",
            ));
        }
        if typed(payload)
            && (req.raw_body.as_object().is_none_or(|b| {
                b.keys().any(|k| {
                    !matches!(
                        k.as_str(),
                        "model"
                            | "prompt"
                            | "n"
                            | "response_format"
                            | "size"
                            | "image"
                            | "images"
                            | "stream"
                    )
                })
            }) || req
                .raw_body
                .get("size")
                .is_some_and(|v| v.as_str() != Some("auto")))
        {
            return Err(input(
                "chataibot_parameter_unsupported",
                "当前 ChatAIBot Web 不支持显式比例或未实现的参数。",
            ));
        }
        let url_response = chataibot::prefers_url_response(req).map_err(no_replay)?;
        let prompt = chataibot::prompt_from_request(req).map_err(no_replay)?;
        if prompt.len() > 8192 {
            return Err(input(
                "chataibot_prompt_limit",
                "ChatAIBot prompt 超过本地预算。",
            ));
        }
        let uploads =
            chataibot::extract_uploads_from_request_body(&req.raw_body).map_err(no_replay)?;
        let upload_bytes = uploads
            .iter()
            .try_fold(0usize, |n, u| n.checked_add(u.base64_data.len()));
        if uploads.len() > 4
            || upload_bytes.is_none_or(|n| n > 16 * 1024 * 1024)
            || uploads.iter().any(|u| {
                !matches!(
                    u.mime_type.as_str(),
                    "image/png" | "image/jpeg" | "image/jpg" | "image/webp" | "image/gif"
                )
            })
        {
            return Err(input(
                "chataibot_upload_limit",
                "ChatAIBot 上传最多四张受支持图片，编码总量最多 16 MiB。",
            ));
        }
        let editing = req.endpoint_kind == EndpointKind::ImagesEdits;
        if editing == uploads.is_empty() {
            return Err(input(
                "chataibot_upload_endpoint",
                "生成不能附图，编辑必须提供图片。",
            ));
        }
        let spec = chataibot::resolve_model_spec(model).ok_or_else(|| {
            input(
                "unsupported_chataibot_model",
                "ChatAIBot 模型未实现；不会猜测账号权益。",
            )
        })?;
        let (path, body) = if uploads.len() > 1 {
            let mode = spec.merge_mode.ok_or_else(|| {
                input("unsupported_chataibot_merge_model", "模型未实现多图合成。")
            })?;
            (
                "/api/file/merge",
                Body::Multipart(
                    chataibot::build_merge_multipart_body(&prompt, &uploads, mode)
                        .map_err(no_replay)?,
                ),
            )
        } else if let Some(upload) = uploads.first() {
            let mode = spec
                .edit_mode
                .ok_or_else(|| input("unsupported_chataibot_edit_model", "模型未实现图片编辑。"))?;
            (
                "/api/file/recognize",
                Body::Multipart(
                    chataibot::build_edit_multipart_body(&prompt, upload, mode)
                        .map_err(no_replay)?,
                ),
            )
        } else {
            let mut body = chataibot::build_generation_request(&prompt, spec);
            body["requestId"] = serde_json::json!(uuid::Uuid::new_v4().to_string());
            ("/api/image/generate", Body::Json(body))
        };
        Ok(Self {
            path,
            body,
            prompt,
            url_response,
        })
    }
}

pub(super) fn headers(
    payload: &ProviderAccountPayload,
    extra: Option<&std::collections::HashMap<String, String>>,
) -> Result<HeaderMap, GatewayError> {
    if !typed(payload) {
        return Ok(crate::upstream::headers::build_upstream_headers_with(
            payload, extra,
        ));
    }
    let invalid = || {
        input(
            "chataibot_web_material",
            "ChatAIBot Web 材料或受众无效；不使用其他产品认证。",
        )
    };
    let url = url::Url::parse(&payload.base_url).map_err(|_| invalid())?;
    if url.scheme() != "https"
        || url.host_str() != Some("chataibot.pro")
        || url.port().is_some()
        || !matches!(url.path(), "" | "/")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || payload.adapter != "chataibot_compatible"
        || payload.api_key.is_empty()
        || payload.api_key.len() > 16384
        || !payload
            .api_key
            .bytes()
            .all(|b| b.is_ascii_graphic() && b != b';')
        || payload.extra_body.as_ref().is_some_and(|b| !b.is_empty())
        || payload.keepalive.is_some()
        || payload.auth_token.is_some()
        || payload.auth_mode.is_some()
        || payload.session_auth.as_ref().is_some_and(|s| {
            s.transport != "cookie" || s.primary_cookie_name.as_deref() != Some("token")
        })
    {
        return Err(invalid());
    }
    for expiry in [
        payload.expires_at.as_deref(),
        payload
            .session_auth
            .as_ref()
            .and_then(|s| s.expires_at.as_deref()),
    ]
    .into_iter()
    .flatten()
    {
        let expiry =
            time::OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
                .map_err(|_| invalid())?;
        if expiry <= time::OffsetDateTime::now_utc() {
            return Err(no_replay(
                GatewayError::unauthorized("ChatAIBot Web 会话已到期。")
                    .with_code("chataibot_web_expired"),
            ));
        }
    }
    let mut headers = HeaderMap::new();
    for (key, value) in &payload.headers {
        let name = key.to_ascii_lowercase();
        if !matches!(name.as_str(), "cookie" | "user-agent" | "accept-language") {
            continue;
        }
        if value.is_empty() || value.len() > if name == "cookie" { 16384 } else { 2048 } {
            return Err(invalid());
        }
        let mut header = HeaderValue::from_str(value).map_err(|_| invalid())?;
        if name == "cookie" {
            let mut names = std::collections::HashSet::new();
            let mut found = false;
            for pair in value.split(';') {
                let (key, val) = pair.trim().split_once('=').ok_or_else(invalid)?;
                if key.is_empty()
                    || !key
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                    || !val.bytes().all(|b| b.is_ascii_graphic() && b != b';')
                    || !names.insert(key)
                {
                    return Err(invalid());
                }
                if key == "token" {
                    if val != payload.api_key {
                        return Err(invalid());
                    }
                    found = true;
                }
            }
            if !found {
                return Err(invalid());
            }
            header.set_sensitive(true);
        }
        if headers
            .insert(
                rquest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|_| invalid())?,
                header,
            )
            .is_some()
        {
            return Err(invalid());
        }
    }
    if !headers.contains_key("cookie") {
        let mut cookie =
            HeaderValue::from_str(&format!("token={}", payload.api_key)).map_err(|_| invalid())?;
        cookie.set_sensitive(true);
        headers.insert("cookie", cookie);
    }
    for (key, value) in [
        ("origin", "https://chataibot.pro"),
        ("referer", "https://chataibot.pro/app/chat?chat_id=-2"),
        ("accept", "application/json"),
        ("x-distribution-channel", "web"),
        ("x-supports-async-image-jobs", "true"),
    ] {
        headers.insert(key, HeaderValue::from_static(value));
    }
    Ok(headers)
}
