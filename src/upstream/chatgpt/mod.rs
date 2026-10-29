mod bootstrap;
pub mod catalog;
mod common;
mod execution;
mod headers;
pub mod official_api;
mod session;
pub mod web_reverse;

pub use execution::{execute, execute_stream};

const PROVIDER: &str = "chatgpt_web_reverse_compatible";
