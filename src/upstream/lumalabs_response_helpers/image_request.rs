//! Admit the local prompt-only image/output contract before potentially spending Web credits.
use crate::{
    error::GatewayError,
    protocol::{canonical::CanonicalRelayRequest, lumalabs},
};

pub(super) fn validate(req: &CanonicalRelayRequest) -> Result<(), GatewayError> {
    lumalabs::prefers_url_response(req)?;
    if req.raw_body.get("n").is_some_and(|n| n.as_u64() != Some(1)) {
        return Err(lumalabs::LumalabsMediaOperation::Image.unsupported_output_count_error());
    }
    // The current shared verifier fully decodes only PNG/JPEG. Reject other explicit formats
    // before submission, not after generating an asset we cannot truthfully return as verified.
    for key in ["output_format", "format"] {
        if req
            .raw_body
            .get(key)
            .is_some_and(|v| !matches!(v.as_str(), Some("png" | "jpeg")))
        {
            return Err(GatewayError::bad_request(
                "Luma Web 图片目前仅支持 output_format=png 或 jpeg。",
            )
            .with_provider("lumalabs_compatible")
            .with_code("unsupported_lumalabs_image_format"));
        }
    }
    let unsupported_images = req
        .raw_body
        .get("images")
        .is_some_and(|v| !v.as_array().is_some_and(|a| a.is_empty()));
    // Keep the existing images:[] compatibility, but do not silently discard malformed uploads.
    if req.raw_body.get("image").is_some()
        || req.raw_body.get("mask").is_some()
        || unsupported_images
    {
        return Err(lumalabs::unsupported_image_inputs_error());
    }
    Ok(())
}
