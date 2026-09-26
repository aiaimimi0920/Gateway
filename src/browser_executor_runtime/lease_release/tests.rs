use super::{DEFAULT_SLOT_TTL_SECONDS, RELEASED_LEASE_TTL_SECONDS, RELEASE_SCRIPT};
use redis::AsyncCommands;

struct ScriptFixture {
    conn: redis::aio::MultiplexedConnection,
    lock_key: String,
    slot_key: String,
    lease_key: String,
}

impl ScriptFixture {
    async fn new() -> Self {
        let url = std::env::var("GATEWAY_BROWSER_EXECUTOR_TEST_REDIS_URL")
            .expect("set the dedicated Redis fixture URL");
        let run_id = std::env::var("GATEWAY_BROWSER_EXECUTOR_TEST_RUN_ID")
            .expect("set the dedicated Redis fixture guard");
        assert!(!run_id.is_empty());
        let mut conn = redis::Client::open(url)
            .unwrap()
            .get_multiplexed_async_connection()
            .await
            .unwrap();
        let guard: Option<String> = conn.get("gw:browser_executor:contract_run").await.unwrap();
        assert_eq!(guard.as_deref(), Some(run_id.as_str()));
        let prefix = format!(
            "gw:browser_executor:lease-release-test:{}",
            uuid::Uuid::new_v4()
        );
        let mut fixture = Self {
            conn,
            lock_key: format!("{prefix}:lock"),
            slot_key: format!("{prefix}:slot"),
            lease_key: format!("{prefix}:lease"),
        };
        fixture.seed("original-owner", "initial slot").await;
        fixture
    }

    async fn seed(&mut self, owner: &str, slot: &str) {
        redis::pipe()
            .cmd("SET")
            .arg(&self.lock_key)
            .arg(owner)
            .arg("EX")
            .arg(60)
            .ignore()
            .cmd("SET")
            .arg(&self.slot_key)
            .arg(slot)
            .arg("EX")
            .arg(60)
            .ignore()
            .cmd("SET")
            .arg(&self.lease_key)
            .arg("active record")
            .arg("EX")
            .arg(60)
            .ignore()
            .query_async::<()>(&mut self.conn)
            .await
            .unwrap();
    }

    async fn commit(
        &mut self,
        owned_at_snapshot: bool,
        observed_slot: Option<&str>,
        released_slot: &str,
    ) -> i64 {
        redis::Script::new(RELEASE_SCRIPT)
            .key(&self.lock_key)
            .key(&self.slot_key)
            .key(&self.lease_key)
            .arg("original-owner")
            .arg(u8::from(owned_at_snapshot))
            .arg(u8::from(observed_slot.is_some()))
            .arg(observed_slot.unwrap_or_default())
            .arg(released_slot)
            .arg("released record")
            .arg(DEFAULT_SLOT_TTL_SECONDS)
            .arg(RELEASED_LEASE_TTL_SECONDS)
            .invoke_async(&mut self.conn)
            .await
            .unwrap()
    }

    async fn assert_storage(&mut self, lock: Option<&str>, slot: &str, lease: &str) {
        let stored: (Option<String>, String, String) = redis::cmd("MGET")
            .arg(&self.lock_key)
            .arg(&self.slot_key)
            .arg(&self.lease_key)
            .query_async(&mut self.conn)
            .await
            .unwrap();
        assert_eq!(stored.0.as_deref(), lock);
        assert_eq!(stored.1, slot);
        assert_eq!(stored.2, lease);
    }
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn changed_slot_retries_without_discarding_heartbeat() {
    let mut fixture = ScriptFixture::new().await;
    fixture.seed("original-owner", "fresh heartbeat").await;
    assert_eq!(
        fixture
            .commit(true, Some("initial slot"), "stale release")
            .await,
        0
    );
    fixture
        .assert_storage(Some("original-owner"), "fresh heartbeat", "active record")
        .await;
    assert_eq!(
        fixture
            .commit(
                true,
                Some("fresh heartbeat"),
                "released with fresh heartbeat"
            )
            .await,
        1
    );
    fixture
        .assert_storage(None, "released with fresh heartbeat", "released record")
        .await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn replacement_owner_after_snapshot_is_preserved() {
    let mut fixture = ScriptFixture::new().await;
    fixture.seed("replacement-owner", "replacement slot").await;
    assert_eq!(
        fixture
            .commit(true, Some("initial slot"), "stale release")
            .await,
        1
    );
    fixture
        .assert_storage(
            Some("replacement-owner"),
            "replacement slot",
            "released record",
        )
        .await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn missing_slot_snapshot_preserves_a_new_slot() {
    let mut fixture = ScriptFixture::new().await;
    assert_eq!(fixture.commit(true, None, "").await, 0);
    fixture
        .assert_storage(Some("original-owner"), "initial slot", "active record")
        .await;
}

#[tokio::test]
#[ignore = "requires an isolated, guarded Redis fixture"]
async fn regained_ownership_requires_a_fresh_snapshot() {
    let mut fixture = ScriptFixture::new().await;
    assert_eq!(fixture.commit(false, Some("initial slot"), "").await, 0);
    fixture
        .assert_storage(Some("original-owner"), "initial slot", "active record")
        .await;
}
