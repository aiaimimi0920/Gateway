mod bootstrap;
mod common;
mod execution;
mod headers;
pub mod official_api;
pub mod web_reverse;

pub use execution::{execute, execute_stream};

const PROVIDER: &str = "chatgpt_web_reverse_compatible";
