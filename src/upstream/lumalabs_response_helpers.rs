//! LumaLabs execution plans, browser input ownership and response dispatch.

mod media_outputs;
mod worker_results;

#[cfg(test)]
mod tests;

// Preserve the crate-visible entry paths, including helpers used within their new owners.
#[allow(unused_imports)]
pub(crate) use worker_results::{
    build_lumalabs_browser_executor_service_result, classify_lumalabs_browser_worker_failure,
    extract_lumalabs_browser_worker_success, parse_lumalabs_browser_worker_output,
    parse_lumalabs_browser_worker_verified_output,
    parse_lumalabs_remote_browser_executor_signed_url, resolve_lumalabs_browser_worker_result,
};

#[allow(unused_imports)]
pub(crate) use media_outputs::{
    build_lumalabs_downloaded_image_response, classify_lumalabs_media_fetch_error,
    ensure_successful_lumalabs_media_fetch_status, resolve_lumalabs_downloaded_image_mime_type,
    resolve_lumalabs_image_generation_plan,
};

use crate::error::GatewayError;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::lumalabs_runtime_helpers::PreparedLumalabsBrowserExecutorServiceInput;
use std::time::Duration;

#[derive(Debug)]
pub(crate) struct PreparedLumalabsMediaPlan {
    pub(crate) operation: crate::protocol::lumalabs::LumalabsMediaOperation,
    pub(crate) prompt: String,
    pub(crate) action_body: serde_json::Value,
    pub(crate) auto_discover_action_type: bool,
    pub(crate) media_operation: &'static str,
    pub(crate) artifact_field: String,
    pub(crate) base_url: String,
    pub(crate) request_timeout: Duration,
    pub(crate) realm_id: String,
    pub(crate) locale: String,
}

#[derive(Debug)]
pub(crate) struct PreparedLumalabsExecutionContext {
    pub(crate) media_plan: PreparedLumalabsMediaPlan,
    pub(crate) browser_input: PreparedLumalabsBrowserExecutorServiceInput,
}

pub(crate) fn prepare_lumalabs_media_plan(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    default_timeout: Duration,
) -> Result<PreparedLumalabsMediaPlan, GatewayError> {
    let operation =
        crate::protocol::lumalabs::LumalabsMediaOperation::from_endpoint_kind(req.endpoint_kind)?;

    if crate::protocol::lumalabs::requested_output_count(req) > 1 {
        return Err(operation.unsupported_output_count_error());
    }

    if operation == crate::protocol::lumalabs::LumalabsMediaOperation::Image {
        let has_input_images = req.raw_body.get("image").is_some()
            || req
                .raw_body
                .get("images")
                .and_then(|value| value.as_array())
                .map(|values| !values.is_empty())
                .unwrap_or(false);
        if has_input_images || req.raw_body.get("mask").is_some() {
            return Err(crate::protocol::lumalabs::unsupported_image_inputs_error());
        }
    }

    let runtime = crate::protocol::lumalabs::runtime_from_payload(payload)?;
    let prompt = crate::protocol::lumalabs::prompt_from_request_for_operation(req, operation)?;
    let optimistic_output_id = crate::protocol::lumalabs::generate_optimistic_output_id();
    let action_body = crate::protocol::lumalabs::build_action_request_for_operation(
        req,
        &runtime,
        operation,
        model,
        &prompt,
        &optimistic_output_id,
    );
    let auto_discover_action_type =
        crate::protocol::lumalabs::should_auto_discover_action_type(req, &runtime, operation);
    let media_operation = crate::protocol::lumalabs::media_operation_name(operation);
    let artifact_field =
        crate::protocol::lumalabs::output_artifact_field_for_operation(req, &runtime, operation);
    let page_base_url = payload.base_url.trim_end_matches('/').to_string();
    let request_timeout = match operation {
        crate::protocol::lumalabs::LumalabsMediaOperation::Image => {
            default_timeout.max(Duration::from_secs(90))
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Video => {
            default_timeout.max(Duration::from_secs(600))
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Audio => {
            default_timeout.max(Duration::from_secs(300))
        }
    };

    Ok(PreparedLumalabsMediaPlan {
        operation,
        prompt,
        action_body,
        auto_discover_action_type,
        media_operation,
        artifact_field,
        base_url: page_base_url,
        request_timeout,
        realm_id: runtime.realm_id,
        locale: crate::upstream::lumalabs_runtime_helpers::lumalabs_locale(payload).to_string(),
    })
}

pub(crate) fn prepare_lumalabs_execution_context(
    payload: &ProviderAccountPayload,
    req: &crate::protocol::canonical::CanonicalRelayRequest,
    model: &str,
    default_timeout: Duration,
) -> Result<PreparedLumalabsExecutionContext, GatewayError> {
    let media_plan = prepare_lumalabs_media_plan(payload, req, model, default_timeout)?;
    Ok(PreparedLumalabsExecutionContext {
        browser_input: prepare_lumalabs_browser_execution_input(&media_plan, &payload.api_key),
        media_plan,
    })
}

pub(crate) fn prepare_lumalabs_browser_execution_input(
    media_plan: &PreparedLumalabsMediaPlan,
    session_token: &str,
) -> PreparedLumalabsBrowserExecutorServiceInput {
    PreparedLumalabsBrowserExecutorServiceInput {
        base_url: media_plan.base_url.clone(),
        realm_id: media_plan.realm_id.clone(),
        media_operation: Some(media_plan.media_operation.to_string()),
        artifact_field: media_plan.artifact_field.clone(),
        session_token: session_token.to_string(),
        action_body: media_plan.action_body.clone(),
        auto_discover_action_type: media_plan.auto_discover_action_type,
        locale: media_plan.locale.clone(),
        timeout: media_plan.request_timeout,
    }
}

pub(crate) fn build_lumalabs_non_image_generation_response(
    operation: crate::protocol::lumalabs::LumalabsMediaOperation,
    model: &str,
    prompt: &str,
    signed_url: &str,
) -> serde_json::Value {
    match operation {
        crate::protocol::lumalabs::LumalabsMediaOperation::Video => {
            crate::protocol::lumalabs::build_video_generation_response(model, prompt, signed_url)
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Audio => {
            crate::protocol::lumalabs::build_audio_generation_response(model, prompt, signed_url)
        }
        crate::protocol::lumalabs::LumalabsMediaOperation::Image => {
            unreachable!("image responses use the image generation plan helper")
        }
    }
}
