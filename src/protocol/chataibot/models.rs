pub const CHATAIBOT_DEFAULT_MODEL: &str = "google-nano-banana-2";
const CHATAIBOT_FREE_TIER_MODEL_IDS: &[&str] =
    &["qwen-lora", "google-nano-banana-2", "gpt-image-1.5"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChataibotModelSpec {
    pub provider: &'static str,
    pub version: Option<&'static str>,
    pub generation_cost: u32,
    pub edit_mode: Option<&'static str>,
    pub edit_cost: Option<u32>,
    pub merge_mode: Option<&'static str>,
    pub merge_cost: Option<u32>,
}

pub fn resolve_model_spec(model: &str) -> Option<ChataibotModelSpec> {
    match model {
        "gpt-image-1.5" => Some(ChataibotModelSpec {
            provider: "GPT_IMAGE_1_5",
            version: None,
            generation_cost: 12,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        "gpt-image-1.5-high" => Some(ChataibotModelSpec {
            provider: "GPT_IMAGE_1_5_HIGH",
            version: None,
            generation_cost: 40,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        "ideogram" => Some(ChataibotModelSpec {
            provider: "IDEOGRAM",
            version: None,
            generation_cost: 8,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        "google-nano-banana-pro" => Some(ChataibotModelSpec {
            provider: "GOOGLE",
            version: Some("nano-banana-pro"),
            generation_cost: 60,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        "google-nano-banana" => Some(ChataibotModelSpec {
            provider: "GOOGLE",
            version: Some("nano-banana"),
            generation_cost: 15,
            edit_mode: Some("edit_google_nano_banana"),
            edit_cost: Some(15),
            merge_mode: Some("merge_google_nano_banana"),
            merge_cost: Some(15),
        }),
        "google-nano-banana-2" => Some(ChataibotModelSpec {
            provider: "GOOGLE",
            version: Some("nano-banana-2"),
            generation_cost: 30,
            edit_mode: Some("edit_google_nano_banana_2"),
            edit_cost: Some(30),
            merge_mode: Some("merge_google_nano_banana_2"),
            merge_cost: Some(30),
        }),
        "midjourney-7" => Some(ChataibotModelSpec {
            provider: "MIDJOURNEY",
            version: Some("7"),
            generation_cost: 20,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        "qwen-lora" => Some(ChataibotModelSpec {
            provider: "QWEN",
            version: Some("lora"),
            generation_cost: 2,
            edit_mode: Some("edit_qwen_lora"),
            edit_cost: Some(2),
            merge_mode: Some("merge_qwen_lora"),
            merge_cost: Some(2),
        }),
        "bytedance-seedream" => Some(ChataibotModelSpec {
            provider: "BYTEDANCE",
            version: Some("seedream-5-lite"),
            generation_cost: 14,
            edit_mode: None,
            edit_cost: None,
            merge_mode: None,
            merge_cost: None,
        }),
        _ => None,
    }
}

pub fn supported_model_ids() -> &'static [&'static str] {
    &[
        "gpt-image-1.5",
        "gpt-image-1.5-high",
        "ideogram",
        "google-nano-banana-pro",
        "google-nano-banana",
        "google-nano-banana-2",
        "midjourney-7",
        "qwen-lora",
        "bytedance-seedream",
    ]
}

pub fn free_tier_model_ids() -> &'static [&'static str] {
    CHATAIBOT_FREE_TIER_MODEL_IDS
}
