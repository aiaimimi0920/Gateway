//! Dispatch folder credential sources without changing provider-specific contracts.

use super::canonicalization::canonicalize_folder_surface_slug;
use super::payload_metadata::apply_folder_sync_metadata;
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;

mod accio;
mod chataibot;
mod chatgpt_auth;
mod chatgpt_web;
mod codex;
mod gemini_business;
mod gemini_canvas;
mod gemini_web;
mod qwen_web;
mod source;

use accio::normalize_accio_import_payload;
use chataibot::normalize_chataibot_import_payload;
use chatgpt_web::normalize_chatgpt_web_import_payload;
use codex::normalize_codex_import_payload;
use gemini_business::normalize_gemini_business_import_payload;
use gemini_canvas::normalize_gemini_canvas_import_payload;
use gemini_web::normalize_gemini_web_import_payload;
use qwen_web::normalize_qwen_web_import_payload;

pub(super) fn normalize_import_payload(
    surface_slug: &str,
    provider_account: &db::GatewayProviderAccountView,
    raw_payload: Value,
    credential_material_kind_hint: Option<&str>,
) -> Result<Value, GatewayError> {
    let surface_slug = canonicalize_folder_surface_slug(surface_slug);
    if surface_slug == "codex" {
        return normalize_codex_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "chatgpt-codex-backend" {
        return normalize_codex_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "accio" {
        return normalize_accio_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "qwen-web-chat" {
        return normalize_qwen_web_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "chataibot-images" {
        return normalize_chataibot_import_payload(provider_account, raw_payload);
    }
    if surface_slug == "gemini-web-chat" {
        return normalize_gemini_web_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if surface_slug == "chatgpt-web-reverse" {
        return normalize_chatgpt_web_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if surface_slug == "gemini-business-images" {
        return normalize_gemini_business_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }
    if matches!(
        surface_slug.as_str(),
        "gemini-canvas"
            | "gemini-canvas-chat-tts"
            | "gemini-canvas-browser-relay"
            | "gemini-canvas-program-relay"
            | "gemini-canvas-images"
            | "gemini-canvas-music"
            | "gemini-canvas-videos"
    ) {
        return normalize_gemini_canvas_import_payload(
            provider_account,
            raw_payload,
            credential_material_kind_hint,
        );
    }

    let Some(mut payload) = raw_payload.as_object().cloned() else {
        return Err(GatewayError::bad_request(
            "provider credential payload 必须是 JSON object",
        ));
    };
    apply_folder_sync_metadata(
        &mut payload,
        provider_account,
        credential_material_kind_hint,
    );
    Ok(Value::Object(payload))
}
