pub(super) fn prompt_contains_aspect_ratio(prompt: &str, aspect_ratio: &str) -> bool {
    prompt.contains(aspect_ratio)
        || prompt
            .to_ascii_lowercase()
            .contains(&format!("aspect ratio {}", aspect_ratio).to_ascii_lowercase())
}

pub(super) fn prompt_contains_guidance_block(prompt: &str, guidance: &str) -> bool {
    prompt
        .to_ascii_lowercase()
        .contains(&guidance.to_ascii_lowercase())
}
