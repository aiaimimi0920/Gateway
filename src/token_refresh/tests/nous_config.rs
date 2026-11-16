//! Configuration is a trust boundary; typed contexts cannot inherit a parent's bearer or bypass ownership.
use super::{nous_fixture::*, *};
use serde_json::json;

#[tokio::test]
async fn nous_declaration_rejects_mixed_material_and_effective_transport_changes() {
    let f = fixture().await;
    let doc = f.runtime.route_config().snapshot().document().clone();
    let original = serde_json::to_value(&doc).unwrap();
    for (pointer, value) in [
        ("/providers/0/explicit_credentials_only", json!(false)),
        (
            "/providers/0/credentials/0/api_key",
            serde_json::Value::Null,
        ),
        (
            "/providers/0/credentials/0/refresh_token",
            serde_json::Value::Null,
        ),
        (
            "/providers/0/credentials/0/expires_at",
            json!("2100-01-01T00:00:00.5Z"),
        ),
        (
            "/providers/0/credentials/0/base_url",
            json!("https://example.com/v1"),
        ),
        (
            "/providers/0/credentials/0/refresh_endpoint",
            json!("https://example.com/token"),
        ),
        (
            "/providers/0/credentials/0/refresh_client_id",
            json!("foreign-client"),
        ),
        (
            "/providers/0/credentials/0/session_auth",
            json!({"transport":"cookie","primary_cookie_name":"session"}),
        ),
        ("/providers/0/credentials/0/auth_mode", json!("bearer")),
        (
            "/providers/0/credentials/0/headers",
            json!({"authorization":"foreign"}),
        ),
    ] {
        let mut value_doc = original.clone();
        // Optional fields omitted by serialization are added at the existing credential boundary.
        let parts: Vec<_> = pointer.rsplitn(2, '/').collect();
        value_doc
            .pointer_mut(parts[1])
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(parts[0].into(), value);
        let parsed = serde_json::from_value(value_doc);
        assert!(
            parsed.map_or(true, |doc| RouteConfigStore::from_document(doc).is_err()),
            "{pointer}"
        );
    }
}
