//! Default non-interactive identity for the capture-verified Codex version.
//! A real inbound Codex identity always takes precedence over this fallback.
use std::sync::OnceLock;

pub(super) const ORIGINATOR: &str = "codex_exec";
const VERSION: &str = "0.155.1";

pub(super) fn user_agent() -> &'static str {
    static UA: OnceLock<String> = OnceLock::new();
    UA.get_or_init(|| {
        format!(
            "{ORIGINATOR}/{VERSION} ({}; {}) unknown ({ORIGINATOR}; {VERSION})",
            platform(),
            std::env::consts::ARCH
        )
    })
}

#[cfg(windows)]
fn platform() -> String {
    // Read once; never run a shell or modify the OS to discover its version.
    let key = windows_registry::LOCAL_MACHINE
        .open(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
        .ok();
    match key.and_then(|key| {
        Some((
            key.get_u32("CurrentMajorVersionNumber").ok()?,
            key.get_u32("CurrentMinorVersionNumber").ok()?,
            key.get_string("CurrentBuildNumber").ok()?,
        ))
    }) {
        Some((major, minor, build)) if build.bytes().all(|v| v.is_ascii_digit()) => {
            format!("Windows {major}.{minor}.{build}")
        }
        _ => "Windows".to_owned(),
    }
}

#[cfg(not(windows))]
fn platform() -> String {
    // Non-Windows fallback is descriptive, not a claim of cross-platform wire verification.
    std::env::consts::OS.to_owned()
}
