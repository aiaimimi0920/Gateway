//! Target syntax and origin tests do not assert model availability, IAM or quota sharing.
use super::{OAuthError, VertexTarget};
use serde_json::json;

#[test]
fn vertex_oauth_target_global_and_regional_are_distinct_from_express() {
    for (location, host) in [
        ("global", "aiplatform.googleapis.com"),
        ("us-central1", "us-central1-aiplatform.googleapis.com"),
        ("future-west42", "future-west42-aiplatform.googleapis.com"),
    ] {
        let target =
            VertexTarget::new("explicit-project", location, Some("billing-project")).unwrap();
        assert_eq!(target.base_url().unwrap(), format!("https://{host}/v1/projects/explicit-project/locations/{location}/publishers/google"));
        assert_eq!(target.project(), "explicit-project");
        assert_eq!(target.location(), location);
        assert_eq!(target.quota_project(), Some("billing-project"));
        assert!(!super::super::vertex_express::owns_base(
            &target.base_url().unwrap()
        ));
    }
}

#[test]
fn vertex_oauth_target_accepts_numeric_and_legacy_domain_project_segments() {
    for project in [
        "123456789012",
        "example.com:legacy-project",
        "my-project",
        "RESOURCE_ID_1",
    ] {
        let target = VertexTarget::new(project, "global", Some(project)).unwrap();
        let base = target.base_url().unwrap();
        assert_eq!(url::Url::parse(&base).unwrap().as_str(), base);
        assert_eq!(target.project(), project);
    }
}

#[test]
fn vertex_oauth_target_rejects_resource_injection_and_header_material() {
    for bad in [
        "",
        ".",
        "..",
        "project/other",
        "project%2fother",
        "project?key=secret",
        "project#fragment",
        "project\\other",
        "user@host",
        " project",
        "project\r\nx: secret",
        "项目",
    ] {
        assert_eq!(
            VertexTarget::new(bad, "global", None).unwrap_err(),
            OAuthError::InvalidInput,
            "project={bad:?}"
        );
        assert!(
            VertexTarget::new("explicit-project", "global", Some(bad)).is_err(),
            "quota={bad:?}"
        );
    }
    assert!(VertexTarget::new(&"a".repeat(257), "global", None).is_err());
    assert!(VertexTarget::new("explicit-project", "global", Some(&"a".repeat(257))).is_err());
}

#[test]
fn vertex_oauth_target_location_cannot_change_google_origin() {
    for bad in [
        "",
        "Global",
        "us.central1",
        "-region",
        "region-",
        "region:443",
        "x/../global",
        "x%2fglobal",
        "x?key=secret",
        "x#fragment",
        "x@evil.test",
        "evil.test/",
        "x\r\nx: secret",
    ] {
        assert!(
            VertexTarget::new("explicit-project", bad, None).is_err(),
            "location={bad:?}"
        );
    }
    assert!(VertexTarget::new("explicit-project", &"a".repeat(64), None).is_err());
    assert!(VertexTarget::new("explicit-project", &"a".repeat(53), None).is_err());
    assert!(VertexTarget::new("explicit-project", &"a".repeat(52), None).is_ok());
}

#[test]
fn vertex_oauth_target_deserialized_values_are_checked_before_url_derivation() {
    let target: VertexTarget =
        serde_json::from_value(json!({"project":"safe/../other", "location":"global"})).unwrap();
    assert!(target.validate().is_err());
    assert!(target.base_url().is_err());
    for value in [
        json!({"project":"p","location":"global","publisher":"other"}),
        json!({"project":"p","location":"global","base_url":"https://evil.test"}),
        json!({"project":"p","location":null}),
        json!({"project":"p"}),
    ] {
        assert!(serde_json::from_value::<VertexTarget>(value).is_err());
    }
}

#[test]
fn vertex_oauth_target_multiregion_uses_explicit_rep_host_not_regional_guess() {
    for location in ["us", "eu"] {
        let target = VertexTarget::new("explicit-project", location, None).unwrap();
        let base = target.base_url().unwrap();
        assert_eq!(base, format!("https://aiplatform.{location}.rep.googleapis.com/v1/projects/explicit-project/locations/{location}/publishers/google"));
        let url = url::Url::parse(&base).unwrap();
        assert_eq!(url.scheme(), "https");
        assert_eq!(
            url.host_str(),
            Some(format!("aiplatform.{location}.rep.googleapis.com").as_str())
        );
        assert!(url.query().is_none() && url.fragment().is_none());
        assert!(!base.contains(&format!("{location}-aiplatform")));
        assert!(!super::super::vertex_express::owns_base(&base));
    }
}
