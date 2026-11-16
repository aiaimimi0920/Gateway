//! One explicitly charged Udio task test. No audio downloads or metadata-as-playback success.
use super::{
    console_model_probe::ModelProbeResult, media_probe_request::MediaProbeRequest,
    ConsoleProbeRequest, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    error::GatewayError,
    protocol::canonical::CanonicalRelayRequest,
    routing::{candidate::ProviderAccountPayload, config::CredentialProbeTarget},
    state::AppState,
};
use serde_json::Value;
use std::time::Instant;

pub(super) fn canonical(
    media: &MediaProbeRequest,
    request: &ConsoleProbeRequest,
    model: &str,
    payload: &ProviderAccountPayload,
) -> Result<CanonicalRelayRequest, GatewayError> {
    #[cfg(feature = "line-udio-web-reverse-api")]
    {
        use crate::{
            protocol::call_flow::SpecialAdapter, routing::candidate::ProviderExecutionMode,
        };
        if !matches!(
            media.operation,
            super::media_probe_request::MediaProbeOperation::Music
        ) || !media.parameters.is_empty()
            || !payload
                .call_flow
                .is_some_and(|flow| flow.special_adapter == SpecialAdapter::UdioWeb)
            || payload.base_url.trim_end_matches('/') != "https://www.udio.com"
            || payload
                .resolve_execution_mode(crate::protocol::canonical::EndpointKind::MusicGenerations)
                != ProviderExecutionMode::BrowserBacked
            || crate::protocol::call_flow::prepare(payload)?.executor != "udio_compatible"
        {
            return Err(invalid());
        }
        // Reuse the public normalizer/model binder. No caller wait/count/continuation or webhook
        // override can turn this consent into another operation; the original IDs must all finish.
        let canonical =
            crate::protocol::producer::normalize_music_generations(serde_json::json!({
                "model":request.model,"prompt":request.prompt,"wait_audio":true
            }))?;
        crate::protocol::udio::build_generate_request(&canonical, model)?;
        Ok(canonical)
    }
    #[cfg(not(feature = "line-udio-web-reverse-api"))]
    {
        let _ = (media, request, model, payload);
        Err(invalid())
    }
}

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    request: &ConsoleProbeRequest,
    media: &MediaProbeRequest,
    canonical: CanonicalRelayRequest,
) -> (String, ProviderPayloadProbeReport) {
    let point = format!("Udio music task test (model: {model})");
    let audit = super::model_probe_recording::begin_media(
        state,
        target,
        model,
        request.model.as_deref().unwrap(),
        media.operation,
        "udio_web",
    )
    .await;
    let started = Instant::now();
    // The ordinary local/remote worker owns one send, native ID admission, deadline and no-replay.
    // It returns metadata, not validated audio. Never fetch/record the returned playback URLs.
    let output = state
        .upstream_client
        .execute_json_passthrough(
            &target.provider_id,
            target
                .payload
                .resolve_execution_mode(canonical.endpoint_kind),
            &target.payload,
            &canonical,
            model,
            None,
        )
        .await
        .and_then(|body| {
            if body["completed"] == true
                && body["data"].as_array().is_some_and(|data| !data.is_empty())
            {
                Ok(body)
            } else {
                Err(GatewayError::server_error(
                    "Udio task test did not produce complete task metadata.",
                )
                .with_provider("udio_compatible")
                .with_code("udio_probe_incomplete"))
            }
        });
    let result = match &output {
        Ok(body) => ModelProbeResult { status:Some(200), usage:Value::Null,
            reply:Ok(format!("Udio 任务终态已确认；model={model}；{} 首歌曲元数据；音频未验证，未恢复模型健康；供应商扣额未知。",
                body["data"].as_array().expect("completed Udio task metadata").len())) },
        Err(error) => ModelProbeResult { status:error.http_status, usage:Value::Null,
            reply:Err(format!("Udio 音乐任务测试失败；HTTP {}；{}。不自动重提；远端扣费可能未知。",
                error.http_status.map_or_else(||"unknown".into(),|status|status.to_string()),
                error.code.as_deref().unwrap_or("udio_probe_failed"))) },
    };
    let audit = audit.media_result("udio_compatible", model, output.as_ref().ok());
    let (status, health_recovery) = if result.reply.is_ok() {
        super::model_probe_recording::finish_without_health(
            target,
            model,
            audit,
            started.elapsed(),
            &result,
        )
        .await;
        // The existing API's neutral status excludes this unverified audio from availability/IQ.
        (ProviderPayloadProbeStatus::Unsupported, None)
    } else {
        let recovery = super::model_probe_recording::finish(
            state,
            target,
            model,
            audit,
            started.elapsed(),
            &result,
        )
        .await;
        (ProviderPayloadProbeStatus::Failed, Some(recovery))
    };
    (
        point,
        ProviderPayloadProbeReport {
            status,
            health_recovery,
            answer: None,
            message: result.reply.unwrap_or_else(|message| message),
        },
    )
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Udio task tests require the typed browser executor, fixed audience, music operation and no parameter overrides.")
        .with_provider("udio_compatible").with_code("udio_probe_input_invalid")
}
