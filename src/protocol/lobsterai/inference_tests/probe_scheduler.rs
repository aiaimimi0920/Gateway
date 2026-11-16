//! 真实调度 timer、独立计划结果和停机准入；仅使用合成 loopback 上游。
use super::super::test_support::{Peer, Reply};
use super::*;
use crate::provider_credential_probe_scheduler::{
    start_provider_credential_probe_scheduler as start,
    sweep_scheduled_provider_credentials_once as sweep,
};
use crate::provider_runtime::test_store;
mod restart;

async fn fixture(peer: &Peer, automatic: bool) -> Fixture {
    Box::pin(Fixture::with_probe_policy(
        peer.builder().retry(rquest::retry::Policy::never()).build().unwrap(),
        peer.origin.clone(), UPSTREAM_MODEL,
        json!([{"modelId":UPSTREAM_MODEL,"apiFormat":"openai","supportsToolCalling":true}]),
        false,
        Some(json!({"automaticEnabled":automatic,"intervalMinutes":1,"modelSelection":"selected","models":[MODEL],
            "cases":[{"id":"text","name":"Text","prompt":"hello","expectedAnswer":"合成答案","difficulty":1,"enabled":true}]})),
    )).await
}
async fn results(f: &Fixture, expected: &str) {
    for plan in [None, Some("named")] {
        let rows = test_store::read_many(&f.state, "lobsterai-public", &["exact"], plan)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        let raw = serde_json::to_value(&rows[0]).unwrap();
        assert_eq!(raw["status"], expected, "{raw}");
        let assessment = rows[0].assessment.as_ref().unwrap();
        assert_eq!(assessment.mode, "automatic");
        assert_eq!(assessment.plan_id.as_deref(), plan);
        assert_eq!(
            assessment.policy_source,
            if plan.is_some() {
                "plan:named"
            } else {
                "account"
            }
        );
        assert_eq!(assessment.cases.len(), 1);
        assert_eq!(
            assessment.cases[0].correct,
            if expected == "passed" {
                Some(true)
            } else {
                None
            }
        );
        assert!(!raw.to_string().contains(KEY));
        assert!(!raw.to_string().contains("private-upstream-error"));
    }
}
async fn wait_round(f: &Fixture, count: usize) {
    loop {
        let local = f.state.local_runtime.as_ref().unwrap();
        let rows = local
            .list_audits(&db::RequestAuditFilters::default())
            .await
            .unwrap();
        let saved: i64 = sqlx::query_scalar("SELECT count(*) FROM credential_test_results")
            .fetch_one(&local.pool)
            .await
            .unwrap();
        if rows.len() == count && rows.iter().all(|r| r.status == "completed") && saved == 2 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}
fn wire(peer: &Peer, count: usize) {
    peer.inspect(|calls| {
        assert_eq!(calls.len(), count);
        for call in calls {
            assert_eq!(call.path, "/api/proxy/v1/chat/completions");
            assert_eq!(call.headers["authorization"], format!("Bearer {KEY}"));
            assert_eq!(call.body["model"], UPSTREAM_MODEL);
            assert_eq!(call.body["stream"], false);
            assert_eq!(call.body["max_tokens"], 256);
            assert!(!call.headers.contains_key("cookie"));
        }
    });
}
#[tokio::test]
async fn lobsterai_scheduler_real_timer_runs_independent_account_and_named_plans() {
    Box::pin(async {
        let peer = Peer::start(
            (0..4)
                .map(|_| Reply::raw(200, native::reply("chat", false, false, true)))
                .collect(),
        )
        .await;
        let f = fixture(&peer, true).await;
        let revision = f.state.route_config.snapshot().revision().id().to_owned();
        let worker = Box::pin(
            super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(), start(f.state.clone())),
        );
        let observe = Box::pin(async {
            wait_round(&f, 2).await;
            results(&f, "passed").await;
            let skipped = sweep(&f.state).await.unwrap();
            assert_eq!(skipped.due_count, 0);
            assert_eq!(skipped.skipped_count, 2);
            // 不暂停时钟、不修改持久到期槽，等待真实 60 秒 tick。
            wait_round(&f, 4).await;
            f.state.shutdown.request("lobsterai-scheduler-finished");
        });
        tokio::time::timeout(Duration::from_secs(135), async {
            tokio::join!(worker, observe);
        })
        .await
        .expect("real scheduler round or shutdown did not finish");
        results(&f, "passed").await;
        wire(&peer, 4);
        let rows = f.audits(4).await;
        assert!(rows
            .iter()
            .all(|r| r.total_tokens == Some(5) && r.access_key_id.is_none()));
        assert!(rows
            .iter()
            .all(|r| r.route_trace.as_ref().unwrap()["realCredentialRef"] == "exact"));
        assert_eq!(f.remaining().await, 2);
        assert_eq!(f.state.route_config.snapshot().revision().id(), revision);
        f.finish().await;
    })
    .await;
}
#[tokio::test]
async fn lobsterai_scheduler_failure_persists_without_replay_or_fabricated_usage() {
    Box::pin(async {
        let peer = Peer::start(
            (0..2)
                .map(|_| Reply::raw(500, b"private-upstream-error".to_vec()))
                .collect(),
        )
        .await;
        let f = fixture(&peer, true).await;
        let summary = super::super::transport::TEST_ORIGIN
            .scope(peer.origin.clone(), Box::pin(sweep(&f.state)))
            .await
            .unwrap();
        assert_eq!(
            (
                summary.scheduled_count,
                summary.due_count,
                summary.failed_count
            ),
            (2, 2, 2)
        );
        assert_eq!(summary.passed_count, 0);
        results(&f, "failed").await;
        let rows = f.audits(2).await;
        assert!(rows
            .iter()
            .all(|r| r.status == "failed" && r.total_tokens.is_none()));
        let again = sweep(&f.state).await.unwrap();
        assert_eq!(again.due_count, 0);
        wire(&peer, 2);
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}
#[tokio::test]
async fn lobsterai_scheduler_disabled_policy_and_shutdown_never_generate() {
    Box::pin(async {
        let peer = Peer::start(vec![]).await;
        let f = fixture(&peer, false).await;
        let summary = sweep(&f.state).await.unwrap();
        assert_eq!(summary.scheduled_count, 0);
        assert!(f.audits(0).await.is_empty());
        f.finish().await;
        let f = fixture(&peer, true).await;
        f.state.shutdown.request("already-stopped");
        tokio::time::timeout(Duration::from_secs(2), start(f.state.clone()))
            .await
            .unwrap();
        wire(&peer, 0);
        assert!(f.audits(0).await.is_empty());
        assert_eq!(f.remaining().await, 2);
        f.finish().await;
    })
    .await;
}
