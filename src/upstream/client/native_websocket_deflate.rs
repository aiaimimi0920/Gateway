//! Offer and negotiation are owned by the pinned CLI fork, not a second parser.
use tungstenite_native::{
    extensions::compression::deflate::DeflateConfig, protocol::WebSocketConfig,
};

pub(super) fn config() -> WebSocketConfig {
    let mut config = WebSocketConfig::default();
    config.max_message_size = Some(16 * 1024 * 1024);
    config.max_frame_size = Some(16 * 1024 * 1024);
    config.extensions.permessage_deflate = Some(DeflateConfig::default());
    config
}
