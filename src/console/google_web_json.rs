//! Offline JSON credential import; returns a transient draft, never a browser/validation success.
mod cookies;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod stitch_tests;
#[cfg(test)]
mod tests;

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::gemini_auth_sessions::{
    CreateGeminiAuthSessionInput, GeminiAuthFamily, GeminiAuthSecretEdit, GeminiAuthSessionStatus,
    GeminiAuthSessionView, GeminiGeneratedCredentialDraft,
};
use crate::{error::GatewayError, object_storage::gateway_object_storage};

const MAX_BYTES: usize = 64 * 1024;
const AISTUDIO_MODELS: [&str; 4] = [
    "gemini-3.5-flash-lite",
    "gemini-3.5-transcribe",
    "gemini-3.8-flash-lite-tts",
    "gemini-3.1-flash-lite-image",
];
const CANVAS_MODELS: [&str; 3] = [
    "gemini-3-flash-preview",
    "gemini-2.5-flash-image",
    "gemini-2.5-flash-preview-tts",
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Material {
    schema_version: u8,
    provider: String,
    auth_user: String,
    cookies: Vec<cookies::Cookie>,
    #[serde(default)]
    session: Option<serde_json::Map<String, Value>>,
}

fn invalid(message: &str) -> GatewayError {
    GatewayError::bad_request(message).with_code("console_google_web_json_invalid")
}

fn provider_name(family: GeminiAuthFamily) -> Result<&'static str, GatewayError> {
    match family {
        GeminiAuthFamily::GeminiWeb => Ok("gemini-web"),
        GeminiAuthFamily::GeminiCanvas => Ok("gemini-canvas"),
        GeminiAuthFamily::AistudioWeb => Ok("aistudio-web"),
        GeminiAuthFamily::FlowWeb => Ok("flow-web"),
        GeminiAuthFamily::StitchWeb => Ok("stitch-web"),
        _ => Err(invalid("该认证类型尚不支持网页 JSON 导入。")),
    }
}

fn parse(source: &str, family: GeminiAuthFamily) -> Result<Material, GatewayError> {
    if source.len() > MAX_BYTES {
        return Err(invalid("网页会话 JSON 不能超过 64 KiB。"));
    }
    let mut material: Material = serde_json::from_str(source)
        .map_err(|_| invalid("网页会话 JSON 格式无效或含有不支持字段；不接受完整浏览器状态。"))?;
    if material.schema_version != 1
        || material.provider != provider_name(family)?
        || material.auth_user.is_empty()
        || material.auth_user.len() > 3
        || !material.auth_user.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid(
            "网页会话 JSON 的版本、服务商类型或账号索引不匹配。",
        ));
    }
    // The legacy Web HTTP owner bootstraps /app and does not apply authUser.
    // Reject secondary-account claims rather than silently calling account zero.
    if family == GeminiAuthFamily::GeminiWeb && material.auth_user != "0" {
        return Err(invalid("Gemini Web JSON 当前只支持 authUser 为 0 的会话。"));
    }
    if family == GeminiAuthFamily::FlowWeb && material.auth_user != "0" {
        return Err(invalid("Flow Web JSON 当前只支持 authUser 为 0 的会话。"));
    }
    if family == GeminiAuthFamily::StitchWeb && material.auth_user != "0" {
        return Err(invalid("Stitch Web JSON 当前只支持 authUser 为 0 的会话。"));
    }
    if let Some(session) = &material.session {
        if family != GeminiAuthFamily::GeminiWeb
            || session.len() > 5
            || session.iter().any(|(key, value)| {
                !matches!(
                    key.as_str(),
                    "modelHeader"
                        | "modelHeader2"
                        | "modelHeader3"
                        | "requestContextHeader"
                        | "language"
                ) || value.as_str().is_none_or(|s| {
                    s.len() > 4096 || !s.bytes().all(|b| (0x20..=0x7e).contains(&b))
                })
            })
        {
            return Err(invalid("网页会话元数据含有不支持的字段或值。"));
        }
    }
    cookies::validate(&mut material.cookies, family)?;
    Ok(material)
}

