//! Dedicated remote lane; never enter ordinary provider retries or paid fallback.
use super::*;
use crate::protocol::canonical::CanonicalRelayResponse;
use crate::{
    pipeline::cursor_remote_identity::Identity, protocol::cursor_remote, routing::config::cursor,
};

pub(super) async fn try_send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Option<Result<PipelineOutput, GatewayError>> {
    if !cursor::private_request(ctx.canonical_req.requested_model.as_deref())
        || !ctx
            .candidates
            .iter()
            .any(|candidate| candidate.payload.auth_mode.as_deref() == Some("cursor_remote"))
    {
        return None;
    }
    Some(send(ctx, state).await.map_err(cursor::stop))
}

async fn send(
    ctx: &mut PipelineContext,
    state: &Arc<AppState>,
) -> Result<PipelineOutput, GatewayError> {
    if ctx.canonical_req.protocol_family
        == crate::protocol::canonical::ProtocolFamily::BedrockConverse
        && ctx
            .request_headers
            .keys()
            .any(|key| key.starts_with("x-amzn-bedrock-"))
    {
        return Err(GatewayError::bad_request(
            "Cursor remote cannot enforce Bedrock-specific headers",
        )
        .with_code("cursor_remote_unsupported_request"));
    }
    let identity = Arc::new(Identity::bind(ctx, state).await?);
    let [candidate] = ctx.candidates.as_slice() else {
        return Err(cursor_remote::error("binding_changed"));
    };
    if candidate.provider_account_id != identity.provider
        || candidate.payload.credential_id.as_deref() != Some(identity.credential.as_str())
        || candidate.payload.auth_mode.as_deref() != Some("cursor_remote")
        || candidate.payload.auth_token.is_some()
    {
        return Err(cursor_remote::error("binding_changed"));
    }
    let candidate = candidate.clone();
    if !ctx.provider_allowed_for_send(&candidate.provider_account_id) {
        return Err(cursor_remote::error("provider_denied"));
    }
    let execution = cursor_remote::ingress::parse_with_continuation(
        &ctx.canonical_req,
        ctx.cursor_remote_continuation.as_ref(),
    )?;
    if !matches!(&execution, cursor_remote::ingress::Execution::Text(_))
        && ctx.cursor_tool_publication.is_none()
    {
        return Err(cursor_remote::error("tool_transport_unsupported"));
    }
    let mut input = identity.input(execution.prompt())?;
    let binding = identity.binding();
    let deadline = ctx
        .request_budget
        .admission_deadline()
        .ok_or_else(|| cursor_remote::error("deadline"))?;
    let gate = stage_rate_limit::provider_attempt_gate(ctx, state, &candidate.provider_account_id)?
        .with_pinned(&identity.provider, &identity.credential, cursor::MODEL);
    recovery_wait::cancellable(
        ctx.caller_cancel.clone(),
        ctx.request_budget.clone(),
        state,
        gate.admit(),
    )
    .await??;
    let reserve = gate.clone();
    let record = gate.clone();
    let authorize = Box::new(move || {
        Box::pin(async move {
            identity.authorize().await?;
            reserve.cursor_remote_reserve()
        }) as futures::future::BoxFuture<'static, Result<(), GatewayError>>
    });
    let started_at = Instant::now();
    ctx.observed_upstream_status = Some(None);
    let (settled, receiver) = tokio::sync::watch::channel(false);
    ctx.cursor_remote_settled = Some(receiver);
    let uncertain = Arc::new(std::sync::atomic::AtomicBool::new(false));
    ctx.cursor_remote_uncertain = Some(uncertain.clone());
    let work = async {
        // Full image decoding runs once, before a new worker or remote Run, inside
        // the same cancellation/deadline owner as execution. No URL is fetched.
        execution.prepare_input(&mut input).await?;
        if let Some(request) = execution.into_tools() {
            state
                .lifecycle
                .cursor_remote
                .execute_tools(
                    binding,
                    request,
                    input,
                    deadline,
                    authorize,
                    Box::new(move || record.cursor_remote_sent()),
                    settled,
                    uncertain,
                )
                .await
        } else {
            state
                .lifecycle
                .cursor_remote
                .execute(
                    input,
                    deadline,
                    authorize,
                    Box::new(move || record.cursor_remote_sent()),
                    settled,
                    uncertain,
                )
                .await
        }
    };
    let result = recovery_wait::cancellable(
        ctx.caller_cancel.clone(),
        ctx.request_budget.clone(),
        state,
        work,
    )
    .await?;
    let mut completed = match result {
        Ok(value) => value,
        Err(error) => {
            gate.finish_error(&error);
            return Err(error);
        }
    };
    if let Some(publication) = completed.publication.take() {
        ctx.cursor_tool_publication
            .take()
            .ok_or_else(|| cursor_remote::error("tool_transport_unsupported"))?
            .send(publication)
            .map_err(|_| cursor_remote::error("cancelled"))?;
    }
    set_selected_candidate(ctx, &candidate, None, &completed.selector, false);
    ctx.selected_execution_mode = Some("cursor_remote".into());
    let semantics = if completed.call.is_some() {
        "tool_calls"
    } else {
        "stop"
    };
    ctx.observed_upstream_status = Some(Some(200));
    // Selector != actual model. Only native telemetry, never a Free billing receipt.
    ctx.observed_usage = Some(completed.usage.clone());
    let canonical = CanonicalRelayResponse {
        model: completed.selector.clone(),
        text: completed.text,
        bedrock_turn: None,
        vertex_turn: None,
        model_declaration: None,
        sources: vec![],
        usage: completed.usage,
        tool_calls: completed
            .call
            .into_iter()
            .map(cursor_remote::runtime::ToolCall::into_canonical)
            .collect(),
        upstream_status: Some(200),
        finish_reason: Some(semantics.into()),
    };
    let metered = cursor_remote::metered_output::supports(ctx.canonical_req.protocol_family);
    let native = if metered {
        Some(
            cursor_remote::metered_output::response(
                &ctx.canonical_req,
                &canonical,
                started_at.elapsed(),
            )
            .map_err(|error| {
                gate.finish_error(&error);
                error
            })?,
        )
    } else {
        cursor_remote::native_output::supports(ctx.canonical_req.protocol_family)
            .then(|| {
                cursor_remote::native_output::response(
                    ctx.canonical_req.protocol_family,
                    &canonical,
                )
            })
            .transpose()
            .map_err(|error| {
                gate.finish_error(&error);
                error
            })?
    };
    if !ctx.canonical_req.stream {
        let response = if let Some(value) = native {
            value
        } else {
            let mut value = pack_response(&ctx.canonical_req, &canonical);
            // Cursor cache counters are independent native telemetry, not a
            // proven subset of OpenAI input tokens. Keep them only in the audit.
            if let Some(usage) = value
                .get_mut("usage")
                .and_then(serde_json::Value::as_object_mut)
            {
                usage.remove("prompt_tokens_details");
                usage.remove("input_tokens_details");
            }
            if let Some(thinking) = &completed.thinking {
                thinking.project(&mut value, ctx.canonical_req.endpoint_kind);
            }
            value
        };
        ctx.canonical_completion_semantics = Some(semantics.into());
        gate.finish_success();
        return Ok(PipelineOutput::Json(response));
    }
    // Buffer each phase until native tool handoff, or verified TurnEnded + EndStream.
    let chat = ctx.canonical_req.endpoint_kind
        == crate::protocol::canonical::EndpointKind::ChatCompletions;
    let frames = if let Some(value) = &native {
        if metered {
            cursor_remote::metered_output::frames(ctx.canonical_req.protocol_family, value)
                .map_err(|error| {
                    gate.finish_error(&error);
                    error
                })?
        } else {
            cursor_remote::native_output::frames(ctx.canonical_req.protocol_family, value)
        }
    } else {
        cursor_remote::sse::frames(
            cursor::MODEL,
            &canonical,
            if chat {
                completed.thinking.as_ref()
            } else {
                None
            },
        )
    };
    let openai = futures::stream::iter(frames.into_iter().map(Ok::<_, StreamError<rquest::Error>>));
    let stream: std::pin::Pin<
        Box<dyn futures::Stream<Item = Result<bytes::Bytes, StreamError<rquest::Error>>> + Send>,
    > = match ctx.canonical_req.endpoint_kind {
        _ if native.is_some() || chat => Box::pin(openai),
        _ => Box::pin(
            crate::protocol::responses::translate_cursor_sse_to_responses(
                openai,
                cursor::MODEL.into(),
                completed.thinking,
            ),
        ),
    };
    ctx.canonical_completion_semantics = Some(semantics.into());
    Ok(PipelineOutput::Sse(streaming::terminal::track(
        ctx,
        state,
        &candidate,
        streaming::terminal::Owner {
            stream,
            permit: None,
            usage: canonical
                .usage
                .map(|usage| Arc::new(std::sync::Mutex::new(Some(usage)))),
            native_semantics: Some(Arc::new(std::sync::Mutex::new(Some(semantics.into())))),
            model: completed.selector,
            route_policy_config: ctx.route_policy_config.clone(),
            controller: state
                .concurrency_registry
                .get_or_create(&candidate.provider_account_id),
            gate,
            started_at,
            legacy_lease: None,
            terminal: None,
        },
    )))
}
