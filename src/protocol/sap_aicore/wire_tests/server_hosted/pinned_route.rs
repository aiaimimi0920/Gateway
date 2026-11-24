//! Exercise Hosted grant freshness with the real access catalog and two database connections.
use super::*;
use crate::{
    auth::{adapter::AuthRequest, authenticate_request},
    db::access::pinned_route::{read_current, PinnedRoute},
};

#[test]
#[ignore = "Requires isolated Hosted PostgreSQL schema and Redis; retains scoped fixture data"]
fn sap_aicore_response_receipts_hosted_pinned_grant_bypasses_stale_cache() {
    run_hosted(hosted);
}

async fn hosted() {
    const SECRET: &str = "synthetic-sap-route-grant-fixture-secret";
    const ACCESS: &str = "/v1/internal/gateway/access";
    let alias = format!("sap-pinned-{}", uuid::Uuid::new_v4().simple());
    let upstream_id = format!("resp_{alias}");
    let peer = peer::Peer::stored_responses(upstream_id.clone()).await;
    let writer = build_with_key_secret(&alias, "writer", &peer, Some(SECRET)).await;
    let reader = build_with_key_secret(&alias, "reader", &peer, Some(SECRET)).await;
    let (provider, credential) = provision(
        &writer,
        &alias,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
        json!({"api_key":KEY}),
    )
    .await;
    // Grant the actual deployment, then resolve it via its project alias.
    let capability = manage(
        &writer,
        &format!("{ACCESS}/provider-capabilities"),
        json!({
            "modelCode":UPSTREAM_MODEL,"endpointKind":"responses","upstreamModel":UPSTREAM_MODEL,
            "providerAccountId":provider,"enabled":true
        }),
    )
    .await;
    let catalog = manage(
        &writer,
        &format!("{ACCESS}/platform-access"),
        json!({
            "modelCode":UPSTREAM_MODEL,"endpointKind":"responses","upstreamModel":UPSTREAM_MODEL,
            "providerCapabilityId":capability["id"],"platformTier":"standard","status":"active",
            "operatorWeight":1,"routingPriority":0,"enabledForSale":true
        }),
    )
    .await;
    let bundle = manage(&writer, &format!("{ACCESS}/bundles"), json!({
        "projectId":PROJECT,"slug":alias,"displayName":alias,"status":"active","billingMode":"message_prepaid"
    })).await;
    let bundle_id = bundle["id"].as_str().unwrap();
    manage(
        &writer,
        &format!("{ACCESS}/bundles/{bundle_id}/items/replace"),
        json!({
            "platformAccessIds":[catalog["id"]]
        }),
    )
    .await;
    let db = writer.pg_pool.as_ref().unwrap();
    // Older isolated Hosted fixtures bootstrap only through 007.
    for sql in [
        crate::protocol::sap_aicore::responses_lifecycle::store::SCHEMA,
        crate::protocol::sap_aicore::responses_lifecycle::store::MUTATION_SCHEMA,
        crate::protocol::sap_aicore::responses_lifecycle::store::ROUTE_SCHEMA,
        crate::protocol::sap_aicore::responses_lifecycle::store::RECOVERY_SCHEMA,
    ] {
        sqlx::raw_sql(sql).execute(db).await.unwrap();
    }
    let tenant: String = sqlx::query_scalar("SELECT tenant_id FROM gateway_projects WHERE id = $1")
        .bind(PROJECT)
        .fetch_one(db)
        .await
        .unwrap();
    let key_request = |kind: &str, name: &str, bundles: Value| {
        json!({
            "ownerType":"user","ownerId":alias,"resolvedProjectId":PROJECT,"resolvedTenantId":tenant,
            "keyKind":kind,"publicKeyPrefix":"neuro","displayName":name,"bundleIds":bundles
        })
    };
    let mut ids = Vec::new();
    for name in ["original", "alternative"] {
        let key = manage(
            &writer,
            &format!("{ACCESS}/keys"),
            key_request("normal", name, json!([bundle_id])),
        )
        .await;
        ids.push(key["id"].as_str().unwrap().to_owned());
    }
    let aggregate = manage(
        &writer,
        &format!("{ACCESS}/keys"),
        key_request("auto_route", "aggregate", json!([])),
    )
    .await;
    let aggregate_id = aggregate["id"].as_str().unwrap();
    manage(&writer, &format!("{ACCESS}/keys/{aggregate_id}/aggregate-memberships/replace"), json!({
        "memberships":[{"memberAccessKeyId":ids[0],"priority":1},{"memberAccessKeyId":ids[1],"priority":2}]
    })).await;
    let session = authenticate_request(
        &reader,
        &AuthRequest {
            authorization: Some(format!("Bearer {}", aggregate["token"].as_str().unwrap())),
            api_key: None,
            path: "/v1/responses".into(),
            method: "POST".into(),
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(session.access_key_id.as_deref(), Some(aggregate_id));
    let pg = reader.pg_pool.as_ref().unwrap();
    let models = crate::db::list_models_for_access_key(pg, &reader.redis_pool, aggregate_id)
        .await
        .unwrap();
    assert!(models.iter().any(|model| model.id == alias));
    let pin = |source| PinnedRoute {
        model: &alias,
        source_key: source,
        platform_access: catalog["id"].as_str().unwrap(),
        provider: &provider,
        credential: &credential,
        account_group: None,
    };
    let (payload, model) = read_current(pg, &session, &reader.route_config, pin(&ids[0]))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(payload.credential_id.as_deref(), Some(credential.as_str()));
    assert_eq!(model, UPSTREAM_MODEL);
    assert!(peer.calls.lock().unwrap().is_empty());
    assert!(peer.oauth.lock().unwrap().is_empty());
    let (bound, path) = exercise_public(
        &writer,
        &reader,
        &peer,
        &alias,
        &ids,
        &aggregate,
        &upstream_id,
    )
    .await;
    let other = ids.iter().find(|id| id.as_str() != bound).unwrap();
    // No mutation of the projection version/cache: expiry also occurs without either.
    sqlx::query(
        "UPDATE gateway_access_keys SET expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(&bound)
    .execute(db)
    .await
    .unwrap();
    assert!(
        read_current(pg, &session, &reader.route_config, pin(&bound))
            .await
            .unwrap()
            .is_none()
    );
    assert!(read_current(pg, &session, &reader.route_config, pin(other))
        .await
        .unwrap()
        .is_some());
    let token = aggregate["token"].as_str().unwrap();
    let (status, _) = retrieve(&reader, &path, token).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("UPDATE gateway_provider_credentials SET status = 'disabled' WHERE id = $1")
        .bind(&credential)
        .execute(db)
        .await
        .unwrap();
    assert!(read_current(pg, &session, &reader.route_config, pin(other))
        .await
        .unwrap()
        .is_none());
    assert_eq!(peer.calls.lock().unwrap().len(), 2);
    finish(writer, reader, peer).await;
}

async fn exercise_public(
    writer: &Arc<AppState>,
    reader: &Arc<AppState>,
    peer: &peer::Peer,
    alias: &str,
    ids: &[String],
    aggregate: &Value,
    upstream_id: &str,
) -> (String, String) {
    for key_id in ids {
        manage(
            writer,
            &format!("/v1/internal/gateway/access/keys/{key_id}/balances/adjust"),
            json!({
                "balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2
            }),
        )
        .await;
    }
    let token = aggregate["token"].as_str().unwrap();
    let db = writer.pg_pool.as_ref().unwrap();
    test_support::origin(peer.origin.clone(), async {
        let (status, body) = call(writer, "/v1/responses", token, json!({
            "model":alias,"input":"synthetic","store":true,"background":true
        })).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["status"], "queued");
        let id = body["id"].as_str().unwrap();
        assert!(id.starts_with("resp_gw_"));
        assert_ne!(id, upstream_id);
        let path = format!("/v1/responses/{id}");
        let (status, body) = retrieve(reader, &path, token).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        let body: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(body["id"], id);
        assert_eq!(body["usage"]["total_tokens"], 5);
        let (bound, settled, phase): (String, i64, String) = sqlx::query_as(
            "SELECT p.source_key_ref, r.settled, r.phase FROM gateway_sap_response_receipts r
             JOIN gateway_sap_response_route_pins p ON p.response_id = r.id WHERE r.id = $1",
        ).bind(id).fetch_one(db).await.unwrap();
        assert_eq!((settled, phase.as_str()), (1, "completed"));
        for key_id in ids {
            let remaining: i64 = sqlx::query_scalar(
                "SELECT remaining_messages FROM gateway_access_key_balances WHERE access_key_id = $1",
            ).bind(key_id).fetch_one(db).await.unwrap();
            assert_eq!(remaining, if *key_id == bound { 1 } else { 2 });
        }
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].2["store"], true);
        assert_eq!(calls[0].2["background"], true);
        assert!(calls[1].0.ends_with(&format!("/{upstream_id}")));
        (bound, path)
    }).await
}

async fn retrieve(state: &Arc<AppState>, path: &str, token: &str) -> (StatusCode, String) {
    let response = Box::pin(
        build_router(state.clone()).oneshot(
            Request::get(path)
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    let status = response.status();
    (status, drain(response.into_body()).await)
}
