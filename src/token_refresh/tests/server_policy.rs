//! Policy changes use the normal authority; a closed cache retains durable pending writeback.
use super::grok_wire::{rotated, Mock};
use super::*;
use crate::routing::config::{CredentialPoolAuthPolicy, TokenRefreshState};
use serde_json::json;

pub(super) fn owner(store: &RouteConfigStore) -> Arc<TokenRefreshState> {
    store
        .snapshot()
        .oauth_refresh_target("build-pool", "exact-build")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap()
}
pub(super) async fn refresh(
    authority: &authority_fixture::Authority,
    mock: &Mock,
    pending: &mut Pending,
) {
    let pool = deadpool_redis::Config::from_url("redis://127.0.0.1:1")
        .create_pool(Some(deadpool_redis::Runtime::Tokio1))
        .unwrap();
    pool.close();
    super::super::grok::TEST_ISSUER
        .scope(
            mock.issuer.clone(),
            refresh_once(
                authority.runtime.route_config(),
                &client(),
                Some(&authority.runtime),
                Some(&pool),
                pending,
                Duration::from_secs(300),
                None,
            ),
        )
        .await;
}
pub(super) async fn edit(authority: &authority_fixture::Authority, mode: &str) {
    let snapshot = authority.runtime.route_config().snapshot();
    let mut document = snapshot.document().clone();
    match mode {
        "policy" => {
            document.providers[0].credential_pool_auth_policy = CredentialPoolAuthPolicy::OAuthOnly
        }
        "disable" => document.providers[0].credentials[0].enabled = Some(false),
        "enable" => document.providers[0].credentials[0].enabled = Some(true),
        "delete" => document.providers[0].credentials.clear(),
        "authorize" => {
            document.providers[0].credentials[0].api_key =
                Some("explicit-new-authorization".into());
            document.providers[0].credentials[0].refresh_token =
                Some("explicit-new-refresh".into());
        }
        _ => panic!("unknown fixture edit"),
    }
    authority
        .runtime
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
}
pub(super) async fn wait_called(mock: &Mock) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while mock.count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn server_policy_or_disable_during_exchange_retains_rotation_and_pending_writeback() {
    for mode in ["policy", "disable"] {
        let mut reply = rotated();
        reply["scope"] = json!("profile");
        let mock = Mock::new(200, reply, 100).await;
        let authority = authority_fixture::Authority::new();
        let store = authority.runtime.route_config();
        let old = owner(&store);
        let mut pending = Pending::new();
        let change = async {
            wait_called(&mock).await;
            edit(&authority, mode).await;
            refresh(&authority, &mock, &mut Pending::new()).await;
        };
        tokio::join!(refresh(&authority, &mock, &mut pending), change);
        assert!(!old.is_active());
        assert_eq!(mock.count(), 1);
        assert_eq!(
            pending.len(),
            1,
            "Server must retain already rotated material"
        );
        let saved = pending.values().next().unwrap().as_result().unwrap();
        assert_eq!(saved.previous_access, "synthetic-build-token");
        assert_eq!(saved.previous_refresh, "synthetic-refresh");
        assert_eq!(
            saved.token.refresh_token.as_deref(),
            Some("rotated-build-refresh")
        );
        assert_eq!(
            serde_json::to_value(&saved.token.grok_oauth).unwrap()["granted_scopes"],
            json!(["profile"])
        );
        assert_eq!(
            owner(&store).runtime_override().0.as_deref(),
            Some("rotated-build-access")
        );
        assert!(
            old.dispatch_readiness().is_err(),
            "old routing wrapper remains revoked even with fresh shared material"
        );
        for _ in 0..2 {
            refresh(&authority, &mock, &mut pending).await;
        }
        assert_eq!(
            mock.count(),
            1,
            "pending retry must not consume another grant"
        );
        mock.stop().await;
    }
}

#[tokio::test]
async fn server_policy_after_refresh_preserves_latest_material_scope_and_expiry() {
    for mode in ["policy", "disable"] {
        let mut reply = rotated();
        reply["scope"] = json!("profile");
        let mock = Mock::new(200, reply, 0).await;
        let authority = authority_fixture::Authority::new();
        let store = authority.runtime.route_config();
        let old = owner(&store);
        let mut pending = Pending::new();
        refresh(&authority, &mock, &mut pending).await;
        let updated = old.runtime_override();
        edit(&authority, mode).await;
        if mode == "disable" {
            edit(&authority, "enable").await;
        }
        let current = owner(&store);
        assert!(!Arc::ptr_eq(&old, &current));
        assert!(!old.is_active());
        assert_eq!(
            current.runtime_override(),
            updated,
            "configuration anchor must not roll back runtime rotation"
        );
        assert_eq!(
            current.refresh_snapshot(Duration::MAX).unwrap().1,
            "rotated-build-refresh"
        );
        assert_eq!(
            serde_json::to_value(current.authorization_context()).unwrap()["granted_scopes"],
            json!(["profile"])
        );
        refresh(&authority, &mock, &mut pending).await;
        assert_eq!(mock.count(), 1);
        assert_eq!(pending.len(), 1);
        mock.stop().await;
    }
}

#[tokio::test]
async fn server_late_failure_backoff_and_terminal_state_follow_authorization_not_policy() {
    for (status, reply) in [
        (503, json!({"error":"unavailable"})),
        (400, json!({"error":"invalid_grant"})),
    ] {
        let mock = Mock::new(status, reply, 100).await;
        let authority = authority_fixture::Authority::new();
        let mut pending = Pending::new();
        let change = async {
            wait_called(&mock).await;
            edit(&authority, "policy").await;
        };
        tokio::join!(refresh(&authority, &mock, &mut pending), change);
        refresh(&authority, &mock, &mut Pending::new()).await;
        assert_eq!(
            mock.count(),
            1,
            "policy must not reset the completed exchange failure"
        );
        edit(&authority, "authorize").await;
        refresh(&authority, &mock, &mut Pending::new()).await;
        assert_eq!(
            mock.count(),
            2,
            "new authorization has an independent retry lifecycle"
        );
        mock.stop().await;
    }
}

#[tokio::test]
async fn server_real_reauthorization_or_deletion_discards_late_exchange_and_pending() {
    for mode in ["authorize", "delete"] {
        let mock = Mock::new(200, rotated(), 100).await;
        let authority = authority_fixture::Authority::new();
        let store = authority.runtime.route_config();
        let old = owner(&store);
        let mut pending = Pending::new();
        let change = async {
            wait_called(&mock).await;
            edit(&authority, mode).await;
        };
        tokio::join!(refresh(&authority, &mock, &mut pending), change);
        assert!(!old.is_active());
        assert!(pending.is_empty());
        assert_eq!(old.runtime_override().0, None);
        assert_eq!(mock.count(), 1);
        mock.stop().await;
    }
}
