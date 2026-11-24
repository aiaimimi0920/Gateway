//! SAP prepares before the real-send gate and never enters a retry loop.
use super::super::*;
use crate::pipeline::stage_rate_limit::ProviderAttemptGate;
use crate::protocol::{
    canonical::{CanonicalRelayRequest, CanonicalRelayResponse},
    sap_aicore::{self as sap, sender::Prepared},
};
use crate::routing::candidate::ProviderAccountPayload;

pub(in crate::pipeline::stage_send) async fn run<T, F, Fut>(
    state: &Arc<AppState>,
    gate: &ProviderAttemptGate,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    status: &mut Option<Option<u16>>,
    decode: F,
) -> Result<T, GatewayError>
where
    F: FnOnce(rquest::Response) -> Fut,
    Fut: std::future::Future<Output = Result<T, GatewayError>>,
{
    *status = Some(None);
    let prepared =
        Prepared::prepare(&state.upstream_client, payload, req, model, req.stream).await?;
    // OAuth may await. Reapply live eligibility and request budget, without
    // fetching another token after the generation attempt has been claimed.
    gate.recheck_send_eligibility().await?;
    gate.begin_attempt()?;
    let started = Instant::now();
    let cancellation = ProviderAttemptCancellation::new(
        global_gateway_metrics(),
        gate.provider_id(),
        model,
        started,
    );
    let result = match prepared.send().await {
        Ok((response, _)) => {
            *status = Some(Some(response.status().as_u16()));
            decode(response).await
        }
        Err(error) => {
            *status = Some(sap::errors::upstream_status(&error));
            Err(error)
        }
    };
    cancellation.disarm();
    gate.observe_result(&result);
    // Streaming metrics belong to preflight/terminal, not successful HTTP headers.
    if !req.stream || result.is_err() {
        observe_provider_attempt_metric(
            global_gateway_metrics().as_ref(),
            gate.provider_id(),
            model,
            RetryAttemptObservation {
                succeeded: result.is_ok(),
                latency_ms: started.elapsed().as_millis().min(u64::MAX as u128) as u64,
                error: result.as_ref().err(),
                is_retry: false,
            },
        );
    }
    result
}

pub(in crate::pipeline::stage_send) async fn json(
    state: &Arc<AppState>,
    gate: &ProviderAttemptGate,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    status: &mut Option<Option<u16>>,
) -> Result<serde_json::Value, GatewayError> {
    run(
        state,
        gate,
        payload,
        req,
        model,
        status,
        |response| async move {
            if sap::native_responses::selected(payload, req.endpoint_kind) {
                sap::native_responses::decode(response).await
            } else {
                sap::orchestration::decode_native(response, req).await
            }
        },
    )
    .await
}

pub(in crate::pipeline::stage_send) async fn canonical(
    state: &Arc<AppState>,
    gate: &ProviderAttemptGate,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    status: &mut Option<Option<u16>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    run(state, gate, payload, req, model, status, |response| {
        sap::runtime::decode(response, payload, req, model)
    })
    .await
}
