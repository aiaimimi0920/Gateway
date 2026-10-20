//! Narrow provider capacity evidence; never scan echoed prompts or arbitrary JSON fields.
pub(crate) fn is_model_capacity(status: u16, code: Option<&str>, message: Option<&str>) -> bool {
    if !matches!(status, 400 | 429 | 503) {
        return false;
    }
    if let Some(code) = code {
        match code.trim().to_ascii_lowercase().as_str() {
            "model_capacity_exhausted" | "model_at_capacity" | "model_is_at_capacity" => {
                return true
            }
            "server_error" | "service_unavailable_error" | "" => {}
            // Explicit auth/validation/quota codes take precedence over prose.
            _ => return false,
        }
    }
    let Some(message) = message.filter(|text| text.len() <= 4096) else {
        return false;
    };
    let lower = message.trim().to_ascii_lowercase();
    [
        "selected model is at capacity",
        "model is at capacity",
        "no capacity available for model",
    ]
    .iter()
    .any(|prefix| {
        lower
            .strip_prefix(prefix)
            .is_some_and(|tail| tail.is_empty() || tail.starts_with(['.', ':', ' ', '\n']))
    })
}
