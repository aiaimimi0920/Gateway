use crate::error::GatewayError;

pub fn unsupported_request_plan_error() -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas adapters use dedicated browser-backed reverse-web text/TTS/media send paths and are not supported by the generic request planner.",
    )
    .with_provider("gemini_canvas_compatible")
    .with_code("unsupported_gemini_canvas_endpoint")
}

pub fn unsupported_media_adapter_endpoint_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas adapters currently support /v1/images/generations, /v1/images/edits, /v1/music/generations, and /v1/videos/generations.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_endpoint")
}

pub fn unsupported_modular_endpoint_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas modular browser relay currently supports /v1/images/generations, /v1/music/generations, /v1/videos/generations, and /v1/audio/speech.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_endpoint")
}

pub fn unsupported_image_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas image generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_image_count")
}

pub fn unsupported_modular_image_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas browser relay image generation currently supports only n=1 requests.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_image_count")
}

pub fn unsupported_image_edit_count_error(provider: &str) -> GatewayError {
    GatewayError::bad_request("Gemini Canvas image edits currently support only n=1 requests.")
        .with_provider(provider)
        .with_code("unsupported_gemini_canvas_image_edit_count")
}

pub fn unsupported_modular_image_edits_error(provider: &str) -> GatewayError {
    GatewayError::bad_request(
        "Gemini Canvas modular browser relay does not implement image edits yet. Keep using the legacy mixed lane until the true browser-owned edit flow is split out.",
    )
    .with_provider(provider)
    .with_code("unsupported_gemini_canvas_modular_image_edits")
}
