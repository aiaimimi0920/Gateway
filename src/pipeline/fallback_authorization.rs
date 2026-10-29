//! Keep fallback authority by owner so key refresh cannot retain stale grants.
use super::PipelineContext;

impl PipelineContext {
    pub(super) fn fallback_provider_authorized(&self, provider: &str) -> bool {
        self.explicit_fallback_provider_ids
            .iter()
            .chain(&self.local_key_fallback_provider_ids)
            .any(|id| self.provider_identities.same(id, provider))
    }

    pub(super) fn has_other_provider_attempt(&self, provider: &str) -> bool {
        self.attempted_provider_ids
            .lock()
            .iter()
            .any(|id| !self.provider_identities.same(id, provider))
    }

    pub(super) fn provider_allowed_for_send(&self, provider: &str) -> bool {
        let policy = self.route_policy_config.as_ref();
        super::stage_route::provider_allowed_by_policy(provider, policy)
            || self
                .provider_identities
                .counterpart(provider)
                .is_some_and(|other| super::stage_route::provider_allowed_by_policy(other, policy))
    }

    pub(super) fn provider_explicitly_allowed_for_send(&self, provider: &str) -> bool {
        let policy = self.route_policy_config.as_ref();
        super::stage_route::provider_explicitly_allowed_by_policy(provider, policy)
            || self
                .provider_identities
                .counterpart(provider)
                .is_some_and(|other| {
                    super::stage_route::provider_explicitly_allowed_by_policy(other, policy)
                })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, ProtocolFamily};

    fn context() -> PipelineContext {
        PipelineContext::new(
            CanonicalRelayRequest {
                protocol_family: ProtocolFamily::OpenAi,
                endpoint_kind: EndpointKind::ChatCompletions,
                requested_model: Some("test-model".into()),
                stream: false,
                messages: vec![],
                tools: vec![],
                tool_choice: None,
                reasoning: None,
                metadata: None,
                raw_body: serde_json::json!({}),
                previous_response_id: None,
                explicit_session_key: None,
                extra: Default::default(),
            },
            None,
        )
    }

    #[test]
    fn refreshed_key_authority_replaces_only_key_grants() {
        let mut ctx = context();
        ctx.explicit_fallback_provider_ids = vec!["route-only".into(), "shared".into()];
        ctx.local_key_fallback_provider_ids = vec!["stale-key-only".into(), "shared".into()];
        assert!(ctx.fallback_provider_authorized("stale-key-only"));
        ctx.local_key_fallback_provider_ids = vec!["new-key-only".into()];
        assert!(!ctx.fallback_provider_authorized("stale-key-only"));
        for id in ["route-only", "shared", "new-key-only"] {
            assert!(ctx.fallback_provider_authorized(id));
        }
        assert!(!ctx.fallback_provider_authorized("unrelated"));
    }

    #[test]
    fn unrestricted_or_empty_key_grants_do_not_authorize_arbitrary_fallback() {
        let mut ctx = context();
        ctx.explicit_fallback_provider_ids = vec!["route-only".into()];
        ctx.local_key_fallback_provider_ids = vec!["key-only".into()];
        ctx.local_key_fallback_provider_ids = None::<Vec<String>>.unwrap_or_default();
        assert!(!ctx.fallback_provider_authorized("key-only"));
        assert!(ctx.fallback_provider_authorized("route-only"));
        ctx.local_key_fallback_provider_ids = Some(vec![]).unwrap_or_default();
        assert!(!ctx.fallback_provider_authorized("arbitrary"));
    }

    #[test]
    fn refreshed_alias_ids_do_not_infer_other_provider_authority() {
        let mut ctx = context();
        ctx.local_key_fallback_provider_ids = vec!["old".into(), "new-api".into()];
        assert!(ctx.fallback_provider_authorized("old"));
        assert!(ctx.fallback_provider_authorized("new-api"));
        for id in ["new", "NEW-API", "other-api"] {
            assert!(!ctx.fallback_provider_authorized(id));
        }
        assert!(ctx.candidates.is_empty());
        assert!(ctx.projected_access_candidates.is_empty());
    }

    fn aliases() -> crate::provider_identity::ProviderAliases {
        crate::provider_identity::ProviderAliases::new(vec![
            ("old".into(), "new-api".into()),
            ("other".into(), "other-api".into()),
        ])
        .unwrap()
    }

    #[test]
    fn rename_is_not_cross_provider_fallback_and_observed_attempt_ids_are_untouched() {
        let mut ctx = context();
        ctx.attempted_provider_ids.lock().push("old".into());
        assert!(ctx.has_other_provider_attempt("new-api"));
        ctx.provider_identities = aliases();
        assert!(!ctx.has_other_provider_attempt("new-api"));
        assert!(!ctx.has_other_provider_attempt("old"));
        assert!(ctx.has_other_provider_attempt("other-api"));
        ctx.attempted_provider_ids.lock().push("other".into());
        assert!(ctx.has_other_provider_attempt("new-api"));
        assert_eq!(*ctx.attempted_provider_ids.lock(), vec!["old", "other"]);
    }

    #[test]
    fn send_policy_and_fallback_grants_expand_only_the_same_principal() {
        for (allowed, target) in [("old", "new-api"), ("new-api", "old")] {
            let mut ctx = context();
            ctx.provider_identities = aliases();
            ctx.route_policy_config = Some(crate::db::GatewayRoutePolicyConfig {
                allowed_provider_account_ids: Some(vec![allowed.into()]),
                ..Default::default()
            });
            ctx.explicit_fallback_provider_ids = vec![allowed.into()];
            assert!(ctx.provider_allowed_for_send(target));
            assert!(ctx.provider_explicitly_allowed_for_send(target));
            assert!(ctx.fallback_provider_authorized(target));
            for other in ["other-api", "unrelated", "NEW-API"] {
                assert!(!ctx.provider_allowed_for_send(other));
                assert!(!ctx.provider_explicitly_allowed_for_send(other));
                assert!(!ctx.fallback_provider_authorized(other));
            }
        }
    }

    #[test]
    fn unconstrained_route_is_still_not_explicit_fallback_authority() {
        let mut ctx = context();
        ctx.provider_identities = aliases();
        for allowed in [None, Some(vec![]), Some(vec![" ".into()])] {
            ctx.route_policy_config = Some(crate::db::GatewayRoutePolicyConfig {
                allowed_provider_account_ids: allowed,
                ..Default::default()
            });
            assert!(ctx.provider_allowed_for_send("new-api"));
            assert!(!ctx.provider_explicitly_allowed_for_send("new-api"));
            assert!(!ctx.fallback_provider_authorized("new-api"));
        }
        ctx.local_key_fallback_provider_ids = vec!["old".into()];
        assert!(ctx.fallback_provider_authorized("new-api"));
        ctx.local_key_fallback_provider_ids.clear();
        assert!(!ctx.fallback_provider_authorized("new-api"));
    }
}
