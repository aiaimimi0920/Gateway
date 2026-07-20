mod browser_request;
mod endpoint_scope;

pub const AISTUDIO_WEB_REVERSE_ADAPTER: &str = "aistudio_web_reverse_compatible";
pub const AISTUDIO_WEB_REVERSE_PROFILE: &str = "aistudio_web_reverse";
pub const AISTUDIO_BROWSER_EXECUTOR_PROVIDER_KEY: &str = "aistudio";

pub use browser_request::{build_browser_request_spec, AIStudioBrowserRequestSpec};
pub use endpoint_scope::supports_text_endpoint;
