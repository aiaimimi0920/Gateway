//! Existing Gemini capture completion: preserve its authentication contracts and chosen account label.
use super::*;

impl GeminiAuthFamily {
    pub fn matches_provider_preset(self, preset: &str) -> bool {
        match self {
            Self::GeminiWeb => matches!(preset, "gemini-web-chat" | "gemini-web-chat-modular"),
            Self::GeminiCanvas => matches!(
                preset,
                "gemini-canvas" | "gemini-canvas-browser-relay" | "gemini-canvas-program-relay"
            ),
            Self::GeminiCanvasChat => preset == "gemini-canvas-chat",
            Self::GeminiBusiness => preset == "gemini-business",
            Self::AistudioWeb => preset == "aistudio-web-reverse",
            Self::FlowmusicWeb => preset == "producer",
            Self::FlowWeb => preset == "flow-web",
            Self::StitchWeb => preset == "stitch-web",
        }
    }
}

pub(super) fn apply_account_label(
    drafts: &mut [GeminiGeneratedCredentialDraft],
    label: Option<&str>,
) {
    if let Some(label) = label.map(str::trim).filter(|label| !label.is_empty()) {
        for draft in drafts {
            draft.credential["account_name"] = Value::String(label.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authentication_family_must_match_persisted_provider_preset() {
        assert!(GeminiAuthFamily::AistudioWeb.matches_provider_preset("aistudio-web-reverse"));
        assert!(!GeminiAuthFamily::AistudioWeb.matches_provider_preset("gemini-web-chat"));
        assert!(!GeminiAuthFamily::GeminiWeb.matches_provider_preset("aistudio-web-reverse"));
        assert!(
            GeminiAuthFamily::GeminiCanvas.matches_provider_preset("gemini-canvas-browser-relay")
        );
    }

    #[test]
    fn account_label_is_applied_without_changing_secret_edits() {
        let mut drafts = vec![GeminiGeneratedCredentialDraft {
            provider_id: "selected".into(),
            credential: serde_json::json!({"id":"unchanged"}),
            secret_edits: Vec::new(),
        }];
        apply_account_label(&mut drafts, Some("  手动账号  "));
        assert_eq!(drafts[0].credential["account_name"], "手动账号");
        assert_eq!(drafts[0].credential["id"], "unchanged");
        assert!(drafts[0].secret_edits.is_empty());
    }
}

impl GeminiAuthSessionManager {
    pub(super) async fn run_business_capture_session(
        self: Arc<Self>,
        session_id: String,
        provider_id: String,
        account_label: Option<String>,
    ) {
        match run_gemini_business_capture_worker(provider_id.as_str()).await {
            Ok(output) => {
                let mut generated_drafts = build_business_generated_drafts(
                    output
                        .jwt
                        .as_deref()
                        .expect("validated Gemini Business jwt"),
                    output
                        .config_id
                        .as_deref()
                        .expect("validated Gemini Business configId"),
                    output
                        .session
                        .as_deref()
                        .expect("validated Gemini Business session"),
                );
                apply_account_label(&mut generated_drafts, account_label.as_deref());
                self.update_session(
                    &session_id,
                    GeminiAuthSessionStatus::Succeeded,
                    "Gemini Business runtime captured.".to_string(),
                    generated_drafts,
                );
            }
            Err(message) => self.update_session(
                &session_id,
                GeminiAuthSessionStatus::Failed,
                message,
                Vec::new(),
            ),
        }
    }

    pub(super) async fn run_web_capture_session(
        self: Arc<Self>,
        session_id: String,
        provider_id: String,
        account_label: Option<String>,
    ) {
        match run_gemini_web_capture_worker(provider_id.as_str()).await {
            Ok(output) => {
                let mut generated_drafts = build_web_generated_drafts(
                    provider_id.as_str(),
                    output
                        .api_key
                        .as_deref()
                        .expect("validated Gemini Web primary cookie"),
                    output.auth_token.as_deref(),
                    &output,
                );
                apply_account_label(&mut generated_drafts, account_label.as_deref());
                self.update_session(
                    &session_id,
                    GeminiAuthSessionStatus::Succeeded,
                    "Gemini Web HTTP runtime captured.".to_string(),
                    generated_drafts,
                );
            }
            Err(message) => self.update_session(
                &session_id,
                GeminiAuthSessionStatus::Failed,
                message,
                Vec::new(),
            ),
        }
    }
}
