//! Parsing remains available for compatibility; a compiled-out Kiro line never sends OAuth grants.
use super::{Directory, OAuthError, Reply};
pub(super) async fn post(
    _: &rquest::Client,
    _: &Directory,
    _: &'static str,
    _: serde_json::Value,
    _: std::time::Duration,
) -> Result<Reply, OAuthError> {
    Err(OAuthError::FeatureDisabled)
}
