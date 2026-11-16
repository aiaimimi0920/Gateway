//! Real Redis, unique namespace, synthetic documents; no FLUSHDB, shared keys or credentials.
use neuro_gateway::console::{
    document::validate_route_document,
    revision::{RevisionActor, RevisionMetadata},
    *,
};
use neuro_gateway::routing::config::RouteConfigYaml;
use redis::AsyncCommands;
use time::OffsetDateTime;

pub struct Fixture {
    pub pool: deadpool_redis::Pool,
    pub store: RouteConfigRedisStore,
    pub revision: RouteConfigRedisRevision,
    pub old_revision: RouteConfigRedisRevision,
    pub prepared: TransactionRecord,
    pub activated: TransactionRecord,
}
impl Fixture {
    pub fn new() -> Self {
        let url = std::env::var("GATEWAY_CONSOLE_FENCE_REDIS_TEST_URL")
            .expect("explicit disposable Redis URL required");
        assert_eq!(url::Url::parse(&url).unwrap().host_str(), Some("127.0.0.1"));
        let pool = deadpool_redis::Config::from_url(url)
            .create_pool(Some(deadpool_redis::Runtime::Tokio1))
            .unwrap();
        let store =
            RouteConfigRedisStore::new(format!("fence_{}", uuid::Uuid::new_v4().simple())).unwrap();
        let document: RouteConfigYaml = serde_json::from_value(serde_json::json!({"providers":[{"id":"fixture","base_url":"https://example.invalid","api_key":"synthetic-only"}]})).unwrap();
        let validated = validate_route_document(document.clone()).unwrap();
        let mut old_document = document.clone();
        old_document.providers[0].api_key = "synthetic-old".into();
        let old_validated = validate_route_document(old_document.clone()).unwrap();
        let old_metadata = RevisionMetadata::from_validated(
            1,
            None,
            RevisionActor::Recovery,
            OffsetDateTime::now_utc(),
            None,
            &old_validated,
        );
        let old_revision = RouteConfigRedisRevision::new(old_metadata, old_document).unwrap();
        let metadata = RevisionMetadata::from_validated(
            2,
            Some(old_revision.metadata().id().into()),
            RevisionActor::Recovery,
            OffsetDateTime::now_utc(),
            None,
            &validated,
        );
        let revision = RouteConfigRedisRevision::new(metadata, document).unwrap();
        let (prepared, activated) = records("tx-first", &revision);
        Self {
            pool,
            store,
            revision,
            old_revision,
            prepared,
            activated,
        }
    }
    pub async fn install_old(&self) {
        let (prepared, activated) = records("tx-bootstrap", &self.old_revision);
        self.store
            .store_revision(&self.pool, &self.old_revision)
            .await
            .unwrap();
        self.store
            .store_prepared_transaction(&self.pool, &prepared)
            .await
            .unwrap();
        assert_eq!(
            self.store
                .activate_revision(&self.pool, None, &self.old_revision, &prepared, &activated)
                .await
                .unwrap(),
            RouteConfigRedisActivationOutcome::Activated
        );
    }
    pub async fn stage(&self) {
        self.store
            .store_revision(&self.pool, &self.revision)
            .await
            .unwrap();
        self.store
            .store_prepared_transaction(&self.pool, &self.prepared)
            .await
            .unwrap();
    }
    pub async fn activate(
        &self,
        prepared: &TransactionRecord,
        activated: &TransactionRecord,
    ) -> RouteConfigRedisActivationOutcome {
        self.store
            .activate_revision(&self.pool, None, &self.revision, prepared, activated)
            .await
            .unwrap()
    }
    pub async fn gate(&self, record: &TransactionRecord) -> Option<String> {
        self.pool
            .get()
            .await
            .unwrap()
            .get(self.store.keys().transaction_key(record.tx_id()))
            .await
            .unwrap()
    }
    pub async fn write_gate(&self, raw: &str) {
        let _: () = self
            .pool
            .get()
            .await
            .unwrap()
            .set(
                self.store.keys().transaction_key(self.prepared.tx_id()),
                raw,
            )
            .await
            .unwrap();
    }
    pub async fn legacy_activate(&self) -> Vec<String> {
        self.legacy_activate_expected(None).await
    }
    pub async fn legacy_activate_expected(&self, expected: Option<&str>) -> Vec<String> {
        let mut connection = self.pool.get().await.unwrap();
        let document = canonical(self.revision.document());
        redis::Script::new(LEGACY_ACTIVATE_LUA)
            .key(self.store.keys().active_revision_key())
            .key(self.store.keys().active_document_key())
            .key(
                self.store
                    .keys()
                    .revision_key(self.revision.metadata().id()),
            )
            .key(self.store.keys().transaction_key(self.prepared.tx_id()))
            .key("")
            .arg(expected.unwrap_or("__NONE__"))
            .arg(self.revision.metadata().id())
            .arg(document)
            .arg(canonical(&self.prepared))
            .arg(canonical(&self.activated))
            .arg(self.revision.to_json().unwrap())
            .invoke_async(&mut connection)
            .await
            .unwrap()
    }
}
pub fn records(
    id: &str,
    revision: &RouteConfigRedisRevision,
) -> (TransactionRecord, TransactionRecord) {
    let prepared = TransactionRecord::new_prepared(
        id,
        revision.metadata().parent(),
        revision.metadata().id(),
        format!("revisions/{}", revision.metadata().id()),
        false,
        None,
        revision.metadata().yaml_digest(),
        OffsetDateTime::now_utc(),
    )
    .unwrap();
    let mut activated = prepared.clone();
    activated
        .transition(
            TransactionPhase::YamlReplaced,
            OffsetDateTime::now_utc(),
            None,
        )
        .unwrap();
    activated
        .transition(
            TransactionPhase::RedisActivated,
            OffsetDateTime::now_utc(),
            None,
        )
        .unwrap();
    (prepared, activated)
}
fn canonical<T: serde::Serialize>(value: &T) -> String {
    let mut value = serde_json::to_value(value).unwrap();
    value.sort_all_objects();
    value.to_string()
}

// Pre-fence Gateway script retained verbatim to verify late old-binary writes,
// not to claim its local AlreadyActive/journal attribution has been repaired.
const LEGACY_ACTIVATE_LUA: &str = r#"
local current = redis.call('GET', KEYS[1])
if not current then current = '' end
if ARGV[1] == '__NONE__' then
  if current ~= '' then
    if current == ARGV[2] then
      return {'already_active'}
    end
    return {'revision_conflict', current}
  end
elseif current ~= ARGV[1] then
  if current == ARGV[2] then
    return {'already_active'}
  end
  return {'revision_conflict', current}
end
local stored_revision = redis.call('GET', KEYS[3])
if stored_revision ~= ARGV[6] then
  return {'revision_payload_mismatch'}
end
local stored_transaction = redis.call('GET', KEYS[4])
if stored_transaction == ARGV[5] and current == ARGV[2] then
  return {'already_active'}
end
if stored_transaction ~= ARGV[4] then
  return {'prepared_transaction_mismatch'}
end
redis.call('SET', KEYS[2], ARGV[3])
if KEYS[5] ~= '' then
  redis.call('SET', KEYS[5], ARGV[3])
end
redis.call('SET', KEYS[4], ARGV[5])
redis.call('SET', KEYS[1], ARGV[2])
return {'activated'}
"#;