fn draft(
    material: Material,
    input: &CreateGeminiAuthSessionInput,
    id: &str,
) -> Result<(GeminiGeneratedCredentialDraft, Option<(String, Vec<u8>)>), GatewayError> {
    let family = provider_name(input.target_family)?;
    let mut credential = json!({"id":format!("{family}-{id}"), "account_name":input.account_label.as_deref().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(family),
        "enabled":true, "scheduled_probe_enabled":false});
    let mut secret_edits = Vec::new();
    let mut object = None;
    if input.target_family == GeminiAuthFamily::GeminiWeb {
        let cookie = material
            .cookies
            .iter()
            .find(|cookie| {
                cookie.name == "__Secure-1PSID" && cookie.matches_root("gemini.google.com")
            })
            .expect("validated primary cookie");
        secret_edits.push(GeminiAuthSecretEdit {
            field: "api_key".into(),
            operation: "replace".into(),
            value: cookie.value.clone(),
        });
        if let Some(cookie) = material.cookies.iter().find(|cookie| {
            cookie.name == "__Secure-1PSIDTS" && cookie.matches_root("gemini.google.com")
        }) {
            secret_edits.push(GeminiAuthSecretEdit {
                field: "auth_token".into(),
                operation: "replace".into(),
                value: cookie.value.clone(),
            });
        }
        secret_edits.push(GeminiAuthSecretEdit {
            field: "headers/Cookie".into(),
            operation: "replace".into(),
            value: cookies::web_header(&material.cookies)?,
        });
        let mut session = material.session.unwrap_or_default();
        session.insert("authUser".into(), json!(material.auth_user));
        credential["extra_body"] = json!(session);
        credential["supported_models"] = json!(["gemini-web-chat"]);
    } else {
        let key = format!("credential-runtime/{family}/{id}/http-cookies.json");
        let value = if matches!(
            input.target_family,
            GeminiAuthFamily::AistudioWeb | GeminiAuthFamily::FlowWeb | GeminiAuthFamily::StitchWeb
        ) {
            json!({"schemaVersion":1,"provider":family,"authUser":material.auth_user,"cookies":material.cookies})
        } else {
            json!({"cookies":material.cookies})
        };
        let bytes = serde_json::to_vec(&value).map_err(|_| invalid("Cookie 无效。"))?;
        if bytes.len() > 32768 {
            return Err(invalid("Cookie 材料不能超过 32 KiB。"));
        }
        object = Some((key.clone(), bytes));
        credential["runtime_state_object_key"] = json!(key);
        credential["supported_models"] = if input.target_family == GeminiAuthFamily::FlowWeb {
            json!([])
        } else if input.target_family == GeminiAuthFamily::StitchWeb {
            // Product operation alias, not a fetched Google model identifier.
            json!([crate::upstream::stitch_web::MODEL])
        } else if input.target_family == GeminiAuthFamily::AistudioWeb {
            json!(AISTUDIO_MODELS)
        } else {
            json!(CANVAS_MODELS)
        };
        if input.target_family == GeminiAuthFamily::GeminiCanvas {
            credential["extra_body"] = json!({"authUser":material.auth_user});
        }
    }
    // These are configured routing hints, not a fetched catalog or a successful model probe.
    credential["model_selection"] = json!({"fetched":[],"manual":credential["supported_models"]});
    Ok((
        GeminiGeneratedCredentialDraft {
            provider_id: input.provider_id.trim().into(),
            credential,
            secret_edits,
        },
        object,
    ))
}

pub(crate) fn validate_flow_bundle(source: &str) -> Result<Vec<u8>, GatewayError> {
    let material = parse(source, GeminiAuthFamily::FlowWeb)?;
    serde_json::to_vec(&json!({"schemaVersion":1,"provider":"flow-web","authUser":material.auth_user,"cookies":material.cookies}))
        .map_err(|_| invalid("Flow Cookie 材料无效。"))
}

pub(crate) fn validate_stitch_bundle(source: &str) -> Result<Vec<u8>, GatewayError> {
    let material = parse(source, GeminiAuthFamily::StitchWeb)?;
    serde_json::to_vec(&json!({"schemaVersion":1,"provider":"stitch-web","authUser":material.auth_user,"cookies":material.cookies}))
        .map_err(|_|invalid("Stitch Cookie 材料无效。"))
}

pub async fn import(
    input: CreateGeminiAuthSessionInput,
    source: &str,
) -> Result<GeminiAuthSessionView, GatewayError> {
    let material = parse(source, input.target_family)?;
    let id = Uuid::new_v4().to_string();
    let (draft, object) = draft(material, &input, &id)?;
    if let Some((key, bytes)) = object {
        let failed = || {
            GatewayError::service_unavailable("无法保存网页凭据对象，请检查 Gateway 存储设置。")
                .with_code("console_google_web_json_storage_failed")
        };
        gateway_object_storage()
            .map_err(|_| failed())?
            .put_bytes(&key, bytes, "application/json")
            .await
            .map_err(|_| failed())?;
    }
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    Ok(GeminiAuthSessionView {
        id,
        target_family: input.target_family,
        provider_id: input.provider_id,
        status: GeminiAuthSessionStatus::Succeeded,
        message: "网页 JSON 已解析并准备账号草稿；未联网验证账号或模型。".into(),
        created_at: now.clone(),
        updated_at: now,
        generated_drafts: vec![draft],
    })
}
