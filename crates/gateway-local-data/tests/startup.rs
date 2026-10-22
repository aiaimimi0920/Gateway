//! Startup policy is additive and never rewrites existing configuration or credentials.
use gateway_local_data::startup::StartupConfig;
use std::{fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "gateway-startup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root.join("gateway.json"))
    }
    fn write(&self, value: &str) {
        fs::write(&self.0, value).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

#[test]
fn missing_file_and_missing_field_preserve_existing_ui_default() {
    let fixture = Fixture::new();
    assert!(
        StartupConfig::from_path(&fixture.0)
            .unwrap()
            .enable_management_ui
    );
    fixture.write(r#"{"future_optional_setting":42}"#);
    assert!(
        StartupConfig::from_path(&fixture.0)
            .unwrap()
            .enable_management_ui
    );
}

#[test]
fn configuration_and_compiled_capability_form_an_and_gate() {
    let fixture = Fixture::new();
    for requested in [false, true] {
        let original = format!(r#"{{"enable_management_ui":{requested}}}"#);
        fixture.write(&original);
        let config = StartupConfig::from_path(&fixture.0).unwrap();
        for compiled in [false, true] {
            assert_eq!(
                config.management_ui_enabled(compiled),
                requested && compiled
            );
        }
        assert_eq!(fs::read_to_string(&fixture.0).unwrap(), original);
    }
}

#[test]
fn invalid_or_unbounded_configuration_does_not_silently_enable_ui() {
    let fixture = Fixture::new();
    for invalid in [r#"{"enable_management_ui":"false"}"#, "{", "null"] {
        fixture.write(invalid);
        assert!(StartupConfig::from_path(&fixture.0).is_err());
    }
    fixture.write(&" ".repeat(65_537));
    assert!(StartupConfig::from_path(&fixture.0).is_err());
}
