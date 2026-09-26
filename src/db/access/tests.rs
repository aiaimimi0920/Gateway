use super::*;
mod integration_fixtures;
use integration_fixtures::{
    access_integration_pool, access_integration_redis_pool, reset_projection_integration_schema,
    reset_rotation_integration_schema, seed_rotation_access_key,
};

fn make_projection_row(
    model_code: &str,
    upstream_model: Option<&str>,
) -> ProjectedPlatformAccessRow {
    ProjectedPlatformAccessRow {
        requesting_access_key_id: "ak-1".to_string(),
        source_access_key_id: "ak-1".to_string(),
        platform_access_id: "pa-1".to_string(),
        provider_account_id: "provider-1".to_string(),
        model_code: model_code.to_string(),
        endpoint_kind: "responses".to_string(),
        upstream_model: upstream_model.map(str::to_string),
        platform_tier: "high".to_string(),
        operator_weight: 100,
        routing_priority: 10,
    }
}

fn make_access_key_auth() -> AccessKeyAuthRecord {
    AccessKeyAuthRecord {
        id: "ak-1".to_string(),
        owner_type: "project".to_string(),
        owner_id: "project-a".to_string(),
        resolved_project_id: "project-a".to_string(),
        resolved_tenant_id: "tenant-a".to_string(),
        key_kind: "normal".to_string(),
        status: "active".to_string(),
        public_key_prefix: "gw-project".to_string(),
        display_name: "test".to_string(),
        external_key: None,
        expires_at: None,
        metadata: None,
        legacy_gateway_api_key_id: None,
        legacy_user_credential_id: None,
        projection_version: "2026-07-19T00:00:00Z".to_string(),
    }
}

