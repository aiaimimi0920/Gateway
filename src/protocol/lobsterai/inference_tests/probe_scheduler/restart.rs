//! 同一隔离存储关闭并重建 AppState，验证结果和到期槽不依赖旧内存。
use super::*;

async fn slots(f: &Fixture) -> Vec<(String, i64)> {
    sqlx::query_as("SELECT id, next FROM credential_test_slots ORDER BY id")
        .fetch_all(&f.state.local_runtime.as_ref().unwrap().pool)
        .await
        .unwrap()
}

async fn saved(f: &Fixture) -> Vec<Value> {
    let mut values = Vec::new();
    for plan in [None, Some("named")] {
        let rows = test_store::read_many(&f.state, "lobsterai-public", &["exact"], plan)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        values.push(serde_json::to_value(&rows[0]).unwrap());
    }
    values
}

#[tokio::test]
async fn lobsterai_scheduler_runtime_restart_preserves_results_slots_and_next_due_round() {
    Box::pin(async {
        let peer = Peer::start(
            (0..4)
                .map(|_| Reply::raw(200, native::reply("chat", false, false, true)))
                .collect(),
        )
        .await;
        let f = fixture(&peer, true).await;
        let summary = crate::protocol::lobsterai::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), Box::pin(sweep(&f.state)))
            .await
            .unwrap();
        assert_eq!((summary.due_count, summary.passed_count), (2, 2));
        results(&f, "passed").await;
        let old_results = saved(&f).await;
        let old_slots = slots(&f).await;
        assert_eq!(old_slots.len(), 2);
        let old_id = f.id.clone();
        let routes = std::fs::read(&f.state.config.console.routes_file).unwrap();
        let old_owner = Arc::downgrade(&f.state);
        let f = Box::pin(f.restart(peer.origin.clone())).await;
        assert!(
            old_owner.upgrade().is_none(),
            "old runtime owner survived restart"
        );
        assert_eq!(f.id, old_id);
        assert_eq!(f.remaining().await, 2);
        assert_eq!(
            std::fs::read(&f.state.config.console.routes_file).unwrap(),
            routes
        );
        assert_eq!(saved(&f).await, old_results);
        assert_eq!(slots(&f).await, old_slots);
        let skipped = sweep(&f.state).await.unwrap();
        assert_eq!(
            (
                skipped.scheduled_count,
                skipped.due_count,
                skipped.skipped_count
            ),
            (2, 0, 2)
        );
        wire(&peer, 2);
        // 使用原到期时间和真实 timer；不改 SQL 时间戳，也不复制内存槽。
        let worker = Box::pin(
            crate::protocol::lobsterai::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), start(f.state.clone())),
        );
        let observe = Box::pin(async {
            wait_round(&f, 4).await;
            f.state.shutdown.request("restart-round-finished");
        });
        tokio::time::timeout(Duration::from_secs(135), async {
            tokio::join!(worker, observe);
        })
        .await
        .expect("restored schedule did not become due or stop");
        results(&f, "passed").await;
        let new_slots = slots(&f).await;
        assert_eq!(new_slots.len(), 2);
        for (old, new) in old_slots.iter().zip(&new_slots) {
            assert_eq!(old.0, new.0);
            assert!(new.1 > old.1);
        }
        let audits = f.audits(4).await;
        assert!(audits
            .iter()
            .all(|r| r.status == "completed" && r.total_tokens == Some(5)));
        wire(&peer, 4);
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}
