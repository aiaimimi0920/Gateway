//! Redis key contracts keep console namespaces isolated from the legacy mirror.

use neuro_gateway::console::RouteConfigRedisKeys;
use neuro_gateway::redis::keys::{
    console_route_config_active_document_key, console_route_config_active_revision_key,
    console_route_config_events_key, console_route_config_revision_key,
    console_route_config_transaction_key, LEGACY_ROUTE_CONFIG_DOCUMENT_KEY,
};
#[test]
fn default_namespace_uses_the_console_route_config_key_family() {
    let keys = RouteConfigRedisKeys::new("default").unwrap();

    assert_eq!(keys.namespace(), "default");
    assert_eq!(
        keys.active_revision_key(),
        "gw:console:route-config:default:active_revision"
    );
    assert_eq!(
        keys.active_document_key(),
        "gw:console:route-config:default:active_document"
    );
    assert_eq!(
        keys.revision_key("r19-123456abcdef"),
        "gw:console:route-config:default:revisions:r19-123456abcdef"
    );
    assert_eq!(
        keys.transaction_key("tx-123"),
        "gw:console:route-config:default:transactions:tx-123"
    );
    assert_eq!(keys.events_key(), "gw:console:route-config:default:events");
    assert_eq!(
        keys.legacy_document_key(),
        Some(LEGACY_ROUTE_CONFIG_DOCUMENT_KEY)
    );
}

#[test]
fn non_default_namespace_does_not_update_the_global_legacy_mirror() {
    let keys = RouteConfigRedisKeys::new("dev_a").unwrap();

    assert!(!keys.updates_legacy_mirror());
    assert_eq!(keys.legacy_document_key(), None);
    assert_eq!(
        keys.active_revision_key(),
        "gw:console:route-config:dev_a:active_revision"
    );
}

#[test]
fn redis_route_config_namespace_rejects_colons_and_other_invalid_characters() {
    let error = RouteConfigRedisKeys::new("dev:blue").unwrap_err();

    assert_eq!(error.code(), "console_invalid_redis_namespace");
}

#[test]
fn low_level_console_route_key_builders_follow_the_specified_layout() {
    assert_eq!(
        console_route_config_active_revision_key("worker"),
        "gw:console:route-config:worker:active_revision"
    );
    assert_eq!(
        console_route_config_active_document_key("worker"),
        "gw:console:route-config:worker:active_document"
    );
    assert_eq!(
        console_route_config_revision_key("worker", "r7-abcdef123456"),
        "gw:console:route-config:worker:revisions:r7-abcdef123456"
    );
    assert_eq!(
        console_route_config_transaction_key("worker", "tx_9"),
        "gw:console:route-config:worker:transactions:tx_9"
    );
    assert_eq!(
        console_route_config_events_key("worker"),
        "gw:console:route-config:worker:events"
    );
}
