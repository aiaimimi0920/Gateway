//! Explicit one-account media tests use ordinary senders; metadata alone never restores audio health.
use super::{
    console_model_probe::ModelProbeResult, media_probe_request::MediaProbeProduct,
    ConsoleProbeRequest, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    routing::{candidate::ProviderExecutionMode, config::CredentialProbeTarget},
    state::AppState,
};
use serde_json::Value;
use std::time::Instant;

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    request: &ConsoleProbeRequest,
    product: MediaProbeProduct,
) -> (String, ProviderPayloadProbeReport) {
    let label = product.label();
    let point = format!("{label} media test (model: {model})");
    let prepared = request
        .media
        .as_ref()
        .filter(|media| media.valid(request))
        .filter(|_| {
            request
                .credential_ids
                .as_ref()
                .is_some_and(|ids| ids.as_slice() == [target.credential_id.as_str()])
        })
        .and_then(|media| {
            media
                .canonical(request, model, &target.payload, product)
                .ok()
                .map(|canonical| (media, canonical))
        });
    let Some((media, canonical)) = prepared else {
        return (point, ProviderPayloadProbeReport { health_recovery: None, answer: None, status: ProviderPayloadProbeStatus::Unsupported,
            message: format!("{label} media operation, exact account or parameters are invalid; generation was not sent.") });
    };
    if matches!(product, MediaProbeProduct::Udio) {
        return super::udio_media_probe::probe(state, target, model, request, media, canonical)
            .await;
    }
    let audit = super::model_probe_recording::begin_media(
        state,
        target,
        model,
        request.model.as_deref().unwrap(),
        media.operation,
        product.family(),
    )
    .await;
    let started = Instant::now();
    // Suno keeps the configured mode: its ordinary browser sender binds the native model
    // and verifies all MP3s. Never force direct HTTP or involve a public Key/other account.
    let execution_mode = if matches!(
        product,
        MediaProbeProduct::Suno | MediaProbeProduct::LumaWeb
    ) {
        target
            .payload
            .resolve_execution_mode(canonical.endpoint_kind)
    } else {
        ProviderExecutionMode::DirectHttp
    };
    let output = state
        .upstream_client
        .execute_json_passthrough(
            &target.provider_id,
            execution_mode,
            &target.payload,
            &canonical,
            model,
            None,
        )
        .await
        .and_then(|body| {
            if matches!(product, MediaProbeProduct::Suno) {
                super::suno_media_probe::completed(&body)?;
            }
            Ok(body)
        });
    let result = match &output {
        Ok(_) if matches!(product, MediaProbeProduct::LumaWeb) => ModelProbeResult {
            status: Some(200), usage: Value::Null,
            reply: Ok(format!("Luma Web 图片任务完成；请求模型={model}；已验证 1 张图片；模型绑定未验证，未恢复模型健康；供应商扣额未知。")),
        },
        Ok(_) if matches!(product, MediaProbeProduct::Chataibot) => ModelProbeResult {
            status: Some(200),
            usage: Value::Null,
            reply: Ok(format!(
                "ChatAIBot 图片测试通过；model={model}；已验证 1 张图片；供应商扣额未知。"
            )),
        },
        Ok(body) if matches!(product, MediaProbeProduct::Suno) => ModelProbeResult {
            status: Some(200),
            usage: Value::Null,
            reply: Ok(super::suno_media_probe::message(body, model)),
        },
        Ok(body) => {
            let native = &body["gateway_luma_generation"];
            let credits = native["usage"]["credits_charged"].clone();
            ModelProbeResult { status: Some(200), usage: Value::Null,
                reply: Ok(format!("Luma Agents 媒体测试通过；model={model}；generation={}；credits_charged={}（null 为未知）。", native["id"].as_str().expect("validated Luma sender must return a generation ID"), credits)) }
        }
        Err(error) => ModelProbeResult {
            status: error.http_status,
            usage: Value::Null,
            reply: Err(format!(
                "{label} 媒体测试失败；HTTP {}；{}。不自动重提；远端扣费可能未知。",
                error
                    .http_status
                    .map_or_else(|| "unknown".into(), |s| s.to_string()),
                error.code.as_deref().unwrap_or("media_probe_failed")
            )),
        },
    };
    let audit = audit.media_result(
        target.payload.canonical_adapter(),
        model,
        output.as_ref().ok(),
    );
    if matches!(product, MediaProbeProduct::LumaWeb) {
        super::model_probe_recording::finish_without_health(
            target,
            model,
            audit,
            started.elapsed(),
            &result,
        )
        .await;
        // Neither success nor failure identifies the actual supplier model. A failure sentinel
        // also prevents the management caller from falling back to account-wide health writes.
        let failed = result.reply.is_err();
        return (point, ProviderPayloadProbeReport {
            status: if failed { ProviderPayloadProbeStatus::Failed } else { ProviderPayloadProbeStatus::Unsupported },
            health_recovery: failed.then_some(crate::db::provider_credential_model_states::recovery::ModelHealthRecovery::FailureNotPenalized),
            answer: None, message: result.reply.unwrap_or_else(|message| message),
        });
    }
    let health_recovery = super::model_probe_recording::finish(
        state,
        target,
        model,
        audit,
        started.elapsed(),
        &result,
    )
    .await;
    let status = if result.reply.is_ok() {
        ProviderPayloadProbeStatus::Passed
    } else {
        ProviderPayloadProbeStatus::Failed
    };
    (
        point,
        ProviderPayloadProbeReport {
            status,
            health_recovery: Some(health_recovery),
            answer: None,
            message: result.reply.unwrap_or_else(|message| message),
        },
    )
}
