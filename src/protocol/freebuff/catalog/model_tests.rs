use super::{tests::body, *};
use serde_json::{json, Value};

fn parse(value: &Value) -> Result<ModelCatalog, CatalogError> {
    ModelCatalog::parse(&serde_json::to_vec(value).unwrap())
}
#[test]
fn unknown_metadata_is_compatible_but_catalog_identity_is_unambiguous() {
    let mut value = body();
    value["serverAddedField"] = json!({"v":2});
    value["rows"][0]["serverAddedField"] = json!(true);
    assert!(parse(&value).is_ok());
    for field in ["key", "handle"] {
        let mut duplicate = body();
        duplicate["rows"][1][field] = duplicate["rows"][0][field].clone();
        assert_eq!(parse(&duplicate).err(), Some(CatalogError::InvalidResponse));
    }
}
#[test]
fn required_core_fields_and_types_are_not_synthesized() {
    for field in [
        "protocol",
        "version",
        "issuedAt",
        "refreshAt",
        "rows",
        "plansUrl",
    ] {
        let mut value = body();
        value.as_object_mut().unwrap().remove(field);
        assert_eq!(
            parse(&value).err(),
            Some(CatalogError::InvalidResponse),
            "{field}"
        );
    }
    for field in [
        "key",
        "handle",
        "displayName",
        "tagline",
        "badges",
        "multimodal",
        "premium",
        "dataUse",
        "access",
        "sortOrder",
    ] {
        let mut value = body();
        value["rows"][0].as_object_mut().unwrap().remove(field);
        assert_eq!(
            parse(&value).err(),
            Some(CatalogError::InvalidResponse),
            "{field}"
        );
    }
    for (field, invalid) in [
        ("issuedAt", json!(-1)),
        ("refreshAt", json!(0.5)),
        ("protocol", json!(2)),
        ("fetchId", json!({})),
    ] {
        let mut value = body();
        value[field] = invalid;
        assert_eq!(parse(&value).err(), Some(CatalogError::InvalidResponse));
    }
}
#[test]
fn bounded_and_header_safe_catalog_does_not_accept_legacy_ids_or_control_text() {
    for (field, invalid) in [
        ("handle", json!("legacy-model")),
        ("handle", json!("fbm1.")),
        ("handle", json!("fbm1.value\r\nInjected: bad")),
        ("displayName", json!("escape\u{1b}[2J")),
        ("access", json!("unknown")),
        ("dataUse", json!("unknown")),
        ("badges", json!([{"kind":"new","label":"bad\nlabel"}])),
        ("opensAt", json!(9_007_199_254_740_992u64)),
    ] {
        let mut value = body();
        value["rows"][0][field] = invalid;
        assert_eq!(
            parse(&value).err(),
            Some(CatalogError::InvalidResponse),
            "{field}"
        );
    }
    let mut value = body();
    value["fetchId"] = json!("bad\nfetch");
    assert!(parse(&value).is_err());
    value = body();
    value["rows"] = json!(vec![value["rows"][0].clone(); 513]);
    assert!(parse(&value).is_err());
    assert_eq!(
        ModelCatalog::parse(&vec![b'x'; 1_048_577]).err(),
        Some(CatalogError::ResponseTooLarge)
    );
}
#[test]
fn server_time_controls_listing_and_locked_rows_remain_visible_not_selectable() {
    let mut value = body();
    value["rows"][0]["opensAt"] = json!(1000);
    let catalog = parse(&value).unwrap();
    let rows: Vec<_> = catalog.rows().map(ModelRow::key).collect();
    assert_eq!(rows, ["open", "locked"]);
    assert_eq!(catalog.rows().nth(1).unwrap().access, ModelAccess::Locked);
    assert!(catalog.select("open").is_ok());
    assert!(catalog.select("locked").is_err());
    value["rows"] = json!([value["rows"][2].clone()]);
    assert_eq!(parse(&value).err(), Some(CatalogError::Unsupported));
}
#[test]
fn each_selected_handle_and_fetch_belong_to_the_same_snapshot() {
    let old = parse(&body()).unwrap();
    let mut next = body();
    next["fetchId"] = json!("new-fetch");
    next["rows"][0]["handle"] = json!("fbm1.new");
    let new = parse(&next).unwrap();
    let old_pick = old.select("open").unwrap();
    let new_pick = new.select("open").unwrap();
    assert_eq!(
        (old_pick.handle, old_pick.fetch_id),
        ("fbm1.synthetic-open", Some("synthetic-fetch"))
    );
    assert_eq!(
        (new_pick.handle, new_pick.fetch_id),
        ("fbm1.new", Some("new-fetch"))
    );
    let debug = format!("{new:?}");
    assert!(!debug.contains("new-fetch") && !debug.contains("fbm1.new"));
}
