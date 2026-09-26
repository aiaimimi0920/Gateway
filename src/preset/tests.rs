use super::*;
use crate::protocol::chataibot::CHATAIBOT_DEFAULT_MODEL;
use crate::protocol::gemini_business::NANO_BANANA_PRO_MODEL;
use crate::protocol::gemini_canvas::{
    GEMINI_CANVAS_DEFAULT_MODEL, GEMINI_CANVAS_DEFAULT_TEXT_MODEL,
};
use crate::protocol::kiro::{KIRO_DEFAULT_MODEL, KIRO_GENERATE_ASSISTANT_RESPONSE_PATH};
use crate::protocol::lumalabs::LUMALABS_DEFAULT_MODEL;
use crate::protocol::producer::PRODUCER_DEFAULT_MODEL;
use crate::protocol::udio::UDIO_DEFAULT_MODEL;
use crate::routing::candidate::SEARCH_API_COMPATIBLE_ADAPTER;

mod api_presets;
mod compile;
mod media_presets;
mod registry;
mod search_presets;
mod session_compile;
mod session_presets;
