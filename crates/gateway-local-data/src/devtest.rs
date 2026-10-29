//! Compile-time, opt-in hand-test isolation. Normal binaries retain their existing defaults.
use std::io;
use std::path::{Path, PathBuf};

pub fn isolated_build() -> bool {
    option_env!("GATEWAY_DEVTEST_ISOLATED") == Some("1")
}

pub fn default_api_port() -> u16 {
    // Hand-test data stays isolated; external clients retain the normal API address.
    4200
}

pub(crate) fn resolve_root(
    executable: &Path,
    home: Option<&Path>,
    explicit: Option<&Path>,
    isolated: bool,
) -> io::Result<PathBuf> {
    if !isolated || explicit.is_some() {
        return super::resolve(executable, home, explicit);
    }
    // Ignore portable .ng in a hand-test build: it may belong to the live instance.
    home.filter(|path| path.is_absolute())
        .map(|path| path.join(".ng-devtest-4300"))
        .ok_or_else(|| io::Error::other("Cannot resolve the current user's home directory"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devtest_isolates_defaults_but_preserves_explicit_data_overrides() {
        let home = std::env::temp_dir();
        let executable = home.join("fixture-package/gateway.exe");
        assert_eq!(
            resolve_root(&executable, Some(&home), None, true).unwrap(),
            home.join(".ng-devtest-4300")
        );
        let explicit = home.join("isolated-test-data");
        assert_eq!(
            resolve_root(&executable, Some(&home), Some(&explicit), true).unwrap(),
            explicit
        );
        assert!(resolve_root(&executable, Some(&home), Some(Path::new("relative")), true).is_err());
        assert_eq!(default_api_port(), 4200);
    }
}
