use crate::error::GatewayError;
use crate::protocol::gemini_canvas as legacy;

pub fn resolve_image_model(model: &str) -> Result<legacy::GeminiCanvasImageModel, GatewayError> {
    match model {
        legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GeminiCanvasImageModel::Gemini25FlashImagePreview)
        }
        legacy::GEMINI_25_FLASH_IMAGE_MODEL => {
            Ok(legacy::GeminiCanvasImageModel::Gemini25FlashImage)
        }
        legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GeminiCanvasImageModel::Gemini31FlashImagePreview)
        }
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas image model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_image_model")),
    }
}

pub fn resolve_official_image_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL | legacy::GEMINI_25_FLASH_IMAGE_MODEL => {
            Ok(legacy::GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL)
        }
        legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GEMINI_CANVAS_OFFICIAL_IMAGE_MODEL_PREVIEW)
        }
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas image model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_image_model")),
    }
}

pub fn resolve_direct_http_image_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL)
        }
        legacy::GEMINI_25_FLASH_IMAGE_MODEL => Ok(legacy::GEMINI_25_FLASH_IMAGE_MODEL),
        legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL => {
            Ok(legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL)
        }
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas image model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_image_model")),
    }
}

pub fn resolve_music_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas music model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_music_model")),
    }
}

pub fn resolve_official_music_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_OFFICIAL_MUSIC_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas music model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_music_model")),
    }
}

pub fn resolve_video_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas video model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_video_model")),
    }
}

pub fn resolve_official_video_model(model: &str) -> Result<&'static str, GatewayError> {
    match model {
        legacy::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL => Ok(legacy::GEMINI_CANVAS_OFFICIAL_VIDEO_MODEL),
        _ => Err(GatewayError::bad_request(format!(
            "Unsupported Gemini Canvas video model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_video_model")),
    }
}

pub fn resolve_text_model(model: &str) -> Result<&str, GatewayError> {
    match model {
        legacy::GEMINI_25_FLASH_IMAGE_PREVIEW_MODEL
        | legacy::GEMINI_25_FLASH_IMAGE_MODEL
        | legacy::GEMINI_31_FLASH_IMAGE_PREVIEW_MODEL
        | legacy::GEMINI_CANVAS_MUSIC_PREVIEW_MODEL
        | legacy::GEMINI_CANVAS_VIDEO_PREVIEW_MODEL => Err(GatewayError::bad_request(format!(
            "Gemini Canvas conversation/TTS endpoints do not support media-only model '{model}'."
        ))
        .with_code("unsupported_gemini_canvas_text_model")),
        _ if model.trim().is_empty() => Err(GatewayError::bad_request(
            "Gemini Canvas requests require a non-empty model name.",
        )
        .with_code("missing_gemini_canvas_text_model")),
        _ => Ok(model),
    }
}
