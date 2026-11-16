//! Isolated LocalRuntime files; reopening preserves real schedule slots and result rows.
use super::*;
use crate::local_runtime::LocalRuntime;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
pub(super) fn access(subject: &str) -> String {
    format!(
        "{}.{}.{}",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(
            json!({"sub":subject,"exp":4070908800_u64,"scope":"inference:invoke"}).to_string()
        ),
        URL_SAFE_NO_PAD.encode(b"synthetic-signature")
    )
}
fn credential(id: &str, target: nous_portal::InferenceTarget) -> Value {
    let access = access(id);
    let receipt = nous_portal::ReceivedTokens::from_json(&serde_json::to_vec(&json!({
        "access_token":access,"refresh_token":format!("nous-refresh-{id}"),"scope":"inference:invoke",
        "inference_base_url":target.base_url()
    })).unwrap()).unwrap();
    let tokens = receipt
        .admit(
            &nous_portal::OAuthClient::new("permitted-gateway").unwrap(),
            None,
        )
        .unwrap();
    json!({"id":id,"api_key":access,"refresh_token":tokens.refresh_token,"nous_portal":tokens.context,
        "supported_models":["org/model"],"expires_at":"2099-01-01T00:00:00Z"})
}

pub(super) fn document(automatic: bool) -> Value {
    let policy = |model: &str, enabled: bool| {
        json!({"automaticEnabled":enabled,"intervalMinutes":1,
        "modelSelection":"selected","models":[model],"cases":[{"id":"text","name":"Text",
        "prompt":"only OK","expectedAnswer":"合成答案","difficulty":1,"enabled":true}]})
    };
    let providers: Vec<_> = [("subscription", nous_portal::InferenceTarget::Subscription), ("welcome", nous_portal::InferenceTarget::Welcome)].into_iter().map(|(id,target)| {
        let credentials: Vec<_> = ["active","disabled","off"].into_iter().map(|suffix| {
            let account = format!("{id}-{suffix}");
            let mut value = credential(&account,target);
            value["enabled"] = json!(suffix != "disabled");
            value["test_policy"] = policy("public-nous", automatic && suffix != "off");
            value
        }).collect();
        json!({"id":id,"preset":"nous-portal-oauth","auth_mode":"nous_portal_oauth",
            "base_url":target.base_url(),"explicit_credentials_only":true,
            "supported_models":["public-nous"],"model_map":{"public-nous":"org/model"},
            "credentials":credentials,
            "test_plans":[{"id":"named","name":"Named","scopes":[{"kind":"account","id":format!("{id}-active")}],"policy":policy("public-nous",automatic)}]})
    }).collect();
    json!({"providers":providers})
}

pub(super) struct Fixture {
    inner: support::ConsoleStateFixture,
    local: LocalRuntime,
}
impl Fixture {
    pub async fn new(peer: &peer::Peer, root: &Path, document: Value) -> Self {
        let mut inner =
            support::ConsoleStateFixture::new(serde_json::from_value(document).unwrap(), true);
        let local = LocalRuntime::open(root).await.unwrap();
        let state = Arc::get_mut(&mut inner.state).unwrap();
        state.upstream_client.http = peer.client();
        state.local_runtime = Some(local.clone());
        Self { inner, local }
    }
    pub fn state(&self) -> &Arc<AppState> {
        &self.inner.state
    }
    pub async fn wait_audits(&self, count: usize) {
        loop {
            let rows = self
                .local
                .list_audits(&db::RequestAuditFilters::default())
                .await
                .unwrap();
            let results: i64 = sqlx::query_scalar("SELECT count(*) FROM credential_test_results")
                .fetch_one(&self.local.pool)
                .await
                .unwrap();
            if rows.len() == count
                && rows.iter().all(|row| row.status == "completed")
                && results == 4
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    pub async fn assert_audits(&self, count: usize, status: &str, tokens: Option<i32>) {
        let rows = self
            .local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        assert_eq!(rows.len(), count);
        for row in rows {
            assert_eq!(row.status, status);
            assert_eq!(row.total_tokens, tokens);
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                format!("{}-active", row.provider_account_id.as_deref().unwrap())
            );
            assert!(row.access_key_id.is_none());
        }
    }
    pub async fn close(self) {
        drop(self.inner);
        self.local.close().await;
    }
}

// This scheduler fixture permits text-only nonstreaming requests, not tool/public-protocol flows.
pub(super) mod native {
    use super::*;
    pub(crate) fn reply(family: &str, tools: bool, stream: bool, terminal: bool) -> Vec<u8> {
        assert_eq!(family, "chat");
        assert!(!tools && !stream && terminal);
        json!({"id":"scheduler-reply","model":"opaque-model","choices":[{"index":0,
            "message":{"role":"assistant","content":"合成答案"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})
        .to_string()
        .into_bytes()
    }
}

// Match the existing management-test harness: large AppState futures need a bounded larger stack.
pub(super) fn run<F: std::future::Future<Output = ()> + 'static>(f: fn() -> F) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(f())
        })
        .unwrap()
        .join()
        .unwrap();
}
