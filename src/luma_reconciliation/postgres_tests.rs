//! Opt-in PostgreSQL contract against the real deploy DDL in a guarded disposable database.
use super::*;
use crate::upstream::luma_agents::{pipeline_fixture::Fixture, test_support::*};
use sqlx::types::Json;

pub(super) async fn fixture_pool() -> sqlx::PgPool {
    let address = std::env::var("GATEWAY_LUMA_TEST_DATABASE_URL").unwrap();
    let url = url::Url::parse(&address).unwrap();
    assert_eq!(url.host_str(), Some("127.0.0.1"));
    assert_eq!(url.path(), "/gateway_luma_fixture");
    assert_ne!(url.port(), Some(5432));
    let pg = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&address)
        .await
        .unwrap();
    let guard: String = sqlx::query_scalar("SELECT run_id FROM luma_fixture_guard")
        .fetch_one(&pg)
        .await
        .unwrap();
    assert_eq!(guard, std::env::var("GATEWAY_LUMA_TEST_RUN_ID").unwrap());
    pg
}

#[tokio::test]
#[ignore = "requires tools/luma/check-postgres.ps1 isolated PostgreSQL fixture"]
async fn luma_reconciliation_postgres_preserves_finalization_and_fences_competing_claims() {
    let address = std::env::var("GATEWAY_LUMA_TEST_DATABASE_URL").unwrap();
    let pg = fixture_pool().await;
    let mut latest = job("completed");
    latest["usage"] = json!({"credits_charged":12.5});
    let server = Server::start(vec![
        Reply::json(200, latest.clone()),
        Reply::json(200, latest),
    ])
    .await;
    within(&server.origin, Box::pin(async {
        let f = Fixture::new(&server, 5).await;
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO gateway_request_audits
            (id,project_id,api_key_id,protocol_family,endpoint_kind,status,response_id,created_at,updated_at)
            VALUES ($1,'platform-default-project','gateway-key','openai','images.generations','running',$1,now(),now())")
            .bind(&id).execute(&pg).await.unwrap();
        let snapshot = f.state.route_config.snapshot();
        let target = snapshot.credential_probe_targets_for_provider("luma-pipeline").unwrap()
            .into_iter().find(|t| t.credential_id == "exact").unwrap();
        let checkpoint = Checkpoint::new(&f.state, &id, "luma-pipeline", "exact", &target.payload).unwrap();
        let store = store::Store::Server(pg.clone());
        let facts = json!({"id":ID,"model":"uni-1","type":"image","state":"queued",
            "usage":{"credits_charged":null},"mediaVerified":false});
        store.checkpoint(&id, &facts, &checkpoint.journal()).await.unwrap();
        assert!(store.due(now()+61).await.unwrap().is_empty());
        let finalization = crate::db::FinalizeRequestAuditInput {
            status:"cancelled".into(), client_disconnected:true, upstream_status:None, duration_ms:5,
            prompt_tokens:None, completion_tokens:None, total_tokens:None,
            cache_creation_input_tokens:None, cache_read_input_tokens:None,
            client_has_cache_control:false, auto_cache_applied:false, error_summary:None,
            access_key_id:None, source_access_key_id:None, session_id:None, route_policy_id:None,
            provider_account_id:None, resolved_model:Some("uni-1".into()), model_alias:None,
            route_attempt_count:1, route_trace:Some(json!({"lumaAgentsGeneration":facts})), response_id:Some(ID.into()),
        };
        crate::db::finalize_request_audit(&pg, &id, finalization.clone()).await.unwrap();
        let due = store.due(now()+61).await.unwrap();
        assert_eq!(due.len(),1);
        assert_eq!(due[0].1["lumaReconciliation"]["state"], "pending");
        let mut claim = due[0].1.clone();
        claim["lumaReconciliation"]["nextAt"] = json!(now()+600);
        let peer = sqlx::postgres::PgPoolOptions::new().max_connections(1).connect(&address).await.unwrap();
        let competing = store::Store::Server(peer.clone());
        let (a,b) = tokio::join!(store.update(&id,&due[0].1,&claim), competing.update(&id,&due[0].1,&claim));
        assert_ne!(a.unwrap(), b.unwrap());
        assert!(competing.due(now()+61).await.unwrap().is_empty());
        worker::tick(&f.state, &competing, now()+601).await.unwrap();
        let (status,response,Json(trace)): (String,String,Json<Value>) = sqlx::query_as(
            "SELECT status,response_id,route_trace FROM gateway_request_audits WHERE id=$1")
            .bind(&id).fetch_one(&peer).await.unwrap();
        assert_eq!(status,"cancelled");
        assert_eq!(response,ID);
        assert_eq!(trace["lumaAgentsGeneration"]["state"],"queued");
        assert_eq!(trace["lumaAgentsGeneration"]["usage"]["credits_charged"],12.5);
        assert_eq!(trace["lumaReconciliation"]["state"],"settled");
        assert!(!store.update(&id,&claim,&due[0].1).await.unwrap());
        assert_eq!(server.calls().len(),1);
        assert_eq!(server.calls()[0].method,"GET");
        // Reproduce a killed Server's durable running row after its maximum generation lifetime.
        sqlx::query("UPDATE gateway_request_audits SET status='running' WHERE id=$1")
            .bind(&id).execute(&pg).await.unwrap();
        store.checkpoint(&id, &facts, &checkpoint.journal()).await.unwrap();
        assert!(store.due(now()+61).await.unwrap().is_empty());
        worker::tick(&f.state, &competing, now()+901).await.unwrap();
        let status: String = sqlx::query_scalar("SELECT status FROM gateway_request_audits WHERE id=$1")
            .bind(&id).fetch_one(&pg).await.unwrap();
        assert_eq!(status,"running");
        crate::db::finalize_request_audit(&pg,&id,finalization).await.unwrap();
        let Json(trace): Json<Value> = sqlx::query_scalar("SELECT route_trace FROM gateway_request_audits WHERE id=$1")
            .bind(&id).fetch_one(&pg).await.unwrap();
        assert_eq!(trace["lumaAgentsGeneration"]["usage"]["credits_charged"],12.5);
        assert_eq!(trace["lumaReconciliation"]["state"],"settled");
        assert_eq!(server.calls().len(),2);
        peer.close().await;
        f.finish().await;
    })).await;
    server.stop().await;
    pg.close().await;
}
