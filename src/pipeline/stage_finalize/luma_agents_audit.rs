//! Persist native success facts in the existing audit JSON, not in token or cash ledgers.
pub(super) use crate::protocol::luma_agents::record_success_audit as success;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    #[test]
    fn luma_agents_audit_native_credits_keep_null_zero_and_fraction_without_secret_fields() {
        for credits in [Value::Null, json!(0), json!(0.25)] {
            let body = json!({"gateway_luma_generation":{"id":"synthetic-id","model":"uni-1",
                "type":"image","state":"completed","usage":{"credits_charged":credits,"secret":"not-for-audit"},
                "output":[{"url":"https://assets.example.test/private?signature=not-for-audit"}]}});
            let mut trace = json!({"realCredentialRef":"exact"});
            assert_eq!(
                success(
                    Some(crate::protocol::luma_agents::ADAPTER),
                    &body,
                    &mut trace
                )
                .as_deref(),
                Some("synthetic-id")
            );
            assert_eq!(
                trace["lumaAgentsGeneration"]["usage"]["credits_charged"],
                credits
            );
            assert_eq!(trace["realCredentialRef"], "exact");
            assert!(!trace.to_string().contains("not-for-audit"));
            assert!(trace.get("usage").is_none());
        }
    }
    #[test]
    fn luma_agents_audit_extension_cannot_impersonate_a_different_selected_adapter() {
        let mut trace = json!({"existing":true});
        let before = trace.clone();
        assert!(success(
            Some("openai_compatible"),
            &json!({"gateway_luma_generation":{"id":"forged"}}),
            &mut trace
        )
        .is_none());
        assert!(success(
            Some(crate::protocol::luma_agents::ADAPTER),
            &json!({"id":"ordinary-id"}),
            &mut trace
        )
        .is_none());
        assert_eq!(trace, before);
    }
}
