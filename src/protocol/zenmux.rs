//! Fixed destination of the official Key product; generic custom endpoints use another preset.
pub const BASE: &str = "https://zenmux.ai/api/v1";
pub const SOURCE: &str = "https://zenmux.ai/api/v1/models";

pub fn owns_payload(payload: &crate::routing::candidate::ProviderAccountPayload) -> bool {
    payload.adapter == "openai_compatible" && payload.base_url.trim_end_matches('/') == BASE
}

pub fn validate(
    preset: Option<&str>,
    payload: &crate::routing::candidate::ProviderAccountPayload,
) -> anyhow::Result<()> {
    if preset == Some("zenmux-api") {
        anyhow::ensure!(
            owns_payload(payload)
                && payload.auth_mode.as_deref() == Some("bearer")
                && payload.chat_completions_path.as_deref() == Some("/chat/completions")
                && payload.responses_path.is_none()
                && payload.session_auth.is_none(),
            "ZenMux requires the official Chat Completions endpoint and Bearer API Key."
        );
    }
    Ok(())
}
