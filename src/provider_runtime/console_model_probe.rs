//! Explicit model/search tests require real output from the selected credential.
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::{ProviderPayloadProbeReport, ProviderPayloadProbeStatus};
use crate::routing::config::CredentialProbeTarget;
use crate::state::AppState;

pub(super) struct ModelProbeResult {
    pub status: Option<u16>,
    pub reply: Result<String, String>,
    pub usage: Value,
}

pub(crate) async fn probe_console_target(
    state: &AppState,
    target: &CredentialProbeTarget,
) -> (String, ProviderPayloadProbeReport) {
    probe_console_target_with_request(state, target, None).await
}

pub(crate) async fn probe_console_target_with_request(
    state: &AppState,
    target: &CredentialProbeTarget,
    options: Option<&super::ConsoleProbeRequest>,
) -> (String, ProviderPayloadProbeReport) {
    let snapshot = state.route_config.snapshot();
    let nvidia = snapshot.document().providers.iter().any(|provider| {
        provider.id == target.provider_id && provider.preset.as_deref() == Some("nvidia-openai")
    }) || target.provider_id == "nvidia";
    let point = super::provider_payload_probe_point(&target.payload);
    if crate::protocol::vercel_free::owns_payload(&target.payload) {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Test Vercel through its configured chat route; no upstream probe was sent.",
            ),
        );
    }
    let strict_product = crate::protocol::commandcode::owns_payload(&target.payload)
        || crate::protocol::sensenova::owns_payload(&target.payload)
        || crate::protocol::agnes::owns_payload(&target.payload)
        || crate::protocol::luma_inference::owns_payload(&target.payload)
        || crate::protocol::minimax::owns_payload(&target.payload)
        || crate::protocol::nous_portal::business::owns_payload(&target.payload)
        || crate::protocol::lobsterai::owns_payload(&target.payload)
        || crate::protocol::quick_oauth::owns_payload(&target.payload);
    if strict_product && options.is_none() {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "This product requires an explicit model test; no discovery or generation was sent."));
    }
    if crate::protocol::sap_aicore::owns_payload(&target.payload) && options.is_none() {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "SAP AI Core requires an explicit model test; no discovery or generation was sent.",
            ),
        );
    }
    if state.lifecycle.model_circuits.network_blocked() {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Gateway egress network is temporarily unstable; probe was not sent.",
            ),
        );
    }
    if !target.enabled {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Credential is disabled.",
            ),
        );
    }
    let media_product = super::media_probe_request::MediaProbeProduct::for_payload(&target.payload);
    let media = options.and_then(|request| request.media.as_ref());
    if media_product.is_some() != media.is_some() {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "Media tests require a supported product, explicit operation, charge confirmation and one exact account; text/default tests never generate media."));
    }
    let codex = snapshot.document().providers.iter().any(|provider| {
        provider.id == target.provider_id
            && provider.preset.as_deref() == Some("chatgpt-codex-oauth-official-api")
    });
    let search = snapshot
        .document()
        .providers
        .iter()
        .find(|provider| provider.id == target.provider_id)
        .and_then(|provider| {
            super::search_model_probe::SearchProbeKind::from_preset(provider.preset.as_deref())
        });
    if codex && options.is_none() {
        return super::codex_model_probe::probe(state, target).await;
    }
    let gemini_web = target.payload.canonical_adapter() == "gemini_web_compatible";
    let accio = target.payload.canonical_adapter() == "accio_compatible";
    let kiro_key = crate::protocol::kiro_api_key::owns_payload(&target.payload)
        || crate::protocol::kiro_oauth::runtime::owns_payload(&target.payload);
    if accio && options.is_none() {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Accio requires an explicit model test; no discovery or generation was sent.",
            ),
        );
    }
    let chatgpt_web = target.payload.canonical_adapter() == "chatgpt_web_reverse_compatible";
    let antigravity = target.payload.adapter == crate::protocol::antigravity::ADAPTER;
    let perplexity_web =
        target.payload.adapter == crate::protocol::perplexity_web::material::ADAPTER;
    let grok_web = crate::upstream::grok_web::owns_payload(&target.payload);
    let qwen_web = crate::upstream::qwen::web_session::owns_payload(&target.payload);
    if qwen_web && options.is_none() {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Qwen Web requires an explicit single-turn model test; no generation was sent.",
            ),
        );
    }
    if grok_web && options.is_none() {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Grok Web requires an explicit single-turn model test; no generation was sent.",
            ),
        );
    }
    if perplexity_web && options.is_none() {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "Perplexity Web requires an explicit single-turn model test; no discovery or generation was sent."));
    }
    if !nvidia && !codex && !gemini_web && search.is_none() && options.is_none() {
        return (
            point,
            super::probe_provider_payload_for_console(
                state.upstream_client.client(),
                &target.payload,
            )
            .await,
        );
    }
    let cohere = target.payload.canonical_adapter() == "cohere_compatible";
    let anthropic = target.payload.canonical_adapter() == "anthropic_compatible";
    let bedrock = target.payload.canonical_adapter() == "bedrock_converse_compatible";
    let gemini = matches!(
        target.payload.canonical_adapter(),
        "gemini_api_compatible" | "gemini_api_modular_compatible"
    );
    if options.is_some()
        && !codex
        && !cohere
        && !anthropic
        && !bedrock
        && !gemini
        && !gemini_web
        && !chatgpt_web
        && !antigravity
        && !perplexity_web
        && !grok_web
        && !qwen_web
        && !accio
        && !kiro_key
        && media_product.is_none()
        && search.is_none()
        && (target.payload.canonical_adapter() != "openai_compatible"
            || target.payload.bridges_openai_text_endpoint_to_responses(
                crate::protocol::canonical::EndpointKind::ChatCompletions,
            ))
    {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "Prompt tests require an OpenAI-compatible chat endpoint or official Codex account; this adapter is not supported."));
    }
    let provider = snapshot
        .get_providers()
        .into_iter()
        .find(|p| p.id == target.provider_id);
    let Some(provider) = provider else {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Provider is no longer configured.",
            ),
        );
    };
    let credential_models = provider
        .credential_pool
        .iter()
        .find(|credential| credential.id == target.credential_id)
        .map(|credential| credential.supported_models.as_slice())
        .unwrap_or_default();
    let configured: Vec<_> = target
        .payload
        .default_model
        .as_deref()
        .into_iter()
        .chain(provider.supported_models.iter().map(String::as_str))
        .chain(credential_models.iter().map(String::as_str))
        .collect();
    let requested = options.and_then(|options| options.model.as_deref());
    if matches!(
        media_product,
        Some(
            super::media_probe_request::MediaProbeProduct::Chataibot
                | super::media_probe_request::MediaProbeProduct::Suno
                | super::media_probe_request::MediaProbeProduct::Udio
                | super::media_probe_request::MediaProbeProduct::LumaWeb
        )
    ) && requested.is_some_and(|model| {
        !configured.contains(&model)
            && !provider.model_map.contains_key(model)
            && !provider.model_map_targets.contains_key(model)
    }) {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "Media tests require an explicitly configured model or alias; generation was not sent."));
    }
    if strict_product
        && requested.is_some_and(|model| {
            !configured.contains(&model)
                && !provider.model_map.contains_key(model)
                && !provider.model_map_targets.contains_key(model)
        })
    {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "This product requires a configured model or alias; call not sent.",
            ),
        );
    }
    if qwen_web && requested.is_some_and(|model| !configured.contains(&model)) {
        return (
            point,
            report(
                ProviderPayloadProbeStatus::Unsupported,
                "Qwen Web model tests require a model configured for this account; call not sent.",
            ),
        );
    }
    if requested
        .is_some_and(|model| !crate::provider_model_id::valid_for_payload(model, &target.payload))
    {
        return (point, report(ProviderPayloadProbeStatus::Unsupported,
            "Model identity exceeds this product's limit or uses invalid syntax; call not sent."));
    }
    let models = requested.map_or(configured, |model| vec![model]);
    let model = models
        .into_iter()
        .find(|model| {
            crate::provider_model_id::valid_for_payload(model, &target.payload)
                && (credential_models.is_empty()
                    || credential_models.iter().any(|allowed| allowed == model)
                    || provider
                        .model_map
                        .get(*model)
                        .is_some_and(|mapped| credential_models.contains(mapped))
                    || provider
                        .model_map_targets
                        .get(*model)
                        .is_some_and(|targets| {
                            targets
                                .iter()
                                .any(|mapped| credential_models.contains(mapped))
                        }))
        })
        .and_then(|model| {
            crate::routing::config::model_for_probe(&provider, &target.credential_id, model)
        })
        .filter(|model| crate::provider_model_id::valid_for_payload(model, &target.payload));
    let Some(model) = model else {
        return ("Model call".into(), report(ProviderPayloadProbeStatus::Unsupported,
            "No configured model is callable by this credential; choose a supported model or configure default_model."));
    };
    if let Some(product) = media_product {
        return super::media_model_probe::probe(state, target, &model, options.unwrap(), product)
            .await;
    }
    if let Some(kind) = search {
        if !kind.supports_model(&model) {
            return (point, report(ProviderPayloadProbeStatus::Unsupported,
                "This model is not supported by the provider's search test; Contents requires a fetch request."));
        }
        // Only absent input gets an operation-specific default; never replace a user's target.
        let query = options.map_or(kind.default_prompt(&model), |options| {
            options.prompt.as_str()
        });
        return super::search_model_probe::probe(state, target, &model, query, kind).await;
    }
    let prompt = options.map_or("Reply with only OK.", |options| options.prompt.as_str());
    if crate::protocol::luma_inference::owns_payload(&target.payload) {
        return super::luma_inference_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::lobsterai::owns_payload(&target.payload) {
        return super::lobsterai_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::quick_oauth::owns_payload(&target.payload) {
        return super::quick_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::nous_portal::business::owns_payload(&target.payload) {
        return super::nous_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::dim_oauth::business::owns_payload(&target.payload) {
        return super::dim_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::sap_aicore::owns_payload(&target.payload) {
        return super::sap_aicore_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::commandcode::owns_payload(&target.payload) {
        return super::commandcode_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::sensenova::owns_payload(&target.payload) {
        return super::sensenova_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::agnes::owns_payload(&target.payload) {
        return super::agnes_model_probe::probe(state, target, &model, prompt).await;
    }
    if qwen_web {
        return super::qwen_web_model_probe::probe(state, target, &model, prompt).await;
    }
    if grok_web {
        return super::grok_web_model_probe::probe(state, target, &model, prompt).await;
    }
    if kiro_key {
        return super::kiro_api_key_model_probe::probe(state, target, &model, prompt).await;
    }
    if accio {
        return super::accio_model_probe::probe(state, target, &model, prompt).await;
    }
    if perplexity_web {
        return super::perplexity_web_model_probe::probe(state, target, &model, prompt).await;
    }
    if crate::protocol::qwen::workspace_api::owns_payload(&target.payload) {
        return super::qwen_model_probe::probe(state, target, &model, prompt).await;
    }
    if antigravity {
        return super::antigravity_model_probe::probe(state, target, &model, prompt).await;
    }
    if codex {
        return super::codex_model_probe::probe_with_prompt(state, target, &model, prompt).await;
    }
    if chatgpt_web {
        return super::chatgpt_web_model_probe::probe(state, target, &model, prompt).await;
    }
    if cohere {
        return super::cohere_model_probe::probe(state, target, &model, prompt).await;
    }
    if anthropic {
        return super::anthropic_model_probe::probe(state, target, &model, prompt).await;
    }
    if bedrock {
        return super::bedrock_model_probe::probe(state, target, &model, prompt).await;
    }
    if gemini || gemini_web {
        return super::gemini_model_probe::probe(state, target, &model, prompt).await;
    }
    let mut body = json!({"model": model, "stream": false, "max_tokens": 256,
        "messages": [{"role": "user", "content": prompt}]});
    if model.starts_with("nvidia/nemotron") {
        body["chat_template_kwargs"] = json!({"enable_thinking": false});
    }
    let plan = crate::protocol::openai::normalize_chat_completions(body).and_then(|request| {
        crate::upstream::openai_compatible_request_plan::build_request_plan(
            &target.payload,
            &request,
            &model,
            false,
        )
    });
    let Ok(plan) = plan else {
        return (
            "NVIDIA model call".into(),
            report(
                ProviderPayloadProbeStatus::Failed,
                "Cannot build NVIDIA model test request.",
            ),
        );
    };
    let point = format!(
        "POST {} (model: {model})",
        crate::console::secrets::redact_url_value(&plan.url)
    );
    let audit = super::model_probe_recording::begin(state, target, &model, false).await;
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        super::openai_model_probe_response::generate(state, target, &plan),
    )
    .await
    .unwrap_or_else(|_| ModelProbeResult {
        status: None,
        usage: Value::Null,
        reply: Err("Model call timed out after 60 seconds.".into()),
    });
    let health_recovery = super::model_probe_recording::finish(
        state,
        target,
        &model,
        audit,
        started.elapsed(),
        &result,
    )
    .await;
    let answer = result.reply.as_ref().ok().cloned();
    let (status, message) = match result.reply {
        Ok(reply) => (
            ProviderPayloadProbeStatus::Passed,
            format!(
                "Model call passed. Model: {model}. Reply: {}",
                reply.chars().take(320).collect::<String>()
            ),
        ),
        Err(error) => (
            ProviderPayloadProbeStatus::Failed,
            format!("Model call failed. Model: {model}. {error}"),
        ),
    };
    (
        point,
        ProviderPayloadProbeReport {
            health_recovery: Some(health_recovery),
            status,
            message,
            answer,
        },
    )
}

fn report(status: ProviderPayloadProbeStatus, message: &str) -> ProviderPayloadProbeReport {
    ProviderPayloadProbeReport {
        health_recovery: None,
        answer: None,
        status,
        message: message.into(),
    }
}
