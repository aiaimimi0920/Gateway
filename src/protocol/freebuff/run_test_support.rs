//! Shared owner for legacy focused fixtures only; production always supplies its runtime owner.
use super::*;
use std::sync::OnceLock;

pub(crate) fn test_runtime() -> &'static Arc<RunRuntime> {
    static OWNER: OnceLock<Arc<RunRuntime>> = OnceLock::new();
    OWNER.get_or_init(|| Arc::new(RunRuntime::default()))
}

pub(crate) fn run_buckets() -> &'static DashMap<String, Arc<Mutex<FreeBuffRunBucket>>> {
    test_runtime().registry.buckets()
}

pub(crate) fn run_bucket(key: &str) -> Result<Arc<Mutex<FreeBuffRunBucket>>, GatewayError> {
    test_runtime().registry.get_or_insert(key)
}

pub(crate) async fn acquire_run_lease(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<FreeBuffRunLease, GatewayError> {
    test_runtime().acquire_run_lease(client, config).await
}

pub(crate) async fn probe_run(
    client: &Client,
    config: &FreeBuffRuntimeConfig,
) -> Result<(), GatewayError> {
    test_runtime().probe_run(client, config).await
}

pub(crate) fn remove_idle_bucket(key: &str, bucket: &Arc<Mutex<FreeBuffRunBucket>>) -> bool {
    remove_idle_bucket_from(test_runtime(), key, bucket)
}
