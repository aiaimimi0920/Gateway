//! Hosted PostgreSQL proof for live stored SSE and a second instance's recovery GET.
use super::*;
use crate::protocol::sap_aicore::responses_lifecycle::store;
use http_body_util::BodyExt;

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; retains scoped fixture data"]
fn sap_aicore_server_hosted_stored_sse_and_cross_instance_recovery() {
    run_hosted(hosted);
}

async fn hosted() {
    let scope = format!("sap-stored-sse-{}", uuid::Uuid::new_v4().simple());
    let first_id = format!("resp_{scope}-a");
    let second_id = format!("resp_{scope}-b");
    let peer = peer::Peer::stored_stream(vec![
        stream_reply(&first_id, true),
        stream_reply(&second_id, false),
        snapshot(&second_id, "completed").to_string().into_bytes(),
    ])
    .await;
    let secret = Some("synthetic-sap-stored-sse-fixture-secret");
    let writer = build_with_key_secret(&scope, "writer", &peer, secret).await;
    let reader = build_with_key_secret(&scope, "reader", &peer, secret).await;
    let pg = reader.pg_pool.as_ref().unwrap();
    for sql in [
        store::SCHEMA,
        store::MUTATION_SCHEMA,
        store::ROUTE_SCHEMA,
        store::RECOVERY_SCHEMA,
    ] {
        sqlx::raw_sql(sql).execute(pg).await.unwrap();
    }
    let (provider, credential) = provision(
        &writer,
        &scope,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
        json!({"api_key":KEY}),
    )
    .await;
    let (key, platform) = super::stored_health::grant(&writer, &scope, &provider).await;
    let key_id = key["id"].as_str().unwrap();
    let token = key["token"].as_str().unwrap();
    manage(
        &writer,
        &format!("/v1/internal/gateway/access/keys/{key_id}/balances/adjust"),
        json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
    )
    .await;
    test_support::origin(peer.origin.clone(), async {
        for (index, upstream) in [&first_id, &second_id].into_iter().enumerate() {
            let complete = index == 0;
            let response = request(
                &writer,
                "POST",
                "/v1/responses",
                token,
                Body::from(
                    json!({"model":scope,"input":"synthetic","store":true,"stream":true})
                        .to_string(),
                ),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()["content-type"], "text/event-stream");
            let mut body = response.into_body();
            let created = frame(&mut body).await;
            assert_eq!(created["type"], "response.created");
            assert_eq!(created["response"]["model"], scope);
            let public_id = created["response"]["id"].as_str().unwrap();
            assert!(public_id.starts_with("resp_gw_"));
            let binding = json!({"upstream":upstream,"provider":provider,"credential":credential,
                "model":scope,"source":key_id,"platform":platform,"reserved":1});
            assert_eq!(
                receipt(pg, public_id).await,
                expected(&binding, "pending", 0, 1, Value::Null)
            );
            let remaining = if complete { 1 } else { 0 };
            assert_eq!(balance(pg, key_id).await, remaining);
            peer.disconnect.notify_one();
            assert_eq!(frame(&mut body).await["delta"], "synthetic");
            if complete {
                let end = frame(&mut body).await;
                assert_eq!(end["type"], "response.completed");
                assert_eq!(end["response"]["id"], public_id);
                assert_eq!(end["response"]["usage"]["total_tokens"], 5);
                assert!(body.frame().await.is_none());
            } else {
                assert!(body.frame().await.unwrap().is_err());
                assert_eq!(
                    receipt(pg, public_id).await,
                    expected(&binding, "pending", 0, 1, Value::Null)
                );
                let response = request(
                    &reader,
                    "GET",
                    &format!("/v1/responses/{public_id}"),
                    token,
                    Body::empty(),
                )
                .await;
                assert_eq!(response.status(), StatusCode::OK);
                let loaded: Value =
                    serde_json::from_str(&drain(response.into_body()).await).unwrap();
                assert_eq!(loaded["id"], public_id);
                assert_eq!(loaded["model"], scope);
                assert_eq!(loaded["usage"]["total_tokens"], 5);
            }
            drop(body);
            assert_eq!(
                receipt(pg, public_id).await,
                expected(&binding, "completed", 1, 0, json!([2, 3]))
            );
            assert_eq!(balance(pg, key_id).await, remaining);
            let audits: Vec<Value> = sqlx::query_scalar(
                "SELECT jsonb_build_array(route_trace->>'storedResponseOperation', status,
                   upstream_status, route_attempt_count, provider_account_id,
                   route_trace->>'realCredentialRef')
                 FROM gateway_request_audits
                 WHERE access_key_id=$1 AND route_trace->>'storedResponseId'=$2
                 ORDER BY route_trace->>'storedResponseOperation'",
            )
            .bind(key_id)
            .bind(public_id)
            .fetch_all(pg)
            .await
            .unwrap();
            let create = |status| json!(["create", status, 200, 1, provider, credential]);
            if complete {
                assert_eq!(audits, vec![create("completed")]);
            } else {
                assert_eq!(
                    audits,
                    vec![
                        create("failed"),
                        json!(["retrieve", "completed", 200, 1, provider, credential])
                    ]
                );
            }
        }
    })
    .await;
    {
        let calls = peer.calls.lock().unwrap();
        let path = "/v2/inference/deployments/deployment1/responses";
        assert_eq!(calls.len(), 3);
        for (actual, headers, req) in &calls[..2] {
            assert_eq!(actual, path);
            assert_eq!(headers["accept"], "text/event-stream");
            assert_eq!(req["store"], true);
            assert_eq!(req["stream"], true);
        }
        assert_eq!(calls[2].0, format!("{path}/{second_id}"));
        assert!(calls[2].2.is_null());
    }
    assert_eq!(peer.oauth.lock().unwrap().len(), 2);
    finish(writer, reader, peer).await;
}

