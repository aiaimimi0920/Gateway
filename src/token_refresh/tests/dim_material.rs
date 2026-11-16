//! Additive typed authority cannot change legacy hashes or widen a consumed authorization.
use super::{dim_fixture::*, *};
use serde_json::json;

#[tokio::test]
async fn dim_material_binds_subject_nonce_site_client_endpoints_and_scope() {
    let f = fixture().await;
    let material = request(&f).1.material;
    let mut next = material.clone();
    let mut context = context();
    context["granted_scopes"] = json!(["openid"]);
    next.dim_oauth = Some(serde_json::from_value(context).unwrap());
    assert!(material.permits(&next));
    assert!(!next.permits(&material));
    assert_eq!(material.family_id(), next.family_id());
    assert_ne!(material.material_id(), next.material_id());
    for (field, value) in [
        ("site", json!("global")),
        ("client_id", json!("other-client")),
        ("subject", json!("other-subject")),
        ("nonce", json!("other-nonce")),
        ("token_endpoint", json!("https://dimagent.cn/other")),
        ("granted_scopes", json!(["openid", "extra"])),
    ] {
        let mut value_raw = serde_json::to_value(&material).unwrap();
        value_raw["dimOauth"][field] = value;
        assert!(
            !material.permits(&serde_json::from_value(value_raw).unwrap()),
            "{field}"
        );
    }
}
#[tokio::test]
async fn dim_absent_context_preserves_legacy_material_id_and_wire_format() {
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
    assert!(serde_json::to_value(material)
        .unwrap()
        .get("dimOauth")
        .is_none());
}
#[tokio::test]
async fn dim_declaration_rejects_wrong_site_and_missing_or_foreign_context() {
    let f = fixture().await;
    let s = f.runtime.route_config().snapshot();
    for (pointer, value) in [
        ("/providers/0/base_url", json!("https://dimagent.com/v1")),
        (
            "/providers/0/credentials/0/dim_oauth",
            serde_json::Value::Null,
        ),
        (
            "/providers/0/credentials/0/refresh_client_id",
            json!("foreign-client"),
        ),
        ("/providers/0/auth_mode", json!("vertex_user_oauth")),
    ] {
        let mut raw = serde_json::to_value(s.document()).unwrap();
        *raw.pointer_mut(pointer).unwrap() = value;
        assert!(
            f.runtime
                .commit_document(
                    s.revision().id(),
                    serde_json::from_value(raw).unwrap(),
                    None
                )
                .await
                .is_err(),
            "{pointer}"
        );
    }
}
