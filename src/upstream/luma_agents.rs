//! Official Agents API owner; never dispatches the existing Luma Web board worker.
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod assets;
pub(crate) mod files;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod files_output;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod files_upload;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod generation;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod layering_output;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod layering_text;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod settlement;
pub(crate) mod task_lookup;
#[cfg(feature = "line-lumalabs-agents-official-api")]
mod transport;
use crate::{
    error::GatewayError, protocol::canonical::CanonicalRelayRequest,
    routing::candidate::ProviderAccountPayload,
};
use serde_json::Value;
use std::time::Duration;

pub(crate) async fn execute(
    client: &rquest::Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request: &CanonicalRelayRequest,
    model: &str,
) -> Result<Value, GatewayError> {
    #[cfg(not(feature = "line-lumalabs-agents-official-api"))]
    {
        let _ = (client, timeout, payload, request, model);
        Err(crate::protocol::luma_agents::failure(
            crate::error::ErrorKind::ServiceUnavailable,
            503,
            "implementation_line_disabled",
            "Luma Agents 官方 API 执行线未编译。",
        ))
    }
    #[cfg(feature = "line-lumalabs-agents-official-api")]
    {
        let material = transport::Material::new(payload)?;
        let request = crate::protocol::luma_agents::GenerationRequest::pack(request, model)?;
        let max = if request.media == crate::protocol::luma_agents::Media::Image
            && request.kind != "layering"
        {
            120
        } else {
            600
        };
        let duration = timeout.min(Duration::from_secs(max));
        if duration.is_zero() {
            return Err(transport::timeout_error());
        }
        // One deadline owns submit, every poll, DNS, download and validation; no detached I/O.
        // Heap-pin the media future so polling/settlement state does not enlarge every
        // enclosing dispatcher and management request frame on Windows' default stack.
        tokio::time::timeout(
            duration,
            Box::pin(generation::run(client, &material, request, duration)),
        )
        .await
        .map_err(|_| transport::timeout_error())?
    }
}

#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod boundary_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod cancellation_audit_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod failure_audit_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod failure_receipt_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod files_browser_host;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod files_management_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod files_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod files_upload_management_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod files_upload_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod layering_fixture;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod layering_flow_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod layering_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod observation_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod pipeline_admission;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod pipeline_failures;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
pub(crate) mod pipeline_fixture;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod pipeline_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod probe_browser_host;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod probe_failure_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod probe_management_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod probe_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod reframe_flow_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod settlement_lifetime_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod settlement_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod task_browser_host;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod task_lookup_tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
pub(crate) mod test_support;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod tests;
#[cfg(all(test, feature = "line-lumalabs-agents-official-api"))]
mod video_edit_flow_tests;