async fn request(
    state: &Arc<AppState>,
    method: &str,
    path: &str,
    token: &str,
    body: Body,
) -> axum::response::Response {
    tokio::time::timeout(
        Duration::from_secs(10),
        Box::pin(
            build_router(state.clone()).oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .body(body)
                    .unwrap(),
            ),
        ),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn frame(body: &mut Body) -> Value {
    let bytes = tokio::time::timeout(Duration::from_secs(10), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .into_data()
        .unwrap();
    let text = std::str::from_utf8(&bytes).unwrap();
    for private in ["sap-private-model", "never-publish", "resp_sap-stored-sse-"] {
        assert!(!text.contains(private));
    }
    serde_json::from_str(
        text.lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap(),
    )
    .unwrap()
}

async fn receipt(pg: &sqlx::PgPool, id: &str) -> Value {
    sqlx::query_scalar(
        "SELECT jsonb_build_object('upstream',r.upstream_id,'provider',r.provider_ref,
         'credential',r.credential_ref,'model',p.public_model,'source',p.source_key_ref,
         'platform',p.platform_access_ref,'reserved',h.reserved,'phase',r.phase,
         'settled',r.settled,'open',h.open,'usage',CASE WHEN r.input_tokens IS NOT NULL
         THEN jsonb_build_array(r.input_tokens,r.output_tokens) END)
         FROM gateway_sap_response_receipts r
         JOIN gateway_sap_response_route_pins p ON p.response_id=r.id
         JOIN gateway_sap_response_balance_holds h ON h.response_id=r.id WHERE r.id=$1",
    )
    .bind(id)
    .fetch_one(pg)
    .await
    .unwrap()
}

fn expected(binding: &Value, phase: &str, settled: i64, open: i64, usage: Value) -> Value {
    let mut row = binding.clone();
    row["phase"] = phase.into();
    row["settled"] = settled.into();
    row["open"] = open.into();
    row["usage"] = usage;
    row
}

async fn balance(pg: &sqlx::PgPool, key: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT remaining_messages FROM gateway_access_key_balances WHERE access_key_id=$1",
    )
    .bind(key)
    .fetch_one(pg)
    .await
    .unwrap()
}

fn snapshot(id: &str, status: &str) -> Value {
    let mut body = json!({"id":id,"object":"response","status":status,"store":true,
        "model":"sap-private-model","output":[],"metadata":{"private":"never-publish"}});
    if status == "completed" {
        body["usage"] = json!({"input_tokens":2,"output_tokens":3,"total_tokens":5});
    }
    body
}

fn stream_reply(id: &str, complete: bool) -> Vec<u8> {
    let event = |name, sequence, mut value: Value| {
        value["type"] = Value::String(name);
        value["sequence_number"] = Value::from(sequence);
        format!(
            "event: {}\ndata: {value}\n\n",
            value["type"].as_str().unwrap()
        )
    };
    let mut reply = event(
        "response.created".into(),
        0,
        json!({"response":snapshot(id,"in_progress")}),
    );
    reply.push_str(&event(
        "response.output_text.delta".into(),
        1,
        json!({"output_index":0,"content_index":0,"item_id":"msg_synthetic","delta":"synthetic"}),
    ));
    if complete {
        reply.push_str(&event(
            "response.completed".into(),
            2,
            json!({"response":snapshot(id,"completed")}),
        ));
    } else {
        reply.push_str("event: response.completed\ndata: {");
    }
    reply.into_bytes()
}
