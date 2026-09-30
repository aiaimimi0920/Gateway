use gateway_local_data::{local_environment, prepare, resolve};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "gateway-ng-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("install")).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn default_is_stable_across_updates_and_does_not_create_portable_directory() {
    let f = Fixture::new();
    let home = f.0.join("user");
    for version in ["old", "new"] {
        let executable = f.0.join(version).join("gateway.exe");
        let root = resolve(&executable, Some(&home), None).unwrap();
        assert_eq!(root, home.join(".ng"));
        prepare(&root).unwrap();
        assert!(!executable.parent().unwrap().join(".ng").exists());
    }
}

#[test]
fn portable_exe_directory_overrides_home_for_both_binaries() {
    let f = Fixture::new();
    let portable = f.0.join("install/.ng");
    fs::create_dir(&portable).unwrap();
    for binary in ["gateway.exe", "gateway-ui.exe"] {
        assert_eq!(
            resolve(
                &f.0.join("install").join(binary),
                Some(&f.0.join("home")),
                None
            )
            .unwrap(),
            portable
        );
    }
    let environment: std::collections::HashMap<_, _> =
        local_environment(&portable).into_iter().collect();
    assert_eq!(environment["GATEWAY_STORAGE_MODE"], "local");
    assert_eq!(
        environment["GATEWAY_STATE_DIR"],
        portable.join("local/state").to_string_lossy()
    );
}

#[test]
fn explicit_absolute_override_is_preserved_and_unsafe_fallback_is_rejected() {
    let f = Fixture::new();
    let executable = f.0.join("install/gateway.exe");
    let override_path = f.0.join("isolated");
    assert_eq!(
        resolve(&executable, None, Some(&override_path)).unwrap(),
        override_path
    );
    assert!(resolve(&executable, None, None).is_err());
    assert!(resolve(&executable, None, Some(std::path::Path::new("relative"))).is_err());
    fs::write(f.0.join("install/.ng"), "not a directory").unwrap();
    assert!(resolve(&executable, Some(&f.0), None).is_err());
}
