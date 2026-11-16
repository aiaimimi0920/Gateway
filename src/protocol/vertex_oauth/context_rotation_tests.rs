//! Only deadline tightening and newly observed fixed-scope evidence are legal rotations.
use super::context_tests::{context, material, value};
use super::OAuthError;
use serde_json::json;
use time::format_description::well_known::Rfc3339;

#[test]
fn vertex_oauth_context_rotation_preserves_or_tightens_deadline_and_scope_evidence() {
    let old = context(value());
    let next = old.rotated(&material(Some(7200), false)).unwrap();
    assert!(old.permits_rotation(&next));
    assert!(next.refresh_expires_at().unwrap() < old.refresh_expires_at().unwrap());
    assert!(next.scope_reported());
    let omitted = next.rotated(&material(None, false)).unwrap();
    assert_eq!(
        omitted.refresh_expires_at().unwrap(),
        next.refresh_expires_at().unwrap()
    );
    assert!(omitted.scope_reported());
    let longer = next.rotated(&material(Some(86400), true)).unwrap();
    assert_eq!(
        longer.refresh_expires_at().unwrap(),
        next.refresh_expires_at().unwrap()
    );
    assert!(next.permits_rotation(&longer));
}

#[test]
fn vertex_oauth_context_rotation_can_learn_deadline_and_reported_scope_once() {
    let mut unknown = value();
    unknown
        .as_object_mut()
        .unwrap()
        .remove("refresh_expires_at");
    unknown["scope_reported"] = json!(false);
    let old = context(unknown);
    let next = old.rotated(&material(Some(7200), true)).unwrap();
    assert!(next.refresh_expires_at().unwrap().is_some());
    assert!(next.scope_reported());
    assert!(old.permits_rotation(&next));
    assert!(!next.permits_rotation(&old));
}

#[test]
fn vertex_oauth_context_rotation_cannot_move_client_scope_or_target() {
    let old = context(value());
    for (pointer, replacement) in [
        ("/client_id", json!("other-client")),
        ("/client_secret", json!("other-secret")),
        ("/requested_scope", json!("openid")),
        ("/target/project", json!("other-project")),
        ("/target/location", json!("us-central1")),
        ("/target/quota_project", json!("other-billing")),
        ("/refresh_expires_at", json!("2099-01-02T00:00:00Z")),
        ("/scope_reported", json!(false)),
    ] {
        let mut changed = value();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        assert!(!old.permits_rotation(&context(changed)), "{pointer}");
    }
    for field in ["refresh_expires_at", "client_secret"] {
        let mut lost = value();
        lost.as_object_mut().unwrap().remove(field);
        assert!(!old.permits_rotation(&context(lost)), "{field}");
    }
    let mut equal_time = value();
    equal_time["refresh_expires_at"] = json!("2099-01-01T01:00:00.123456789+01:00");
    assert!(old.permits_rotation(&context(equal_time)));
    let precise = old
        .refresh_expires_at()
        .unwrap()
        .unwrap()
        .format(&Rfc3339)
        .unwrap();
    assert_eq!(precise, "2099-01-01T00:00:00.123456789Z");
}

#[test]
fn vertex_oauth_context_rotation_keeps_expired_evidence_and_rejects_invalid_original() {
    let mut expired = value();
    expired["refresh_expires_at"] = json!("2000-01-01T00:00:00.123456789Z");
    let old = context(expired);
    let next = old.rotated(&material(Some(7200), false)).unwrap();
    assert!(old.permits_rotation(&next));
    assert_eq!(
        next.refresh_expires_at().unwrap(),
        old.refresh_expires_at().unwrap()
    );
    assert_eq!(
        next.refresh_grant("synthetic-refresh").unwrap_err(),
        OAuthError::Expired
    );
    let mut invalid = value();
    invalid["requested_scope"] = json!("openid");
    let invalid = context(invalid);
    assert!(invalid.rotated(&material(None, false)).is_err());
    assert!(!invalid.permits_rotation(&next));
    assert!(!next.permits_rotation(&invalid));
}
