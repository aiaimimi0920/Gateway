use super::*;

#[tokio::test]
async fn providers_with_the_same_task_stripe_do_not_block_each_other() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-pool-collision-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    let (first, second) = ("provider-29", "provider-41");
    assert_eq!(Sha256::digest(first)[0], Sha256::digest(second)[0]);
    let first_guard = local.try_pool_capacity_lock(first).await.unwrap().unwrap();
    let second_guard = local.try_pool_capacity_lock(second).await.unwrap().unwrap();
    assert!(local.try_pool_capacity_lock(first).await.unwrap().is_none());
    drop((first_guard, second_guard));
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn provider_capacity_lock_excludes_other_instances_and_releases_on_drop() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-pool-admission-{}",
        uuid::Uuid::new_v4()
    ));
    let first = LocalRuntime::open(&root).await.unwrap();
    let second = LocalRuntime::open(&root).await.unwrap();
    let guard = first
        .try_pool_capacity_lock("same-key")
        .await
        .unwrap()
        .unwrap();
    assert!(second
        .try_pool_capacity_lock("same-key")
        .await
        .unwrap()
        .is_none());
    let task_guard = first.try_refill_lock("same-key").await.unwrap().unwrap();
    drop(task_guard);
    drop(guard);
    assert!(second
        .try_pool_capacity_lock("same-key")
        .await
        .unwrap()
        .is_some());
    first.close().await;
    second.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}

#[tokio::test]
async fn cancelling_the_lock_owner_releases_provider_admission() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-pool-admission-cancel-{}",
        uuid::Uuid::new_v4()
    ));
    let local = LocalRuntime::open(&root).await.unwrap();
    let guard = local
        .try_pool_capacity_lock("provider")
        .await
        .unwrap()
        .unwrap();
    let task = tokio::spawn(async move {
        let _guard = guard;
        std::future::pending::<()>().await;
    });
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(local
        .try_pool_capacity_lock("provider")
        .await
        .unwrap()
        .is_some());
    local.close().await;
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
