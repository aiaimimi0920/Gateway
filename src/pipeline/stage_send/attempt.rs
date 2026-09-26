//! Values already prepared and admitted for exactly one candidate attempt.
use super::*;

pub(super) type ByteStream =
    std::pin::Pin<Box<dyn futures::Stream<Item = Result<Bytes, rquest::Error>> + Send>>;
pub(super) type UsageHandle = Arc<std::sync::Mutex<Option<crate::protocol::canonical::TokenUsage>>>;

// Preserve the outer loop's error precedence without repeating feedback or admission.
pub(super) enum AttemptError {
    Next(GatewayError),
    Stop(GatewayError),
}

pub(super) struct PreparedAttempt {
    pub(super) route_policy_config: Option<crate::db::GatewayRoutePolicyConfig>,
    pub(super) provider_attempt_gate: super::super::stage_rate_limit::ProviderAttemptGate,
    pub(super) model: String,
    pub(super) reply_model: String,
    pub(super) effective_payload: crate::routing::candidate::ProviderAccountPayload,
    pub(super) retry_policy: RetryPolicy,
    pub(super) original_tools: Vec<crate::protocol::canonical::CanonicalTool>,
    pub(super) original_tool_choice: Option<serde_json::Value>,
    pub(super) original_messages_text: String,
    pub(super) tools_were_injected: bool,
    pub(super) controller: Arc<crate::concurrency::aimd::AimdController>,
    pub(super) permit: crate::concurrency::aimd::ConcurrencyPermit,
}
