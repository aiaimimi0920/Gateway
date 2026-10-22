//! Request-scoped identity context, never serialized or reconstructed from upstream headers.
use std::future::Future;

#[derive(Clone, Default)]
pub(crate) struct Context {
    pub caller_scope: String,
    pub session_hint: Option<String>,
    pub native_claude: bool,
    pub requested_betas: String,
    pub native_os: Option<String>,
    pub native_arch: Option<String>,
}

tokio::task_local! { static CURRENT: Context; }

pub(crate) async fn within<F: Future>(context: Context, future: F) -> F::Output {
    // Scoped per poll and restored on cancellation; no process-global caller identity.
    CURRENT.scope(context, future).await
}

pub(crate) fn current() -> Option<Context> {
    CURRENT.try_with(Clone::clone).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cpa_context_isolated_across_tasks_and_nested_scopes() {
        assert!(current().is_none());
        let run = |name: &'static str| async move {
            within(
                Context {
                    caller_scope: name.into(),
                    ..Default::default()
                },
                async {
                    tokio::task::yield_now().await;
                    assert_eq!(current().unwrap().caller_scope, name);
                    within(Context::default(), async {
                        assert!(current().unwrap().caller_scope.is_empty());
                    })
                    .await;
                    assert_eq!(current().unwrap().caller_scope, name);
                    assert!(tokio::spawn(async { current().is_none() }).await.unwrap());
                },
            )
            .await;
        };
        tokio::join!(run("a"), run("b"));
        assert!(current().is_none());
    }
}
