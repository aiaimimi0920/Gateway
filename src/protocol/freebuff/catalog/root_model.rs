//! Same-row root inputs and compiled-model hints, NOT a prompt, harness, auth grant or routing fallback.
//! Fixed public catalog/root contract and Apache-2.0 source attribution are recorded in REFERENCE.md.
use super::{RootCompactionPolicy, SelectedModel};

pub(crate) struct RootModelInput<'a> {
    pub(crate) model: &'a str,
    pub(crate) model_label: &'a str,
    pub(crate) compaction: Option<&'a RootCompactionPolicy>,
    pub(crate) compiled_model_id: Option<&'static str>,
    pub(crate) turn_ending_note: bool,
    pub(crate) batching: bool,
}
impl<'a> SelectedModel<'a> {
    pub(crate) fn root_input(&self) -> RootModelInput<'a> {
        // Reference order matters: first recognized digest in the ROW, not first matching flag.
        // An unknown digest is not an error and must never be inferred from a label or handle.
        let compiled_model_id = self.legacy_digests.iter().find_map(|digest| {
            COMPILED_MODELS
                .iter()
                .find(|(known, _)| digest.as_str() == *known)
                .map(|(_, id)| *id)
        });
        let lean = self
            .compaction
            .is_some_and(|policy| policy.lean == Some(true));
        RootModelInput {
            model: self.handle,
            model_label: self.display_name,
            compaction: self.compaction,
            compiled_model_id,
            turn_ending_note: matches!(
                compiled_model_id,
                Some(
                    "meta/muse-spark-1.3-contributor"
                        | "upstage/solar-pro4"
                        | "upstage/solar-mini4"
                )
            ),
            batching: !lean
                && matches!(
                    compiled_model_id,
                    Some("deepseek/deepseek-v4-flash" | "deepseek/deepseek-v4-flash-fast")
                ),
        }
    }
}
impl std::fmt::Debug for RootModelInput<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffRootModelInput { catalog: <redacted> }")
    }
}
// Exactly the fixed reference's 21 SUPPORTED_FREEBUFF_MODELS, not current entitlement or model names
// to send. Digests are public non-cryptographic legacy identity hints; they cannot authorize anything.
// Static pairs avoid hashing/allocation per selection; lookup is bounded by 64 row digests x 21 pairs.
const COMPILED_MODELS: &[(&str, &str)] = &[
    ("0debeff6b9aed083", "stealth/ox-alpha"),
    ("b6c8bfa2f7ffab8f", "stealth/space-bunny-alpha"),
    ("03eb93cc2ce021a9", "deepseek/deepseek-v4-pro"),
    ("5ca70eae3ec024c9", "minimax/minimax-m3"),
    ("cb77b077909430a6", "openai/gpt-5.6-luna"),
    ("6fe0bf9071b3f925", "openai/gpt-6-luna"),
    ("7d047539bb50f128", "openai/gpt-6.1-sol"),
    ("62b1176d144cf710", "openai/gpt-6.1-terra"),
    ("cd8249565a5b0303", "google/gemini-4-argon"),
    ("d0c9e1b3ffecc344", "upstage/solar-pro4"),
    ("957291675c9f9132", "upstage/solar-mini4"),
    ("59ce6b7e3475ef0f", "google/gemini-3.8-flash"),
    ("21dd0695d74d9a00", "meta/muse-spark-1.3-contributor"),
    ("5ef411d6e9c19a25", "meta/muse-spark-1.2-contributor"),
    ("348303247ca1e06f", "z-ai/glm-5.2"),
    ("197cbcdc4b67262b", "z-ai/glm-5.3-flash"),
    ("1e303ac563a6f9cc", "deepseek/deepseek-v4-flash"),
    ("1e775c266d0065f1", "deepseek/deepseek-v4-flash-fast"),
    ("5acfab992d88345c", "mimo/mimo-v2.5"),
    ("e95ba88e0d8e5c57", "mimo/mimo-v2.6-pro"),
    ("fa7c88d7a2a873be", "anthropic/claude-fable-5.1"),
];
#[cfg(test)]
#[path = "root_model_tests.rs"]
mod tests;
