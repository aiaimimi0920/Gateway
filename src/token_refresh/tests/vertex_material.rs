//! Material IDs are additive; mutable Vertex fields cannot widen authority or bypass family fences.
use super::{vertex_fixture::*, *};
use serde_json::json;

#[tokio::test]
async fn vertex_oauth_grant_material_rotation_rejects_target_client_scope_and_deadline_widening() {
    let f = fixture().await;
    let material = request(&f).1.material;
    let mut next = material.clone();
    let mut context = context();
    context["scope_reported"] = json!(true);
    context["refresh_expires_at"] = json!("2080-01-01T00:00:00Z");
    next.vertex_oauth = Some(serde_json::from_value(context).unwrap());
    assert!(material.permits(&next));
    assert!(!next.permits(&material));
    assert_eq!(material.family_id(), next.family_id());
    assert_ne!(material.material_id(), next.material_id());
    for (pointer, value) in [
        ("/vertexOauth/target/project", json!("other-project")),
        ("/vertexOauth/client_id", json!("other-client")),
        ("/vertexOauth/client_secret", json!("other-private-client")),
        ("/vertexOauth/requested_scope", json!("different-scope")),
        (
            "/vertexOauth/refresh_expires_at",
            json!("2099-01-02T00:00:00Z"),
        ),
    ] {
        let mut raw = serde_json::to_value(&material).unwrap();
        *raw.pointer_mut(pointer).unwrap() = value;
        let changed = serde_json::from_value(raw).unwrap();
        assert!(!material.permits(&changed), "{pointer}");
    }
}

#[tokio::test]
async fn vertex_oauth_none_extension_preserves_legacy_material_hash_and_json() {
    use sha2::{Digest, Sha256};
    let f = LocalFixture::new("https://example.com/token").await;
    let material = request(&f).1.material;
    let legacy = json!([
        "gateway-refresh-material-v1",
        material.endpoint,
        material.client,
        material.refresh,
        material.access,
        material.context,
        material.expires_at
    ]);
    assert_eq!(
        material.material_id(),
        hex::encode(Sha256::digest(legacy.to_string().as_bytes()))
    );
    assert!(serde_json::to_value(&material)
        .unwrap()
        .get("vertexOauth")
        .is_none());
}

#[tokio::test]
async fn vertex_oauth_expired_refresh_deadline_fails_before_sending_admission() {
    let f = fixture().await;
    let snapshot = f.runtime.route_config().snapshot();
    let mut raw = serde_json::to_value(snapshot.document()).unwrap();
    raw["providers"][0]["credentials"][0]["vertex_oauth"]["refresh_expires_at"] =
        json!("2021-01-01T00:00:00Z");
    f.runtime
        .commit_document(
            snapshot.revision().id(),
            serde_json::from_value(raw).unwrap(),
            None,
        )
        .await
        .unwrap();
    let original = request(&f).1;
    let mock = MockOAuth::new(200, &response(), 0).await;
    refresh(&f, &mock, &mut Pending::new()).await;
    assert_eq!(mock.count(), 0);
    assert!(!persistence(&f)
        .state_root()
        .join("refresh-grants")
        .join(original.family_id())
        .join(format!("{}.json", original.material_id()))
        .exists());
}
