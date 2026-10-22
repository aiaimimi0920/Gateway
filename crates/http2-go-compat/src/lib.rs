//! Fixed-source HTTP/2 dependency. Default behavior is unchanged; CPA opts in per request.
#![allow(clippy::type_complexity, clippy::manual_range_contains)]

/// Internal request extension enabling Go-compatible HPACK for this header block.
/// This is not an HTTP header and cannot be selected by untrusted inbound fields.
#[derive(Clone, Copy, Debug)]
pub struct GoCompatibility;

include!(concat!(env!("OUT_DIR"), "/http2-0.5.20/src/lib_body.rs"));

#[cfg(test)]
#[path = "../compat/transport_tests.rs"]
mod transport_tests;