#[tokio::test]
#[ignore = "requires isolated GATEWAY_ACCESS_TEST_DATABASE_URL PostgreSQL fixture"]
async fn normal_projection_excludes_cross_project_bundle_bindings() {
    let pool = access_integration_pool().await;
    reset_projection_integration_schema(&pool).await;
    sqlx::raw_sql(
        r#"
        insert into gateway_access_keys (id, resolved_project_id)
        values ('key-a', 'project-a');

        insert into gateway_provider_capability_catalog (id, provider_account_id, enabled)
        values ('cap-a', 'provider-a', true);

        insert into gateway_platform_access_catalog (
          id, provider_capability_id, model_code, endpoint_kind, upstream_model, platform_tier,
          status, operator_weight, routing_priority, enabled_for_sale
        ) values
          ('access-project-a', 'cap-a', 'model-a', 'responses', 'model-a', 'high', 'active', 10, 100, true),
          ('access-global', 'cap-a', 'model-global', 'responses', 'model-global', 'high', 'active', 10, 100, true),
          ('access-project-b', 'cap-a', 'model-b', 'responses', 'model-b', 'high', 'active', 10, 100, true);

        insert into gateway_access_bundles (id, project_id, status) values
          ('bundle-a', 'project-a', 'active'),
          ('bundle-global', null, 'active'),
          ('bundle-b', 'project-b', 'active');

        insert into gateway_access_bundle_items (bundle_id, platform_access_id) values
          ('bundle-a', 'access-project-a'),
          ('bundle-global', 'access-global'),
          ('bundle-b', 'access-project-b');

        insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id) values
          ('key-a', 'bundle-a'),
          ('key-a', 'bundle-global'),
          ('key-a', 'bundle-b');
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed projection boundary fixture");

    let platform_access_ids = build_projection_rows_for_normal_key(&pool, "key-a")
        .await
        .expect("build normal access projection")
        .into_iter()
        .map(|row| row.platform_access_id)
        .collect::<BTreeSet<_>>();

    assert_eq!(
        platform_access_ids,
        BTreeSet::from(["access-global".to_string(), "access-project-a".to_string(),])
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and Redis access fixtures"]
async fn rotation_preserves_balances_memberships_and_projection_versions() {
    let pool = access_integration_pool().await;
    let redis_pool = access_integration_redis_pool();
    reset_rotation_integration_schema(&pool).await;
    seed_rotation_access_key(&pool, "old-key", "project-a", "tenant-a", "auto_route").await;
    seed_rotation_access_key(&pool, "member-key", "project-a", "tenant-a", "normal").await;
    seed_rotation_access_key(&pool, "parent-key", "project-a", "tenant-a", "auto_route").await;
    sqlx::raw_sql(
        r#"
        update gateway_access_keys
        set legacy_gateway_api_key_id = 'legacy-api-key',
            legacy_user_credential_id = 'legacy-user-credential'
        where id = 'old-key';
        insert into gateway_access_bundles (id, project_id, status)
        values ('bundle-a', 'project-a', 'active');
        insert into gateway_access_key_bundle_bindings (access_key_id, bundle_id, created_at)
        values ('old-key', 'bundle-a', now());
        insert into gateway_access_key_balances (
          access_key_id, balance_mode, status, total_tokens, remaining_tokens,
          total_messages, remaining_messages, updated_at
        ) values ('old-key', 'token_prepaid', 'active', 1000, 725, 40, 29, now());
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        ) values
          ('old-key', 'member-key', 7, now()),
          ('parent-key', 'old-key', 9, now());
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed rotation relationship fixture");

    let rotated = rotate_access_key(&pool, &redis_pool, "old-key", None)
        .await
        .expect("rotate access key");

    let old_status: String =
        sqlx::query_scalar("select status from gateway_access_keys where id = 'old-key'")
            .fetch_one(&pool)
            .await
            .expect("read old key status");
    assert_eq!(old_status, "revoked");

    let legacy_ids = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "select legacy_gateway_api_key_id, legacy_user_credential_id from gateway_access_keys where id = $1",
    )
    .bind(&rotated.id)
    .fetch_one(&pool)
    .await
    .expect("rotated key legacy identifiers must be preserved");
    assert_eq!(
        legacy_ids,
        (
            Some("legacy-api-key".to_string()),
            Some("legacy-user-credential".to_string()),
        )
    );
    let old_legacy_ids = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        "select legacy_gateway_api_key_id, legacy_user_credential_id from gateway_access_keys where id = 'old-key'",
    )
    .fetch_one(&pool)
    .await
    .expect("read old key legacy identifiers");
    assert_eq!(old_legacy_ids, (None, None));

    let balance = sqlx::query_as::<
        _,
        (
            String,
            String,
            Option<i64>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        ),
    >(
        r#"
        select balance_mode, status, total_tokens, remaining_tokens, total_messages, remaining_messages
        from gateway_access_key_balances where access_key_id = $1
        "#,
    )
    .bind(&rotated.id)
    .fetch_one(&pool)
    .await
    .expect("rotated key balance must be preserved");
    assert_eq!(
        balance,
        (
            "token_prepaid".to_string(),
            "active".to_string(),
            Some(1000),
            Some(725),
            Some(40),
            Some(29),
        )
    );

    let bundle_count: i64 = sqlx::query_scalar(
        "select count(*) from gateway_access_key_bundle_bindings where access_key_id = $1 and bundle_id = 'bundle-a'",
    )
    .bind(&rotated.id)
    .fetch_one(&pool)
    .await
    .expect("read rotated bundle binding");
    assert_eq!(bundle_count, 1);

    let outgoing_priority: i32 = sqlx::query_scalar(
        "select priority from gateway_access_key_aggregate_memberships where aggregate_access_key_id = $1 and member_access_key_id = 'member-key'",
    )
    .bind(&rotated.id)
    .fetch_one(&pool)
    .await
    .expect("rotated aggregate membership must be preserved");
    assert_eq!(outgoing_priority, 7);

    let incoming_priority: i32 = sqlx::query_scalar(
        "select priority from gateway_access_key_aggregate_memberships where aggregate_access_key_id = 'parent-key' and member_access_key_id = $1",
    )
    .bind(&rotated.id)
    .fetch_one(&pool)
    .await
    .expect("rotated member relationship must be preserved");
    assert_eq!(incoming_priority, 9);

    let stale_projection_versions: i64 = sqlx::query_scalar(
        "select count(*) from gateway_access_keys where updated_at <= '1970-01-01T00:00:00Z'::timestamptz",
    )
    .fetch_one(&pool)
    .await
    .expect("read projection versions");
    assert_eq!(stale_projection_versions, 0);
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL and Redis access fixtures"]
async fn rotation_rolls_back_revoke_when_relationship_copy_fails() {
    let pool = access_integration_pool().await;
    let redis_pool = access_integration_redis_pool();
    reset_rotation_integration_schema(&pool).await;
    seed_rotation_access_key(&pool, "old-key", "project-a", "tenant-a", "auto_route").await;
    seed_rotation_access_key(&pool, "member-key", "project-a", "tenant-a", "normal").await;
    sqlx::query(
        r#"
        insert into gateway_access_key_aggregate_memberships (
          aggregate_access_key_id, member_access_key_id, priority, created_at
        ) values ('old-key', 'member-key', 7, now())
        "#,
    )
    .execute(&pool)
    .await
    .expect("seed rollback membership");
    sqlx::raw_sql(
        r#"
        create or replace function reject_rotated_membership_copy() returns trigger as $$
        begin
          if new.aggregate_access_key_id <> 'old-key' then
            raise exception 'forced rotated membership copy failure';
          end if;
          return new;
        end;
        $$ language plpgsql;
        create trigger reject_rotated_membership_copy_trigger
        before insert on gateway_access_key_aggregate_memberships
        for each row execute function reject_rotated_membership_copy();
        "#,
    )
    .execute(&pool)
    .await
    .expect("install forced relationship-copy failure");

    rotate_access_key(&pool, &redis_pool, "old-key", None)
        .await
        .expect_err("relationship-copy failure must abort the entire rotation");

    let old_status: String =
        sqlx::query_scalar("select status from gateway_access_keys where id = 'old-key'")
            .fetch_one(&pool)
            .await
            .expect("read old key status after rollback");
    assert_eq!(old_status, "active");
    let replacement_count: i64 = sqlx::query_scalar(
        "select count(*) from gateway_access_keys where rotated_from_access_key_id = 'old-key'",
    )
    .fetch_one(&pool)
    .await
    .expect("count rolled-back replacement keys");
    assert_eq!(replacement_count, 0);
}

#[test]
fn cache_projection_rejects_scope_or_version_mismatch() {
    let access_key = make_access_key_auth();
    let projection = group_projection_rows_by_model(&access_key, Vec::new());
    assert!(cache_projection_is_compatible(&projection, &access_key));

    let mut wrong_scope = projection.clone();
    wrong_scope.resolved_project_id = "project-b".to_string();
    assert!(!cache_projection_is_compatible(&wrong_scope, &access_key));

    let mut stale_version = projection;
    stale_version.projection_version = "2026-07-18T00:00:00Z".to_string();
    assert!(!cache_projection_is_compatible(&stale_version, &access_key));
}

#[test]
fn expand_projection_rows_with_aliases_adds_alias_entry() {
    let rows = vec![make_projection_row("gpt-5.4", Some("gpt-5.4"))];
    let aliases = vec![ModelAliasProjectionRow {
        alias: "gpt-5".to_string(),
        provider_account_id: "provider-1".to_string(),
        upstream_model: Some("gpt-5.4".to_string()),
    }];

    let expanded = expand_projection_rows_with_aliases(rows, &aliases);
    assert!(expanded.iter().any(|row| row.model_code == "gpt-5.4"));
    assert!(expanded.iter().any(|row| row.model_code == "gpt-5"));
}

#[test]
fn expand_projection_rows_with_aliases_ignores_provider_mismatch() {
    let rows = vec![make_projection_row("gpt-5.4", Some("gpt-5.4"))];
    let aliases = vec![ModelAliasProjectionRow {
        alias: "gpt-5".to_string(),
        provider_account_id: "provider-2".to_string(),
        upstream_model: Some("gpt-5.4".to_string()),
    }];

    let expanded = expand_projection_rows_with_aliases(rows, &aliases);
    assert!(!expanded.iter().any(|row| row.model_code == "gpt-5"));
}

#[test]
fn credential_route_row_filter_honors_supported_models() {
    let row = make_projection_row("qwen3.5-35b-a3b", Some("astron-code-latest"));
    let payload = serde_json::json!({
        "supportedModels": ["qwen3.5-35b-a3b"]
    });
    assert!(credential_payload_supports_route_row(&payload, &row));

    let mismatched = serde_json::json!({
        "supportedModels": ["qwen3.5-2b"]
    });
    assert!(!credential_payload_supports_route_row(&mismatched, &row));
}

#[test]
fn collect_model_catalog_ids_from_access_projection_returns_sorted_alias_only_models() {
    let mut rows = vec![
        make_projection_row("gpt-5.4", Some("gpt-5.4")),
        make_projection_row("gpt-5.4", Some("gpt-5.4")),
        make_projection_row("claude-sonnet-4-6", Some("claude-sonnet-4-6")),
    ];
    rows.push(ProjectedPlatformAccessRow {
        model_code: "gpt-5".to_string(),
        ..make_projection_row("gpt-5.4", Some("gpt-5.4"))
    });

    let projection = group_projection_rows_by_model(&make_access_key_auth(), rows);
    let model_ids = collect_model_catalog_ids_from_access_projection(&projection);

    assert_eq!(
        model_ids,
        vec!["claude-sonnet-4-6".to_string(), "gpt-5".to_string()]
    );
}

#[test]
fn collect_model_catalog_ids_from_access_projection_hides_upstream_model_when_alias_exists() {
    let rows = vec![
        make_projection_row(
            "nvidia/llama-3.3-nemotron-super-49b-v1.5",
            Some("nvidia/llama-3.3-nemotron-super-49b-v1.5"),
        ),
        ProjectedPlatformAccessRow {
            model_code: "nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string(),
            ..make_projection_row(
                "nvidia/llama-3.3-nemotron-super-49b-v1.5",
                Some("nvidia/llama-3.3-nemotron-super-49b-v1.5"),
            )
        },
    ];

    let projection = group_projection_rows_by_model(&make_access_key_auth(), rows);
    let model_ids = collect_model_catalog_ids_from_access_projection(&projection);

    assert_eq!(
        model_ids,
        vec!["nvidia-llama-3-3-nemotron-super-49b-v1-5".to_string()]
    );
}
