use super::*;

fn source(domain: &str) -> String {
    json!({"schemaVersion":1,"provider":"stitch-web","authUser":"0","cookies":[{
        "name":"OSID","value":"synthetic","domain":domain,"path":"/","expires":-1,"secure":true
    }]})
    .to_string()
}

#[test]
fn stitch_web_json_is_product_scoped_and_declares_only_its_design_operation_alias() {
    let family = GeminiAuthFamily::StitchWeb;
    let material = parse(&source("stitch.withgoogle.com"), family).unwrap();
    let input = CreateGeminiAuthSessionInput {
        target_family: family,
        provider_id: "renamed-stitch-pool".into(),
        account_label: Some("Stitch account".into()),
    };
    let (draft, object) = draft(material, &input, "fixture").unwrap();
    assert_eq!(
        draft.credential["supported_models"],
        json!(["stitch-design"])
    );
    assert_eq!(draft.credential["model_selection"]["fetched"], json!([]));
    assert_eq!(draft.provider_id, "renamed-stitch-pool");
    assert!(draft.secret_edits.is_empty());
    let (key, bytes) = object.unwrap();
    assert!(key.starts_with("credential-runtime/stitch-web/"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["provider"],
        "stitch-web"
    );
}

#[test]
fn stitch_web_rejects_cross_product_cookie_broadening_and_secondary_account_claims() {
    for domain in [
        ".google.com",
        "accounts.google.com",
        "flow.google.com",
        "evil.withgoogle.com",
    ] {
        assert!(validate_stitch_bundle(&source(domain)).is_err());
    }
    let mut material: Value = serde_json::from_str(&source("stitch.withgoogle.com")).unwrap();
    material["authUser"] = json!("1");
    assert!(validate_stitch_bundle(&material.to_string()).is_err());
    material["authUser"] = json!("0");
    material["cookies"][0]["name"] = json!("__Secure-invalid");
    material["cookies"][0]["secure"] = json!(false);
    assert!(validate_stitch_bundle(&material.to_string()).is_err());
}
