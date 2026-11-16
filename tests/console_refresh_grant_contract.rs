//! Durable grant contracts use synthetic material and the real local authority/atomic file owner.
use neuro_gateway::console::{refresh_grants::*, *};
use neuro_gateway::routing::config::RouteConfigYaml;

struct Fixture {
    persistence: RouteConfigPersistence,
    revision: RouteConfigRedisRevision,
    runtime: RouteConfigRuntime,
}

impl Fixture {
    fn new() -> Self {
        Self::new_for_endpoint("https://issuer.invalid/token")
    }

    fn new_for_endpoint(endpoint: &str) -> Self {
        let root = std::env::temp_dir().join(format!("refresh-grant-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let routes = root.join("routes.yaml");
        let document: RouteConfigYaml = serde_json::from_value(serde_json::json!({
            "providers":[{"id":"oauth","base_url":"https://example.invalid/v1",
                "credentials":[{"id":"account","api_key":"synthetic-access",
                    "refresh_token":"synthetic-refresh","refresh_endpoint":endpoint,
                    "refresh_client_id":"synthetic-client","expires_at":"2020-01-01T00:00:00Z"}]}]
        }))
        .unwrap();
        std::fs::write(&routes, serde_yaml::to_string(&document).unwrap()).unwrap();
        let config = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(root.join("state")),
            routes_file: Some(routes),
            ..Default::default()
        })
        .unwrap();
        let runtime = RouteConfigRuntime::new_local(&config).unwrap();
        let snapshot = runtime.route_config().snapshot();
        Self {
            persistence: RouteConfigPersistence::new(&config.state_dir, &config.routes_file)
                .unwrap(),
            revision: RouteConfigRedisRevision::new(snapshot.revision().clone(), document).unwrap(),
            runtime,
        }
    }

    fn request(&self) -> GrantRequest {
        GrantRequest::from_revision(&self.revision, "oauth", "account").unwrap()
    }

    async fn publish(&self, access: &str, refresh: &str, expiry: &str) -> RouteConfigRedisRevision {
        let mut document = self.revision.document().clone();
        let credential = &mut document.providers[0].credentials[0];
        credential.api_key = Some(access.into());
        credential.refresh_token = Some(refresh.into());
        credential.expires_at = Some(expiry.into());
        credential.token_expires_in_secs = Some(3600);
        let saved = self
            .runtime
            .commit_document(self.revision.metadata().id(), document.clone(), None)
            .await
            .unwrap();
        RouteConfigRedisRevision::new(saved.revision().clone(), document).unwrap()
    }
}

#[path = "console_refresh_grant_contract/redis.rs"]
mod redis_contract;

#[path = "console_refresh_grant_contract/receipt.rs"]
mod receipt_contract;

fn metadata_variant(revision: &RouteConfigRedisRevision) -> RouteConfigRedisRevision {
    let mut metadata = serde_json::to_value(revision.metadata()).unwrap();
    metadata["timestamp"] = serde_json::json!("1999-01-01T00:00:00Z");
    metadata["message"] = serde_json::json!("fresh bootstrap metadata, unchanged authorization");
    let metadata: neuro_gateway::console::revision::RevisionMetadata =
        serde_json::from_value(metadata).unwrap();
    RouteConfigRedisRevision::new(metadata, revision.document().clone()).unwrap()
}

#[test]
fn local_bootstrap_metadata_difference_does_not_change_the_authorized_document() {
    let f = Fixture::new();
    let expected = metadata_variant(&f.revision);
    assert_ne!(f.revision.to_json().unwrap(), expected.to_json().unwrap());
    let request = GrantRequest::from_revision(&expected, "oauth", "account").unwrap();
    let guard = f.persistence.try_writer_lock().unwrap();
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &expected, &request)
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
}

#[tokio::test]
async fn same_tokens_with_extended_expiry_have_a_safe_published_successor_and_permanent_history() {
    let f = Fixture::new();
    let request = f.request();
    let saved = {
        let guard = f.persistence.try_writer_lock().unwrap();
        let GrantReservation::Reserved(record) = f
            .persistence
            .reserve_refresh_grant_locked(&guard, &f.revision, &request)
            .unwrap()
        else {
            panic!("not reserved")
        };
        let result = request
            .result("synthetic-access", None, 3600, "2030-01-01T00:00:00Z", None)
            .unwrap();
        f.persistence
            .store_refresh_grant_result_locked(&guard, &record, &result)
            .unwrap()
    };
    let next = f
        .publish(
            "synthetic-access",
            "synthetic-refresh",
            "2030-01-01T00:00:00Z",
        )
        .await;
    let next_request = GrantRequest::from_revision(&next, "oauth", "account").unwrap();
    assert_eq!(request.family_id(), next_request.family_id());
    assert_ne!(request.material_id(), next_request.material_id());
    let guard = f.persistence.try_writer_lock().unwrap();
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &next, &next_request)
            .unwrap(),
        GrantReservation::Blocked
    ));
    let published = f
        .persistence
        .mark_refresh_grant_published_locked(&guard, &next, &saved)
        .unwrap();
    assert_eq!(published.phase(), GrantPhase::Published);
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &next, &next_request)
            .unwrap(),
        GrantReservation::Reserved(_)
    ));
    let old_path = f
        .persistence
        .state_root()
        .join("refresh-grants")
        .join(request.family_id())
        .join(format!("{}.json", request.material_id()));
    assert_eq!(
        GrantRecord::from_json(&std::fs::read_to_string(old_path).unwrap()).unwrap(),
        published
    );
}

