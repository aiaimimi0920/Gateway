use super::wire;
use serde_json::json;

#[test]
fn stitch_web_requires_its_own_live_bootstrap_and_encodes_nemo_rpc() {
    assert!(wire::bootstrap(r#"{"FdrFJe":"1","cfb2h":"public"}"#).is_err());
    let boot = wire::bootstrap(r#"{"SNlM0e":"test\\token","FdrFJe":"-123","cfb2h":"nemo-build"}"#)
        .unwrap();
    assert_eq!(boot.sid, "-123");
    assert_eq!(boot.build, "nemo-build");
    let encoded = wire::form(
        wire::GENERATE,
        &wire::generate_params("projects/123", "A blue task list"),
        &boot.at,
    );
    let fields: std::collections::HashMap<_, _> =
        url::form_urlencoded::parse(encoded.as_bytes()).collect();
    let frame: serde_json::Value = serde_json::from_str(&fields["f.req"]).unwrap();
    assert_eq!(frame[0][0][0], "IEkn6e");
    assert!(fields.get("at").is_some());
    assert!(!encoded.contains("recaptcha"));
}

#[test]
fn stitch_web_rpc_rejects_errors_wrong_ids_null_and_duplicates() {
    let row = json!([
        "wrb.fr",
        wire::LIST,
        "[\"synthetic-user\"]",
        null,
        null,
        null
    ]);
    assert_eq!(
        wire::decode(&json!([row.clone()]).to_string(), wire::LIST).unwrap(),
        json!(["synthetic-user"])
    );
    for text in [
        json!([["er", wire::LIST, 16]]).to_string(),
        json!([row.clone(), row]).to_string(),
        json!([["wrb.fr", "other", "[]", null, null, null]]).to_string(),
        json!([["wrb.fr", wire::LIST, "null", null, null, null]]).to_string(),
    ] {
        assert!(wire::decode(&text, wire::LIST).is_err());
    }
}

#[test]
fn stitch_web_resource_paths_cannot_inject_urls_or_parent_segments() {
    assert_eq!(
        wire::project(&json!(["projects/123"])).unwrap(),
        "projects/123"
    );
    assert_eq!(
        wire::session(&json!(["projects/123/sessions/456"])).unwrap(),
        "projects/123/sessions/456"
    );
    for resource in [
        "https://evil.invalid/",
        "projects/../sessions/id",
        "projects/12?token=x",
        "projects/a/b",
    ] {
        assert!(wire::project(&json!([resource])).is_err());
        assert!(wire::session(&json!([resource])).is_err());
    }
}

#[test]
fn stitch_web_only_refreshes_server_issued_xsrf_for_read_only_project_listing() {
    let response = json!([["er", wire::LIST, 16, ["xsrf", "synthetic-next"]]]).to_string();
    assert_eq!(
        wire::read_only_refresh(&response, wire::LIST, "old"),
        Some("synthetic-next".into())
    );
    assert!(wire::read_only_refresh(&response, wire::LIST, "synthetic-next").is_none());
    for rpc in [wire::CREATE, wire::GENERATE, wire::POLL, wire::SCREENS] {
        assert!(wire::read_only_refresh(&response, rpc, "old").is_none());
    }
    for response in [
        "not-json",
        "[[\"xsrf\",\"\"]]",
        "[[\"challenge\",\"not-an-xsrf-token\"]]",
    ] {
        assert!(wire::read_only_refresh(response, wire::LIST, "old").is_none());
    }
}
