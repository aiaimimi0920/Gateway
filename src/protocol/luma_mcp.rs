//! Bounded read-only MCP catalog and model discovery; no persistence or generic MCP execution.
pub mod boundary;
mod catalog;
mod cleanup;
mod decode;
mod inspection;
mod models;
mod transport;

pub use catalog::ToolDefinition;
pub use inspection::inspect_tools;
pub use models::{read_models, ModelQuery};

const VERSION: &str = "2025-03-26";

/// Classified failures never include upstream bodies, credentials or session headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum McpError {
    #[error("luma_mcp_invalid_response")]
    InvalidResponse,
    #[error("luma_mcp_rejected")]
    Rejected,
    #[error("luma_mcp_limit")]
    Limit,
    #[error("luma_mcp_timeout")]
    Timeout,
    #[error("luma_mcp_expired")]
    Expired,
    #[error("luma_mcp_transport")]
    Transport,
    #[error("luma_mcp_redirect")]
    Redirect,
    #[error("luma_mcp_unsupported")]
    Unsupported,
}

#[cfg(test)]
mod cleanup_tests;
#[cfg(test)]
mod decode_tests;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod models_tests;
#[cfg(test)]
mod tests;
