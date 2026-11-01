//! Local state remains isolated; explicitly selected credential object roots are opt-in.
use std::path::Path;

pub(crate) fn defaults(
    values: Vec<(&'static str, String)>,
    explicit_data_root: bool,
    driver: Option<&str>,
    object_root: Option<&Path>,
) -> Vec<(&'static str, String)> {
    let separate_objects =
        explicit_data_root && driver == Some("local") && object_root.is_some_and(Path::is_absolute);
    values
        .into_iter()
        .filter(|(key, _)| {
            !separate_objects
                || !matches!(
                    *key,
                    "AI_GATEWAY_OBJECT_STORAGE_DRIVER" | "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR"
                )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> Vec<(&'static str, String)> {
        [
            "GATEWAY_DATA_DIR",
            "GATEWAY_ROUTES_FILE",
            "GATEWAY_STATE_DIR",
            "AI_GATEWAY_OBJECT_STORAGE_DRIVER",
            "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
        ]
        .map(|key| (key, "isolated-default".to_string()))
        .to_vec()
    }

    #[test]
    fn explicit_local_object_root_preserves_only_object_overrides() {
        let result = defaults(
            values(),
            true,
            Some("local"),
            Some(Path::new("C:/credential-owner/objects")),
        );
        assert_eq!(result.len(), 3);
        assert!(result.iter().all(|(key, _)| key.starts_with("GATEWAY_")));
        assert_eq!(result[1].0, "GATEWAY_ROUTES_FILE");
    }

    #[test]
    fn defaults_stay_isolated_without_a_complete_explicit_selection() {
        for (explicit, driver, root) in [
            (
                false,
                Some("local"),
                Some(Path::new("C:/credential-owner/objects")),
            ),
            (true, None, Some(Path::new("C:/credential-owner/objects"))),
            (
                true,
                Some("s3-compatible"),
                Some(Path::new("C:/credential-owner/objects")),
            ),
            (true, Some("local"), Some(Path::new("relative/objects"))),
            (true, Some("local"), None),
        ] {
            assert_eq!(defaults(values(), explicit, driver, root), values());
        }
    }
}
