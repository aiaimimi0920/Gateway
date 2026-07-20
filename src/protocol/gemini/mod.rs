// Shared Gemini official protocol core.
// It is owned directly by AI Studio / Google Agent Platform official lines,
// and also reused by AI Studio Web Reverse direct-http replay plus
// ChatGPT Web Reverse cross-family Gemini ingress normalization.
#[cfg(any(
    feature = "line-aistudio-official",
    feature = "line-google-agent-platform-official",
    feature = "line-aistudio-web-reverse",
    feature = "line-chatgpt-web-reverse",
    feature = "line-qwen-web-reverse"
))]
pub mod api;
#[cfg(not(any(
    feature = "line-aistudio-official",
    feature = "line-google-agent-platform-official",
    feature = "line-aistudio-web-reverse",
    feature = "line-chatgpt-web-reverse",
    feature = "line-qwen-web-reverse"
)))]
#[path = "api_disabled.rs"]
pub mod api;

// Gemini Canvas program line owns this module tree.
#[cfg(feature = "line-gemini-canvas-program")]
pub mod canvas_program_web_reverse;
#[cfg(not(feature = "line-gemini-canvas-program"))]
#[path = "canvas_program_web_reverse_disabled.rs"]
pub mod canvas_program_web_reverse;
pub mod canvas_web_reverse;
pub mod shared;
// Gemini Web reverse protocol helpers are shared by:
// - Gemini Web Reverse
// - Gemini Canvas Program bootstrap / invoke helpers
#[cfg(any(
    feature = "line-gemini-web-reverse",
    feature = "line-gemini-canvas-program"
))]
pub mod web_reverse;
#[cfg(not(any(
    feature = "line-gemini-web-reverse",
    feature = "line-gemini-canvas-program"
)))]
#[path = "web_reverse_disabled.rs"]
pub mod web_reverse;
#[cfg(any(
    feature = "line-gemini-web-reverse",
    feature = "line-gemini-canvas-program"
))]
mod web_reverse_media_prompt;
