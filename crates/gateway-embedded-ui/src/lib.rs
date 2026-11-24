//! Immutable embedded Web UI bundle decoupled from the core Gateway Rust crate.

use std::borrow::Cow;

include!(concat!(env!("OUT_DIR"), "/gateway_embedded_ui.rs"));

/// Look up an embedded web console asset by normalized relative path.
pub fn get_asset(path: &str) -> Option<Cow<'static, [u8]>> {
    EmbeddedUi::get(path).map(|asset| asset.data)
}

/// Return true when an exact embedded asset path exists in the bundle.
pub fn asset_exists(path: &str) -> bool {
    EmbeddedUi::iter().any(|candidate| candidate == path)
}
