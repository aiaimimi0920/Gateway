//! Explicit storage-task barriers and independent real SQLite quota fixtures.
use super::*;
use crate::{auth::session::AuthenticatedSession, db, local_runtime::LocalRuntime};
use serde_json::json;
use std::{future::Future, sync::Mutex, time::Duration};
use tokio::sync::Notify;

pub(super) fn run<F: Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(Duration::from_secs(30), test())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}
#[derive(Default)]
pub(super) struct Pause {
    reached: Notify,
    resume: Notify,
}
impl Pause {
    pub(super) fn release(&self) {
        self.resume.notify_one();
    }
    async fn wait(&self) {
        self.reached.notify_one();
        self.resume.notified().await;
    }
}
#[derive(Clone, Default)]
pub(super) struct Hooks {
    pub before: Option<(Stage, Arc<Pause>)>,
    pub after: Option<(Stage, Arc<Pause>)>,
    pub terminal: Option<Arc<Pause>>,
    pub returning: Option<Arc<Notify>>,
    pub events: Arc<Mutex<Vec<Stage>>>,
}
tokio::task_local! { static HOOKS: Hooks; }
pub(super) fn capture() -> Hooks {
    HOOKS.try_with(Clone::clone).unwrap_or_default()
}
impl Hooks {
    pub(super) async fn within<T>(&self, future: impl Future<Output = T>) -> T {
        HOOKS.scope(self.clone(), future).await
    }
    pub(super) async fn before(&self, stage: Stage) {
        self.events.lock().unwrap().push(stage);
        if let Some((target, pause)) = &self.before {
            if *target == stage {
                pause.wait().await;
            }
        }
    }
    pub(super) async fn after(&self, stage: Stage) {
        if let Some((target, pause)) = &self.after {
            if *target == stage {
                pause.wait().await;
            }
        }
    }
    pub(super) fn returning(&self) {
        if let Some(returning) = &self.returning {
            returning.notify_one();
        }
    }
    pub(super) async fn terminal(&self) {
        if let Some(pause) = &self.terminal {
            pause.wait().await;
        }
    }
}
#[path = "../../tests/console_contract_support/mod.rs"]
pub(super) mod console;
pub(super) struct Fixture {
    pub console: console::ConsoleStateFixture,
    pub local: LocalRuntime,
    pub key: String,
}
impl Fixture {
    pub(super) async fn new() -> Self {
        Self::with_quota(json!({"providers":[]}), "token_prepaid", 1000).await
    }
    pub(super) async fn with_quota(document: serde_json::Value, mode: &str, limit: i64) -> Self {
        let mut console =
            console::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), false);
        let directory =
            std::env::temp_dir().join(format!("pipeline-lifecycle-{}", uuid::Uuid::new_v4()));
        let local = LocalRuntime::open(&directory).await.unwrap();
        let input = db::UpsertAccessKeyInput {
            owner_type: "user".into(),
            owner_id: "fixture".into(),
            resolved_project_id: "local".into(),
            resolved_tenant_id: "local".into(),
            key_kind: "normal".into(),
            public_key_prefix: "sk-gw".into(),
            display_name: "lifecycle-quota".into(),
            expires_at: None,
            bundle_ids: vec![],
            metadata: None,
        };
        let quota = crate::access_balance::quota::KeyQuotaInput {
            mode: mode.into(),
            limit: Some(limit),
            currency: (mode == "cash_prepaid").then(|| "USD".into()),
        };
        let key = local
            .save_access_key_with_quota(None, input, Some(&quota))
            .await
            .unwrap()
            .id;
        Arc::get_mut(&mut console.state).unwrap().local_runtime = Some(local.clone());
        Self {
            console,
            local,
            key,
        }
    }
    pub(super) fn ctx(&self) -> PipelineContext {
        let req = crate::protocol::openai::normalize_chat_completions(json!({"model":"synthetic-model", "messages":[{"role":"user","content":"Synthetic nonempty text"}],"stream":false,"max_tokens":16})).unwrap();
        let mut ctx = PipelineContext::new(req, None);
        ctx.requesting_access_key_id = Some(self.key.clone());
        ctx.session = Some(AuthenticatedSession {
            project_id: "local".into(),
            tenant_id: "local".into(),
            user_id: None,
            credential_ref: None,
            scopes: vec![],
            api_key_id: None,
            user_credential_id: None,
            access_key_id: Some(self.key.clone()),
            access_key_kind: Some("normal".into()),
        });
        ctx
    }
    pub(super) async fn balance(&self) -> (i64, i64) {
        let balance = self.local.access_balance(&self.key).await.unwrap().unwrap();
        let pending:i64 = sqlx::query_scalar("SELECT coalesce(sum(i.pending),0) FROM local_balance_inflight i JOIN local_balance_keys k ON k.account_id=i.account_id WHERE k.key_id=?")
            .bind(&self.key).fetch_one(&self.local.pool).await.unwrap();
        (balance.remaining_tokens.unwrap(), pending)
    }
    pub(super) async fn audit(&self, status: &str) -> db::GatewayRequestAuditView {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let rows = self
                    .local
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == 1 && rows[0].status == status {
                    return rows.into_iter().next().unwrap();
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    pub(super) async fn close(self) {
        drop(self.console);
        tokio::time::timeout(Duration::from_secs(3), self.local.close())
            .await
            .unwrap();
    }
}
pub(super) type PinFuture<'a> = std::pin::Pin<Box<dyn Future<Output = ()> + 'a>>;
pub(super) async fn ready(pause: &Pause, future: &mut PinFuture<'_>) {
    tokio::time::timeout(Duration::from_secs(3),async {
        tokio::select! { _=pause.reached.notified()=>{}, _=future.as_mut()=>panic!("Storage barrier missed") }
    }).await.unwrap();
}
