//! Discovery failures must not become a successful but wrong-product selection.
use super::*;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

#[tokio::test]
async fn kiro_object_discovery_propagates_storage_and_parse_failures() {
    // Synthetic loader codes verify propagation, not the storage parser's code vocabulary.
    for code in [
        "object_storage_object_too_large",
        "object_storage_json_invalid",
        "object_storage_object_not_found",
    ] {
        let hidden = account("hidden", key::BASE_URL);
        let inline = account("inline", key::BASE_URL);
        let mut selection = ImportAccountSelection::new(vec![
            metadata(&inline, Some(key::BASE_URL)),
            metadata(&hidden, None),
        ]);
        let error = selection
            .select_with(&descriptor(key::PRESET_ID), |_| async {
                Err(GatewayError::bad_request("synthetic read failure").with_code(code))
            })
            .await
            .expect_err("an inline candidate cannot hide an unresolved object");
        assert_eq!(error.code.as_deref(), Some(code));
        assert!(!error.message.contains("synthetic-key"));
        assert!(!selection.kiro_discovered);
    }
}

struct DropProbe(Arc<AtomicBool>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test(start_paused = true)]
async fn kiro_object_discovery_uses_one_deadline_and_cancels_the_current_read() {
    let first = account("first", key::BASE_URL);
    let second = account("second", key::BASE_URL);
    let third = account("third", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![
        metadata(&first, None),
        metadata(&second, None),
        metadata(&third, None),
    ]);
    selection.discovery_budget = Duration::from_millis(30);
    let calls = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicBool::new(false));
    let error = selection
        .select_with(&descriptor(key::PRESET_ID), |id| {
            let call = calls.fetch_add(1, Ordering::SeqCst);
            let probe = (call == 1).then(|| DropProbe(dropped.clone()));
            let value = if id == "first" {
                first.clone()
            } else {
                second.clone()
            };
            async move {
                let _probe = probe;
                tokio::time::sleep(Duration::from_millis(if call == 0 { 20 } else { 25 })).await;
                Ok(value)
            }
        })
        .await
        .expect_err("each object must share the discovery deadline");
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_account_discovery_timeout")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(dropped.load(Ordering::SeqCst));
    assert!(!selection.kiro_discovered);
}

#[tokio::test]
async fn kiro_object_discovery_does_not_retain_material_outside_the_existing_cache() {
    let key_account = account("key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    selection
        .select_with(&descriptor(key::PRESET_ID), |_| {
            let value = key_account.clone();
            async move { Ok(value) }
        })
        .await
        .unwrap();
    assert_eq!(
        selection.accounts[0].payload_base_url.as_deref(),
        Some(key::BASE_URL)
    );
    assert!(selection.cache.get("key").is_some());
}

#[tokio::test(start_paused = true)]
async fn kiro_object_discovery_caller_cancellation_drops_the_inflight_loader() {
    let key_account = account("key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    let dropped = Arc::new(AtomicBool::new(false));
    let result = tokio::time::timeout(
        Duration::from_millis(5),
        selection.select_with(&descriptor(key::PRESET_ID), |_| {
            let probe = DropProbe(dropped.clone());
            async move {
                let _probe = probe;
                std::future::pending::<Result<db::GatewayProviderAccountView, GatewayError>>().await
            }
        }),
    )
    .await;
    assert!(result.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert!(!selection.kiro_discovered);
    assert!(selection.cache.get("key").is_none());
}

#[test]
fn kiro_object_discovery_bounds_retained_base_metadata_before_cloning() {
    let key_account = account("key", key::BASE_URL);
    let mut remaining = key::BASE_URL.len();
    assert_eq!(
        discovered_base(&key_account, &mut remaining)
            .unwrap()
            .as_deref(),
        Some(key::BASE_URL)
    );
    assert_eq!(remaining, 0);
    let error = discovered_base(&key_account, &mut remaining).unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_account_discovery_metadata_limit")
    );
    assert_eq!(remaining, 0);
    assert!(!error.message.contains(key::BASE_URL));
}

#[tokio::test(start_paused = true)]
async fn kiro_object_discovery_rejects_a_late_ready_result() {
    let key_account = account("key", key::BASE_URL);
    let mut selection = ImportAccountSelection::new(vec![metadata(&key_account, None)]);
    selection.discovery_budget = Duration::from_millis(10);
    let error = selection
        .select_with(&descriptor(key::PRESET_ID), |_| {
            let value = key_account.clone();
            async move {
                tokio::time::advance(Duration::from_millis(11)).await;
                Ok(value)
            }
        })
        .await
        .unwrap_err();
    assert_eq!(
        error.code.as_deref(),
        Some("provider_credential_folder_sync_account_discovery_timeout")
    );
    assert!(!selection.kiro_discovered);
}
