//! Actual Hosted send evidence updates only its exact health row and never proves a refund.
use super::*;
use crate::protocol::sap_aicore::responses_lifecycle::store;

#[test]
#[ignore = "Requires dedicated PostgreSQL/Redis; retains scoped receipt, balance and health evidence"]
fn sap_aicore_server_stored_http_failure_retains_hold_and_exact_health() {
    run_hosted(hosted);
}

async fn hosted() {
    const SECRET: &str = "synthetic-sap-stored-health-key-secret";
    let scope = format!("sap-stored-health-{}", uuid::Uuid::new_v4().simple());
    let peer = peer::Peer::new("responses", false, false, 503).await;
    let writer = build_with_key_secret(&scope, "writer", &peer, Some(SECRET)).await;
    let reader = build_with_key_secret(&scope, "reader", &peer, Some(SECRET)).await;
    let (provider, credential) = provision(&writer, &scope,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
        json!({"api_key":KEY})).await;
    let (key, platform) = grant(&writer, &scope, &provider).await;
    let key_id = key["id"].as_str().unwrap();
    for sql in [
        store::SCHEMA,
        store::MUTATION_SCHEMA,
        store::ROUTE_SCHEMA,
        store::RECOVERY_SCHEMA,
    ] {
        sqlx::raw_sql(sql)
            .execute(writer.pg_pool.as_ref().unwrap())
            .await
            .unwrap();
    }
    manage(
        &writer,
        &format!("/v1/internal/gateway/access/keys/{key_id}/balances/adjust"),
        json!({"balanceMode":"message_prepaid","totalMessages":2,"remainingMessages":2}),
    )
    .await;
    let (status, text) = test_support::origin(
        peer.origin.clone(),
        call(
            &writer,
            "/v1/responses",
            key["token"].as_str().unwrap(),
            json!({"model":scope,"input":"synthetic","store":true}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{text}");
    assert!(
        text.contains("sap_aicore_response_delivery_unknown"),
        "{text}"
    );
    assert_eq!(peer.calls.lock().unwrap().len(), 1);
    assert_eq!(peer.oauth.lock().unwrap().len(), 1);

    // Read through the independent AppState, not the writer's in-memory projection.
    let pg = reader.pg_pool.as_ref().unwrap();
    let rows: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('id',r.id,'phase',r.phase,'upstream',r.upstream_id,
          'settled',r.settled,'credential',r.credential_ref,'model',p.public_model,
          'source',p.source_key_ref,'platform',p.platform_access_ref,'key',h.key_ref,
          'account',h.account_ref,'mode',h.mode,'reserved',h.reserved,'open',h.open)
         FROM gateway_sap_response_receipts r
         JOIN gateway_sap_response_route_pins p ON p.response_id=r.id
         JOIN gateway_sap_response_balance_holds h ON h.response_id=r.id
         WHERE r.provider_ref=$1",
    )
    .bind(&provider)
    .fetch_all(pg)
    .await
    .unwrap();
    assert_eq!(rows.len(), 1);
    let id = rows[0]["id"].as_str().unwrap();
    assert!(id.starts_with("resp_gw_"));
    assert_eq!(
        rows[0],
        json!({"id":id,"phase":"submitting","upstream":null,"settled":0,
        "credential":credential,"model":scope,"source":key_id,"platform":platform,
        "key":key_id,"account":key_id,"mode":"message_prepaid","reserved":1,"open":1})
    );
    let balance: (String, Option<i64>, Option<i64>) = sqlx::query_as(
        "SELECT balance_mode,total_messages,remaining_messages
         FROM gateway_access_key_balances WHERE access_key_id=$1",
    )
    .bind(key_id)
    .fetch_one(pg)
    .await
    .unwrap();
    assert_eq!(balance, ("message_prepaid".into(), Some(2), Some(1)));
    let unsent: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM gateway_sap_response_unsent_proofs WHERE response_id=$1)",
    )
    .bind(id)
    .fetch_one(pg)
    .await
    .unwrap();
    assert!(
        !unsent,
        "HTTP rejection cannot prove that a request was never sent"
    );

    let health = db::list_provider_credential_model_states(
        pg,
        db::CredentialModelStateFilters {
            provider_account_id: Some(provider.clone()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(health.len(), 1);
    let health = &health[0];
    assert_eq!(
        health.id,
        db::provider_credential_model_state_id(
            &provider,
            Some(&credential),
            Some(&credential),
            Some(super::super::super::PROFILE),
            UPSTREAM_MODEL
        )
    );
    assert_eq!(health.provider_account_id, provider);
    assert_eq!(
        health.provider_credential_id.as_deref(),
        Some(credential.as_str())
    );
    assert_eq!(
        health.provider_credential_ref.as_deref(),
        Some(credential.as_str())
    );
    assert_eq!(
        health.protocol_profile.as_deref(),
        Some(super::super::super::PROFILE)
    );
    assert_eq!(health.model, UPSTREAM_MODEL);
    assert_eq!(health.failure_count, 1);
    assert_eq!(health.last_upstream_status, Some(503));

    let audits: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('status',status,'upstream',upstream_status,'project',project_id,
          'provider',provider_account_id,'key',access_key_id,'source',source_access_key_id,
          'requested',requested_model,'resolved',resolved_model,'attempts',route_attempt_count,
          'tokens',total_tokens,'trace',route_trace)
         FROM gateway_request_audits WHERE access_key_id=$1",
    )
    .bind(key_id)
    .fetch_all(pg)
    .await
    .unwrap();
    assert_eq!(audits.len(), 1);
    let trace = audits[0]["trace"].clone();
    assert_eq!(
        audits[0],
        json!({"status":"failed","upstream":503,"project":PROJECT,
        "provider":provider,"key":key_id,"source":key_id,"requested":scope,
        "resolved":UPSTREAM_MODEL,"attempts":1,"tokens":null,"trace":trace})
    );
    assert_eq!(trace["realCredentialRef"], credential);
    assert_eq!(trace["routeAttemptCount"], 1);
    assert_eq!(trace["storedResponseOperation"], "create");
    assert_eq!(trace["storedResponseId"], id);
    finish(writer, reader, peer).await;
}

async fn grant(writer: &Arc<AppState>, scope: &str, provider: &str) -> (Value, String) {
    const ACCESS: &str = "/v1/internal/gateway/access";
    let capability = manage(
        writer,
        &format!("{ACCESS}/provider-capabilities"),
        json!({
            "modelCode":UPSTREAM_MODEL,"endpointKind":"responses","upstreamModel":UPSTREAM_MODEL,
            "providerAccountId":provider,"enabled":true
        }),
    )
    .await;
    let catalog = manage(
        writer,
        &format!("{ACCESS}/platform-access"),
        json!({
            "modelCode":UPSTREAM_MODEL,"endpointKind":"responses","upstreamModel":UPSTREAM_MODEL,
            "providerCapabilityId":capability["id"],"platformTier":"standard","status":"active",
            "operatorWeight":1,"routingPriority":0,"enabledForSale":true
        }),
    )
    .await;
    let platform = catalog["id"].as_str().unwrap().to_owned();
    let bundle = manage(writer, &format!("{ACCESS}/bundles"), json!({
        "projectId":PROJECT,"slug":scope,"displayName":scope,"status":"active","billingMode":"message_prepaid"
    })).await;
    let bundle_id = bundle["id"].as_str().unwrap();
    manage(
        writer,
        &format!("{ACCESS}/bundles/{bundle_id}/items/replace"),
        json!({"platformAccessIds":[platform]}),
    )
    .await;
    let tenant: String = sqlx::query_scalar("SELECT tenant_id FROM gateway_projects WHERE id=$1")
        .bind(PROJECT)
        .fetch_one(writer.pg_pool.as_ref().unwrap())
        .await
        .unwrap();
    let key = manage(writer, &format!("{ACCESS}/keys"), json!({
        "ownerType":"user","ownerId":scope,"resolvedProjectId":PROJECT,"resolvedTenantId":tenant,
        "keyKind":"normal","publicKeyPrefix":"neuro","displayName":"stored health",
        "bundleIds":[bundle_id]
    })).await;
    (key, platform)
}
