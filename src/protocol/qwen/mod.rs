#[cfg(feature = "line-qwen-official-core")]
pub mod official_api;
#[cfg(not(feature = "line-qwen-official-core"))]
#[path = "official_api_disabled.rs"]
pub mod official_api;

#[cfg(feature = "line-qwen-web-reverse")]
pub mod web_reverse;
#[cfg(not(feature = "line-qwen-web-reverse"))]
#[path = "web_reverse_disabled.rs"]
pub mod web_reverse;
