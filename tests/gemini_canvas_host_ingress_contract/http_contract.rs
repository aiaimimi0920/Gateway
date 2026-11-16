//! Exercise public routes against the HTTP-only host using cfg(test) loopback admission.
mod documents;
mod http_fixture;
#[path = "../console_contract_support/mod.rs"]
mod support;
mod transcription_http;
mod transcription_transport;
