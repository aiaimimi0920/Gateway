//! Compile the immutable upstream build driver as a host library, not copied source debt.
use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let repo = manifest.parent().unwrap().parent().unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let script = repo.join("tools/prepare-cpa-bssl.py");
    for path in [
        script.clone(),
        repo.join("crates/cpa-tls-profile/compat"),
        repo.join("crates/cpa-tls-profile/upstream"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    // A content-addressed preparation directory preserves older build evidence on edits.
    let archive = repo.join("crates/cpa-tls-profile/upstream/rama-boring-sys-0.6.20.crate");
    let mut stamp = std::collections::hash_map::DefaultHasher::new();
    use std::hash::{Hash, Hasher};
    fs::read(&script).unwrap().hash(&mut stamp);
    let mut files: Vec<_> = fs::read_dir(repo.join("crates/cpa-tls-profile/compat"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.sort();
    for file in files {
        fs::read(file).unwrap().hash(&mut stamp);
    }
    out.hash(&mut stamp); // Distinct Cargo target directories must never prepare the same tree concurrently.
    fs::metadata(archive).unwrap().len().hash(&mut stamp);
    println!("cargo:rerun-if-env-changed=GATEWAY_CPA_BSSL_BUILD_ROOT");
    let local = repo.parent().unwrap().parent().unwrap().join("linshi");
    // NASM on Windows still rejects long paths; keep generated native source out of nested OUT_DIR.
    let base = env::var_os("GATEWAY_CPA_BSSL_BUILD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if local.is_dir() {
                local.join("cpa-bssl")
            } else {
                env::temp_dir().join("gateway-cpa-bssl")
            }
        });
    let prepared = base.join(format!("{:x}", stamp.finish()));
    println!("cargo:rerun-if-env-changed=PYTHON");
    let python = env::var_os("PYTHON").unwrap_or_else(|| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    });
    let result = Command::new(python)
        .arg(&script)
        .arg("--output")
        .arg(&prepared)
        .output()
        .expect("Python is required to prepare pinned CPA BoringSSL");
    assert!(
        result.status.success(),
        "CPA source preparation failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let package = prepared.join("rama-boring-sys-0.6.20");
    let source = fs::read_to_string(package.join("build/main.rs")).unwrap();
    assert_eq!(source.matches("fn main() {").count(), 1);
    let source = source
        .replace("fn main() {", "pub fn upstream_main() {")
        .replace(
            "mod config;",
            &format!(
                "#[path = {:?}] mod config;",
                package.join("build/config.rs")
            ),
        )
        .replace(
            "mod windows;",
            &format!(
                "#[path = {:?}] mod windows;",
                package.join("build/windows.rs")
            ),
        );
    fs::write(out.join("upstream_build.rs"), source).unwrap();
    println!("cargo:rustc-env=CPA_BSSL_PACKAGE={}", package.display());
}
