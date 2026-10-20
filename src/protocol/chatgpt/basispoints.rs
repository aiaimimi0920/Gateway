//! Explicit alternate product transport; the original credential/refresh owner stays native.
pub mod request;
pub mod response;
mod schema;
mod tools;

use crate::routing::candidate::ProviderAccountPayload;

pub const ADAPTER: &str = "basispoints_compatible";
pub const URL: &str = "https://bps.openai.com/basispoints/api/responses";

pub(crate) fn configure(payload: &mut ProviderAccountPayload, enabled: bool) -> anyhow::Result<()> {
    if !enabled {
        anyhow::ensure!(
            payload.adapter != ADAPTER,
            "Select Basispoints with codex_basispoints_enabled on a native Codex provider."
        );
        return Ok(());
    }
    anyhow::ensure!(
        cfg!(feature = "line-chatgpt-codex-oauth-official"),
        "Basispoints requires the Codex OAuth protocol feature."
    );
    let url = url::Url::parse(&payload.base_url)?;
    anyhow::ensure!(
        payload.adapter == "openai_compatible"
            && url.scheme() == "https"
            && url.host_str() == Some("chatgpt.com")
            && url.path().trim_end_matches('/') == "/backend-api/codex"
            && url.username().is_empty() && url.password().is_none()
            && url.query().is_none() && url.fragment().is_none()
            && payload.discovered_protocols.is_empty(),
        "Basispoints requires a native Codex credential, not a discovered or arbitrary API endpoint."
    );
    payload.adapter = ADAPTER.into();
    Ok(())
}

pub(crate) fn account_id(payload: &ProviderAccountPayload) -> Option<&str> {
    payload.headers.iter().find_map(|(name, value)| {
        (name.eq_ignore_ascii_case("chatgpt-account-id") && !value.trim().is_empty())
            .then_some(value.as_str())
    })
}
