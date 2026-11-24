//! Storage-phase barriers are test-only and explicitly cloned across the owned task boundary.
use super::*;
use serde_json::json;
use tokio::sync::Notify;

pub(super) fn run<F: std::future::Future<Output = ()> + 'static>(test: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    tokio::time::timeout(std::time::Duration::from_secs(15), test())
                        .await
                        .unwrap();
                });
        })
        .unwrap()
        .join()
        .unwrap();
}

#[derive(Default)]
pub(crate) struct Pause {
    reached: Notify,
    resume: Notify,
}
impl Pause {
    pub(crate) async fn entered(&self) {
        self.reached.notified().await;
    }
    pub(crate) fn release(&self) {
        self.resume.notify_one();
    }
    async fn wait(&self) {
        self.reached.notify_one();
        self.resume.notified().await;
    }
}
#[derive(Default, Clone)]
pub(crate) struct Hooks {
    pub before: Option<Arc<Pause>>,
    pub created: Option<Arc<Pause>>,
    pub finish: Option<Arc<Pause>>,
    pub returning: Option<Arc<Notify>>,
}
tokio::task_local! { static HOOKS: Hooks; }
pub(super) fn capture() -> Hooks {
    HOOKS.try_with(Clone::clone).unwrap_or_default()
}
impl Hooks {
    #[cfg(any(feature = "line-commandcode-provider-api", feature = "line-sensenova-token-plan"))]
    pub(crate) fn pause_at_finish(pause: Arc<Pause>) -> Self {
        Self {
            finish: Some(pause),
            ..Self::default()
        }
    }

    pub(crate) async fn within<T>(&self, future: impl std::future::Future<Output = T>) -> T {
        HOOKS.scope(self.clone(), future).await
    }
    pub(super) async fn before_create(&self) {
        if let Some(pause) = &self.before {
            pause.wait().await;
        }
    }
    pub(super) async fn after_create(&self) {
        if let Some(pause) = &self.created {
            pause.wait().await;
        }
    }
    pub(super) async fn before_finish(&self) {
        if let Some(pause) = &self.finish {
            pause.wait().await;
        }
    }
    pub(super) fn returning(&self) {
        if let Some(returning) = &self.returning {
            returning.notify_one();
        }
    }
}
#[path = "../../tests/console_contract_support/mod.rs"]
pub(super) mod console;
pub(super) async fn fixture() -> (console::ConsoleStateFixture, LocalRuntime) {
    let mut fixture = console::ConsoleStateFixture::new(
        serde_json::from_value(json!({"providers":[]})).unwrap(),
        false,
    );
    let directory = std::env::temp_dir().join(format!("probe-audit-{}", uuid::Uuid::new_v4()));
    let local = LocalRuntime::open(&directory).await.unwrap();
    Arc::get_mut(&mut fixture.state).unwrap().local_runtime = Some(local.clone());
    (fixture, local)
}
pub(crate) fn input() -> db::CreateRequestAuditInput {
    db::CreateRequestAuditInput {
        project_id: "console-model-probe".into(),
        api_key_id: None,
        user_credential_id: None,
        access_key_id: None,
        source_access_key_id: None,
        session_id: None,
        route_policy_id: None,
        provider_account_id: Some("synthetic-provider".into()),
        protocol_family: "bedrock_converse".into(),
        endpoint_kind: "converse_stream".into(),
        requested_model: Some("synthetic-model".into()),
        resolved_model: Some("synthetic-model".into()),
        model_alias: None,
        stream: true,
        route_attempt_count: 1,
        response_id: format!("probe-{}", uuid::Uuid::new_v4()),
        previous_response_id: None,
        route_trace: Some(json!({"realCredentialRef":"synthetic-exact","consoleModelProbe":true})),
    }
}
pub(super) async fn terminal(local: &LocalRuntime, status: &str) -> db::GatewayRequestAuditView {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            let rows = local
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            if rows.len() == 1 && rows[0].status == status {
                return rows.into_iter().next().unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
pub(super) async fn ready(pause: &Pause, future: &mut PinFuture<'_>) {
    tokio::time::timeout(std::time::Duration::from_secs(3),async {
        tokio::select! { _ = pause.entered() => {}, _ = future.as_mut() => panic!("Storage barrier missed") }
    }).await.unwrap();
}
pub(super) type PinFuture<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>;
pub(super) async fn close(fixture: console::ConsoleStateFixture, local: LocalRuntime) {
    drop(fixture);
    tokio::time::timeout(std::time::Duration::from_secs(3), local.close())
        .await
        .unwrap();
}
