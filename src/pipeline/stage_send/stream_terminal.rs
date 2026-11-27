//! The sole public stream success/failure owner, shared by ordinary and cooperative catalog streams.
use super::*;
#[path = "stream_diagnostics.rs"]
mod diagnostics;
pub(in super::super) struct Owner {
    pub stream: ByteStream,
    pub permit: Option<crate::concurrency::aimd::ConcurrencyPermit>,
    pub usage: Option<UsageHandle>,
    pub native_semantics: Option<Arc<std::sync::Mutex<Option<String>>>>,
    pub model: String,
    pub route_policy_config: Option<crate::db::GatewayRoutePolicyConfig>,
    pub controller: Arc<crate::concurrency::aimd::AimdController>,
    pub gate: stage_rate_limit::ProviderAttemptGate,
    pub started_at: Instant,
    pub legacy_lease: Option<freebuff::FreeBuffLeaseHandle>,
    pub terminal: Option<Arc<std::sync::atomic::AtomicBool>>,
}
pub(in super::super) fn track(
    ctx: &PipelineContext,
    state: &Arc<AppState>,
    candidate: &crate::routing::candidate::RouteCandidate,
    owner: Owner,
) -> TrackedStream<StreamError<rquest::Error>> {
    let Owner {
        stream: byte_stream,
        permit,
        usage: stream_usage_handle,
        native_semantics,
        model,
        route_policy_config,
        controller,
        gate: terminal_gate,
        started_at: stream_started_at,
        legacy_lease: freebuff_lease_handle,
        terminal,
    } = owner;
    let native_cursor_bedrock = ctx.selected_execution_mode.as_deref() == Some("cursor_remote")
        && ctx.canonical_req.protocol_family
            == crate::protocol::canonical::ProtocolFamily::BedrockConverse;
    let sap = crate::protocol::sap_aicore::owns_payload(&candidate.payload);
    let (byte_stream, stream_completion_semantics_handle) = if native_cursor_bedrock {
        (byte_stream, None)
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Messages
        && (crate::protocol::commandcode::owns_payload(&candidate.payload)
            || crate::protocol::sensenova::owns_payload(&candidate.payload))
    {
        let (stream, handle) =
            crate::upstream::stream::tap_messages_completion_with_error(byte_stream);
        (Box::pin(stream) as ByteStream, Some(handle))
    } else if let Some(observe) =
        responses_wire::observer(&candidate.payload, ctx.canonical_req.endpoint_kind)
    {
        let (stream, handle) = crate::upstream::stream::tap_native_responses_completion_with_error(
            byte_stream,
            observe,
        );
        (Box::pin(stream) as ByteStream, Some(handle))
    } else if is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
        let (tapped_stream, completion_semantics_handle) =
            tap_sse_completion_semantics_with_error(byte_stream);
        (
            Box::pin(tapped_stream) as ByteStream,
            Some(completion_semantics_handle),
        )
    } else {
        (byte_stream, None)
    };

    // A strict cooperative owner already knows the original finish reason. Do not infer it from a
    // lossy public status (e.g. both length and content_filter become Responses incomplete).
    let stream_completion_semantics_handle =
        native_semantics.or(stream_completion_semantics_handle);
    let (byte_stream, stream_archive_handle) =
        if !native_cursor_bedrock && is_conversation_endpoint(ctx.canonical_req.endpoint_kind) {
            let (tapped_stream, archive_handle) = tap_stream_archive_with_error(
                byte_stream,
                crate::conversation_archive::CONVERSATION_ARCHIVE_MAX_RESPONSE_BYTES,
            );
            (Box::pin(tapped_stream) as ByteStream, Some(archive_handle))
        } else {
            (byte_stream, None)
        };

    // Clone the controller Arc so the callback can notify on
    // stream completion.
    let ctrl_for_cb = Arc::clone(&controller);
    let provider_id = candidate.provider_account_id.clone();
    let state_for_cb = Arc::clone(state);
    let route_policy_for_cb = route_policy_config.clone();
    let resolved_model_for_cb = model.clone();
    let audit_snapshot = crate::pipeline::stage_finalize::snapshot_request_audit(ctx);
    let failure_finalize_snapshot = crate::pipeline::stage_finalize::FailureFinalizeSnapshot {
        request_id: ctx.req_id.to_string(),
        credential_id: ctx
            .quota_credential_id
            .clone()
            .or_else(|| ctx.credential_ref.clone()),
        project_id: ctx
            .session
            .as_ref()
            .map(|session| session.project_id.clone()),
        user_id: ctx.neuro_user_id.clone().or_else(|| {
            ctx.session
                .as_ref()
                .and_then(|session| session.user_id.clone())
        }),
        model: Some(model.clone()),
        provider: Some(candidate.adapter.clone()),
        started_at: ctx.started_at,
        pre_deducted_tokens: ctx.quota_pre_deducted_tokens,
        request_audit: audit_snapshot.clone(),
    };

    let provider_credential_id_for_cb = candidate.provider_credential_id.clone();
    let budget_for_cb = ctx.request_budget.clone();
    let latency_key =
        crate::routing::latency::LatencyKey::for_request(candidate, &ctx.canonical_req);
    let (byte_stream, failure_evidence) = if native_cursor_bedrock {
        (byte_stream, Arc::new(parking_lot::Mutex::new(None)))
    } else {
        diagnostics::observe(byte_stream)
    };
    // SSE taps/archive precede encoding; native EOF validation must precede the sole success owner.
    let (byte_stream, stream_usage_handle): (ByteStream, _) = if ctx.canonical_req.protocol_family
        == crate::protocol::canonical::ProtocolFamily::BedrockConverse
        && !native_cursor_bedrock
    {
        let (stream, usage) =
            crate::protocol::bedrock_converse::encode_native_stream_with_started_at(
                byte_stream,
                model.clone(),
                ctx.started_at,
            );
        (Box::pin(stream), Some(usage))
    } else {
        (byte_stream, stream_usage_handle)
    };
    // Only the admitted cooperative source owns its deadline/FINISH after handoff.
    let budgeted = if terminal.is_some() {
        byte_stream
    } else {
        Box::pin(ctx.request_budget.constrain_stream(byte_stream)) as ByteStream
    };
    let observed_stream = diagnostics::observe_errors(budgeted, Arc::clone(&failure_evidence));
    let mut tracked = TrackedStream::new_with_started_at_and_error(
        observed_stream,
        stream_started_at,
        move |metrics, success| {
            drop(permit);
            let failure = failure_evidence.lock().clone();
            let evidenced_failure = failure.is_some();
            let success = success && !evidenced_failure;
            if success
                && metrics.termination
                    == Some(crate::upstream::stream::StreamTermination::Completed)
            {
                if let (Some(key), Some(ms)) = (latency_key, metrics.first_token_latency_ms) {
                    state_for_cb
                        .routing_latency
                        .record(key, std::time::Duration::from_millis(ms));
                }
            }
            observe_provider_stream_metric(
                global_gateway_metrics(),
                &provider_id,
                &resolved_model_for_cb,
                &metrics,
                success,
            );
            if let Some(lease_handle) = freebuff_lease_handle {
                tokio::spawn(async move {
                    if success {
                        lease_handle.release().await;
                    } else {
                        lease_handle
                            .invalidate("freebuff stream terminated before completion")
                            .await;
                    }
                });
            }
            if success {
                terminal_gate.finish_success();
                state_for_cb.lifecycle.smart_routing.outcome(
                    &provider_id,
                    audit_snapshot
                        .requested_model
                        .as_deref()
                        .unwrap_or_default(),
                    true,
                );
                ctrl_for_cb.on_success();
                spawn_record_provider_success(
                    Arc::clone(&state_for_cb),
                    provider_id.clone(),
                    provider_credential_id_for_cb.clone(),
                );
                let state_for_audit = Arc::clone(&state_for_cb);
                let audit_snapshot = audit_snapshot.clone();
                let usage = stream_usage_handle.as_ref().and_then(snapshot_tapped_usage);
                let canonical_completion_semantics = stream_completion_semantics_handle
                    .as_ref()
                    .and_then(snapshot_tapped_completion_semantics);
                let archive_snapshot = stream_archive_handle.as_ref().map(snapshot_tapped_archive);
                let finalizer = state_for_audit
                    .local_runtime
                    .as_ref()
                    .map(|local| local.track_finalizer());
                tokio::spawn(async move {
                    let _finalizer = finalizer;
                    crate::pipeline::stage_finalize::run_stream_success(
                        audit_snapshot,
                        usage,
                        canonical_completion_semantics,
                        archive_snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.text.clone()),
                        archive_snapshot
                            .as_ref()
                            .map(|snapshot| snapshot.truncated)
                            .unwrap_or(false),
                        &state_for_audit,
                    )
                    .await;
                });
            } else {
                let upstream_failed = budget_for_cb.stopped_reason().is_none()
                    && (evidenced_failure
                        || metrics.termination
                            == Some(crate::upstream::stream::StreamTermination::Interrupted));
                let cause = crate::pipeline::stage_finalize::StreamFailureCause::from_observation(
                    budget_for_cb.stopped_reason(),
                    upstream_failed,
                );
                let classified = failure
                    .as_ref()
                    .and_then(|failure| failure.classified.as_ref());
                let sap_indeterminate =
                    sap && classified.is_none_or(crate::protocol::sap_aicore::errors::neutral);
                let failure_message = if sap_indeterminate {
                    crate::protocol::sap_aicore::errors::STREAM_UNVERIFIED.to_owned()
                } else {
                    failure
                        .as_ref()
                        .map(|failure| failure.message.clone())
                        .unwrap_or_else(|| {
                            format!(
                                "stream terminated before completion ({:?}); termination_reason={}",
                                metrics.termination,
                                cause.label()
                            )
                        })
                };
                warn!(req_id = %failure_finalize_snapshot.request_id,
                provider = %provider_id, error = %failure_message,
                chunks = metrics.chunk_count, bytes = metrics.total_bytes,
                duration_ms = metrics.total_duration_ms, "stream failed after handoff");
                // A local deadline or caller cancellation still finalizes/refunds,
                // but must not poison provider health or reduce its capacity.
                if upstream_failed
                    && !sap_indeterminate
                    && !classified.is_some_and(|error| {
                        error.code.as_deref() == Some("freebuff_business_run_journal_unavailable")
                    })
                {
                    if let Some(error) = classified {
                        terminal_gate.finish_error(error);
                    } else {
                        terminal_gate.finish_stream_failure();
                    }
                    terminal_gate.release_failed_sticky();
                    state_for_cb.lifecycle.smart_routing.outcome(
                        &provider_id,
                        audit_snapshot
                            .requested_model
                            .as_deref()
                            .unwrap_or_default(),
                        false,
                    );
                    ctrl_for_cb
                        .on_failure(classified.map_or(FailureKind::General, classify_failure_kind));
                    spawn_record_provider_failure(
                        audit_snapshot.started_at,
                        Arc::clone(&state_for_cb),
                        provider_id.clone(),
                        provider_credential_id_for_cb.clone(),
                        route_policy_for_cb.clone(),
                        failure_message.clone(),
                    );
                }
                terminal_gate.finish_neutral();
                let state_for_finalize = Arc::clone(&state_for_cb);
                let finalizer = state_for_finalize
                    .local_runtime
                    .as_ref()
                    .map(|local| local.track_finalizer());
                tokio::spawn(async move {
                    let _finalizer = finalizer;
                    crate::pipeline::stage_finalize::run_stream_failure(
                        failure_finalize_snapshot,
                        &state_for_finalize,
                        &failure_message,
                        cause,
                    )
                    .await;
                });
            }
            tracing::debug!(
                provider = %provider_id,
                chunks = metrics.chunk_count,
                bytes = metrics.total_bytes,
                duration_ms = metrics.total_duration_ms,
                success,
                "stream completed"
            );
        },
    );

    if let Some(terminal) = terminal {
        tracked = tracked.with_validated_owner_terminal(terminal);
    } else if ctx.canonical_req.endpoint_kind == EndpointKind::Responses {
        tracked = tracked.with_responses_terminal();
    }
    tracked
}