#[tokio::test]
async fn editing_expiry_or_access_cannot_escape_an_unknown_refresh_token_family() {
    let f = Fixture::new();
    let request = f.request();
    {
        let guard = f.persistence.try_writer_lock().unwrap();
        assert!(matches!(
            f.persistence
                .reserve_refresh_grant_locked(&guard, &f.revision, &request)
                .unwrap(),
            GrantReservation::Reserved(_)
        ));
    }
    let next = f
        .publish("other-access", "synthetic-refresh", "2030-01-01T00:00:00Z")
        .await;
    let request = GrantRequest::from_revision(&next, "oauth", "account").unwrap();
    let guard = f.persistence.try_writer_lock().unwrap();
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &next, &request)
            .unwrap(),
        GrantReservation::Blocked
    ));
    assert!(matches!(
        f.persistence
            .reserve_refresh_grant_locked(&guard, &f.revision, &f.request())
            .unwrap(),
        GrantReservation::Stale
    ));
}

#[test]
fn dangling_or_corrupt_head_never_becomes_an_absent_intent() {
    let f = Fixture::new();
    let request = f.request();
    let dir = f
        .persistence
        .state_root()
        .join("refresh-grants")
        .join(request.family_id());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("head.json"),
        serde_json::json!({"material":request.material_id()}).to_string(),
    )
    .unwrap();
    let guard = f.persistence.try_writer_lock().unwrap();
    assert!(f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &request)
        .is_err());
    std::fs::write(dir.join("head.json"), "{\"material\":\"../escape\"}").unwrap();
    assert!(f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &request)
        .is_err());
}

#[test]
fn abandoned_send_intent_survives_new_persistence_and_never_becomes_a_lease() {
    let f = Fixture::new();
    let request = f.request();
    let first = {
        let guard = f.persistence.try_writer_lock().unwrap();
        match f
            .persistence
            .reserve_refresh_grant_locked(&guard, &f.revision, &request)
            .unwrap()
        {
            GrantReservation::Reserved(record) => record,
            other => panic!("expected first reservation: {other:?}"),
        }
    };
    let reopened = RouteConfigPersistence::new(
        f.persistence.state_root().parent().unwrap(),
        f.persistence.routes_path(),
    )
    .unwrap();
    let guard = reopened.try_writer_lock().unwrap();
    let second = reopened
        .reserve_refresh_grant_locked(&guard, &f.revision, &request)
        .unwrap();
    assert!(matches!(second, GrantReservation::Existing(ref record) if record == &first));
    assert_eq!(first.phase(), GrantPhase::Sending);
    assert!(!format!("{first:?}").contains("synthetic-refresh"));
    assert!(!format!("{first:?}").contains("synthetic-access"));
}

#[test]
fn result_is_immutable_replayable_and_cannot_claim_publication_before_authority() {
    let f = Fixture::new();
    let request = f.request();
    let guard = f.persistence.try_writer_lock().unwrap();
    let GrantReservation::Reserved(first) = f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &request)
        .unwrap()
    else {
        panic!("not reserved")
    };
    let result = request
        .result(
            "rotated-access",
            Some("rotated-refresh"),
            3600,
            "2030-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    let saved = f
        .persistence
        .store_refresh_grant_result_locked(&guard, &first, &result)
        .unwrap();
    assert_eq!(saved.phase(), GrantPhase::Result);
    assert_eq!(
        GrantRecord::from_json(&saved.to_json().unwrap()).unwrap(),
        saved
    );
    assert_eq!(
        f.persistence
            .store_refresh_grant_result_locked(&guard, &first, &result)
            .unwrap(),
        saved
    );
    let conflicting = request
        .result(
            "other-access",
            Some("other-refresh"),
            3600,
            "2030-01-01T00:00:00Z",
            None,
        )
        .unwrap();
    assert!(f
        .persistence
        .store_refresh_grant_result_locked(&guard, &first, &conflicting)
        .is_err());
    assert!(f
        .persistence
        .mark_refresh_grant_published_locked(&guard, &f.revision, &saved)
        .is_err());
    let GrantReservation::Existing(replayed) = f
        .persistence
        .reserve_refresh_grant_locked(&guard, &f.revision, &request)
        .unwrap()
    else {
        panic!("not replayed")
    };
    assert_eq!(replayed, saved);
}

#[test]
fn malformed_unknown_and_oversized_records_fail_closed_without_secret_errors() {
    for raw in [
        "{}".to_owned(),
        "{\"version\":2}".to_owned(),
        "x".repeat(524_289),
    ] {
        let error = GrantRecord::from_json(&raw).unwrap_err();
        assert!(!error.to_string().contains(&raw));
    }
    let f = Fixture::new();
    assert!(GrantRequest::from_revision(&f.revision, "other-provider", "account").is_err());
    let request = f.request();
    assert!(request
        .result("", None, 3600, "2030-01-01T00:00:00Z", None)
        .is_err());
    assert!(request
        .result("valid", None, 0, "2030-01-01T00:00:00Z", None)
        .is_err());
    assert!(request
        .result("valid", None, 3600, "not-a-date", None)
        .is_err());
}
