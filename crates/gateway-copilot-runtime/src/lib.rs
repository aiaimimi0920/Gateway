//! 显式本地 Copilot runtime 的所有权层，不提供公共推理路由。
mod config;
mod owner;
mod text;
pub use config::RuntimeConfig;
pub use owner::{CopilotRuntime, RuntimeError};
pub use text::{TextCompletion, TextError};
