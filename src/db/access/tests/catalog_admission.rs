use super::super::projection::collect_admitted_model_catalog_ids;
use super::*;

#[test]
fn catalog_admission_access_denied_alias_cannot_shadow_native_or_mutate_projection() {
    let projection = group_projection_rows_by_model(
        &make_access_key_auth(),
        vec![
            make_projection_row("alias", Some("actual")),
            make_projection_row("actual", None),
        ],
    );
    let before = serde_json::to_value(&projection).unwrap();
    assert_eq!(
        collect_admitted_model_catalog_ids(&projection, &|row| row.model_code != "alias"),
        vec!["actual"]
    );
    assert_eq!(serde_json::to_value(&projection).unwrap(), before);
    assert_eq!(
        collect_model_catalog_ids_from_access_projection(&projection),
        vec!["alias"]
    );
}

#[test]
fn catalog_admission_access_provider_identity_not_global_model_name_is_authority() {
    let projection = group_projection_rows_by_model(
        &make_access_key_auth(),
        vec![
            make_projection_row("alias", Some("actual")),
            ProjectedPlatformAccessRow {
                provider_account_id: "other".into(),
                ..make_projection_row("actual", None)
            },
        ],
    );
    assert_eq!(
        collect_admitted_model_catalog_ids(&projection, &|row| row.provider_account_id == "other"),
        vec!["actual"]
    );
    assert!(collect_admitted_model_catalog_ids(&projection, &|_| false).is_empty());
}

#[test]
fn catalog_admission_access_same_visible_model_survives_any_authorized_healthy_row() {
    let projection = group_projection_rows_by_model(
        &make_access_key_auth(),
        vec![
            make_projection_row("alias", Some("actual")),
            ProjectedPlatformAccessRow {
                provider_account_id: "other".into(),
                ..make_projection_row("alias", Some("other-actual"))
            },
        ],
    );
    assert_eq!(
        collect_admitted_model_catalog_ids(&projection, &|row| row.provider_account_id == "other"
            && row.upstream_model.as_deref() == Some("other-actual")),
        vec!["alias"]
    );
}
