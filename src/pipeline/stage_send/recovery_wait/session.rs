//! A root conversation and its child tasks share fairness, never execution or authority.
use crate::pipeline::PipelineContext;
use sha2::{Digest, Sha256};

pub(super) fn waiting_identity(ctx: &PipelineContext) -> Option<[u8; 32]> {
    // Codex children inherit the root session-id while owning separate thread-ids.
    // Group the whole task tree as requested; never infer it from a client/window,
    // child thread or cache key. Only use the separately validated header capture.
    let canonical = ctx.client_session_hints.get("session-id");
    let alias = ctx.client_session_hints.get("session_id");
    if canonical
        .zip(alias)
        .is_some_and(|(a, b)| a.trim() != b.trim())
    {
        return None;
    }
    let value = canonical.or(alias)?;
    if [canonical, alias].into_iter().flatten().any(|value| {
        value.chars().any(char::is_control) || value.trim().is_empty() || value.trim().len() > 256
    }) {
        return None;
    }
    let mut hash = Sha256::new();
    hash.update(b"gateway-recovery-root-session:v1\0");
    hash.update(value.trim().as_bytes());
    Some(hash.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> PipelineContext {
        PipelineContext::new(
            crate::protocol::responses::normalize_responses(serde_json::json!({
                "input":"fixture", "user":"shared-user", "prompt_cache_key":"shared-cache",
                "previous_response_id":"prior-response", "metadata":{"session_id":"not-a-header"}
            }))
            .unwrap(),
            None,
        )
    }

    #[test]
    fn repeated_requests_and_compatible_aliases_share_root_identity() {
        let mut first = context();
        let mut second = context();
        first
            .client_session_hints
            .insert("session-id".into(), "same-root".into());
        second
            .client_session_hints
            .insert("session_id".into(), "same-root".into());
        assert_ne!(first.req_id, second.req_id);
        assert!(waiting_identity(&first).is_some());
        assert_eq!(waiting_identity(&first), waiting_identity(&second));
        second
            .client_session_hints
            .insert("session_id".into(), "other-root".into());
        assert_ne!(waiting_identity(&first), waiting_identity(&second));
    }

    #[test]
    fn child_threads_share_the_root_but_another_main_conversation_is_independent() {
        let mut a = context();
        let mut b = context();
        for ctx in [&mut a, &mut b] {
            ctx.request_headers
                .insert("user-agent".into(), "same-client/1.0".into());
            ctx.request_headers
                .insert("x-codex-window-id".into(), "same-window".into());
            ctx.client_session_hints
                .insert("x-session-affinity".into(), "same-client".into());
        }
        assert!(waiting_identity(&a).is_none() && waiting_identity(&b).is_none());
        a.client_session_hints
            .insert("session-id".into(), "main-conversation-a".into());
        b.client_session_hints
            .insert("session-id".into(), "main-conversation-b".into());
        assert!(waiting_identity(&a).is_some() && waiting_identity(&b).is_some());
        assert_ne!(waiting_identity(&a), waiting_identity(&b));
        a.request_headers
            .insert("thread-id".into(), "root-a".into());
        for child in ["child-a1", "child-a2", "grandchild-a1"] {
            b.client_session_hints
                .insert("session-id".into(), "main-conversation-a".into());
            b.request_headers.insert("thread-id".into(), child.into());
            b.request_headers
                .insert("x-codex-parent-thread-id".into(), "root-a".into());
            assert_eq!(waiting_identity(&a), waiting_identity(&b));
        }
        b.request_headers
            .insert("user-agent".into(), "same-client/2.0".into());
        assert_eq!(waiting_identity(&a), waiting_identity(&b));
    }

    #[test]
    fn absent_invalid_or_conflicting_root_does_not_infer_a_task_tree() {
        let mut ctx = context();
        for key in [
            "x-client-request-id",
            "x-session-affinity",
            "x-codex-parent-thread-id",
            "x-session-id",
            "thread-id",
        ] {
            ctx.client_session_hints.insert(key.into(), "shared".into());
        }
        ctx.request_headers
            .insert("session-id".into(), "not-a-validated-capture".into());
        assert!(waiting_identity(&ctx).is_none());
        for invalid in [String::new(), " ".into(), "x\ny".into(), "x".repeat(257)] {
            ctx.client_session_hints
                .insert("session-id".into(), invalid);
            assert!(waiting_identity(&ctx).is_none());
        }
        ctx.client_session_hints
            .insert("session-id".into(), "one-root".into());
        ctx.client_session_hints
            .insert("session_id".into(), "other-root".into());
        assert!(waiting_identity(&ctx).is_none());
        ctx.client_session_hints
            .insert("session_id".into(), "one-root\t".into());
        assert!(waiting_identity(&ctx).is_none());
    }

    #[test]
    fn sixteen_main_conversations_admit_children_without_spending_more_key_slots() {
        let queue = std::sync::Arc::new(crate::request_wait::WaitRegistry::default());
        let register = |root: &str, thread: &str| {
            let mut ctx = context();
            ctx.client_session_hints
                .insert("session-id".into(), root.into());
            ctx.request_headers
                .insert("thread-id".into(), thread.into());
            queue.enter_session("key".into(), waiting_identity(&ctx), "provider".into(), 1)
        };
        let mut roots: Vec<_> = (0..16)
            .map(|n| register(&format!("root-{n}"), &format!("thread-{n}")).unwrap())
            .collect();
        let children: Vec<_> = (0..5)
            .map(|n| register("root-0", &format!("child-{n}")).unwrap())
            .collect();
        assert!(register("root-16", "new-main-conversation").is_err());
        drop(roots.remove(0));
        assert!(
            register("root-16", "new-main-conversation").is_err(),
            "children still own their root task's slot"
        );
        drop(children);
        let replacement = register("root-16", "new-main-conversation").unwrap();
        drop((roots, replacement));
        assert!(register("root-17", "later-main-conversation").is_ok());
    }
}
