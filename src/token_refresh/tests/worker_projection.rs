//! Recovery does not trust a marker or digest without checking the complete finite effect.
use super::*;
use crate::token_refresh::is_refresh_projection;
use serde_json::json;

#[test]
fn worker_projection_rejects_non_token_changes_scope_expansion_and_material_deletion() {
    let before: RouteConfigYaml = serde_json::from_value(grok_context::document()).unwrap();
    let mut next = serde_json::to_value(&before).unwrap();
    let material = &mut next["providers"][0]["credentials"][0];
    material["api_key"] = json!("opaque-rotation");
    material["refresh_token"] = json!("new-refresh");
    material["expires_at"] = json!("2030-01-01T00:00:00Z");
    material["token_expires_in_secs"] = json!(3600);
    material["grok_oauth"]["granted_scopes"] = json!(["profile"]);
    let after: RouteConfigYaml = serde_json::from_value(next.clone()).unwrap();
    assert!(is_refresh_projection(
        &before,
        &after,
        "build-pool",
        "exact-build"
    ));
    for pointer in [
        "/providers/0/base_url",
        "/providers/0/preset",
        "/providers/0/credentials/0/enabled",
        "/providers/0/credentials/0/headers",
        "/providers/0/credentials/0/refresh_endpoint",
        "/providers/0/credentials/0/refresh_client_id",
        "/providers/0/credentials/0/refresh_token",
        "/providers/0/credentials/0/grok_oauth/client_id",
        "/providers/0/credentials/0/grok_oauth/granted_scopes",
    ] {
        let mut changed = next.clone();
        let value = match pointer {
            value if value.ends_with("enabled") => json!(false),
            value if value.ends_with("headers") => json!({"X-Injected":"forbidden"}),
            value if value.ends_with("refresh_token") => serde_json::Value::Null,
            value if value.ends_with("granted_scopes") => json!(["openid", "profile", "forbidden"]),
            _ => json!("forbidden-other-target"),
        };
        let segments: Vec<_> = pointer.trim_start_matches('/').split('/').collect();
        // Fields absent from the synthetic document may be inserted only at the credential/provider object.
        let (leaf, parent) = segments.split_last().unwrap();
        let parent_pointer = format!("/{}", parent.join("/"));
        changed.pointer_mut(&parent_pointer).unwrap()[*leaf] = value;
        if let Ok(changed) = serde_json::from_value::<RouteConfigYaml>(changed) {
            assert!(
                !is_refresh_projection(&before, &changed, "build-pool", "exact-build"),
                "accepted {pointer}"
            );
        }
    }
}
