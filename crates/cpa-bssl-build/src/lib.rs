//! Upstream build driver runs in the rama-only build process. Never set Cargo-global BSSL env.
include!(concat!(env!("OUT_DIR"), "/upstream_build.rs"));

pub fn build() {
    let package = std::path::Path::new(env!("CPA_BSSL_PACKAGE"));
    // Other TLS crates use the same variable names; scope all overrides to this subprocess.
    for (name, _) in std::env::vars_os() {
        let key = name.to_string_lossy();
        if [
            "BORING_BSSL_PATH",
            "BORING_BSSL_INCLUDE_PATH",
            "BORING_BSSL_SOURCE_PATH",
            "BORING_BSSL_ASSUME_PATCHED",
        ]
        .iter()
        .any(|prefix| key.contains(prefix))
        {
            std::env::remove_var(name);
        }
    }
    std::env::set_var("CARGO_MANIFEST_DIR", package);
    std::env::set_var("BORING_BSSL_SOURCE_PATH", package.join("deps/boringssl"));
    std::env::set_var("BORING_BSSL_ASSUME_PATCHED", "1");
    upstream_main();
}
