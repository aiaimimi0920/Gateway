//! Udio generation parameter precedence and outgoing JSON values.

use super::field_values::{
    read_number_fields, read_number_fields_from_object, read_object_bool, read_object_string,
    read_optional_bool, read_optional_string,
};
use super::request_options::prompt_from_request;
use super::UDIO_DEFAULT_MODEL_TYPE;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;
use serde_json::Map;
use serde_json::Number;
use serde_json::Value;

pub fn build_generate_request(
    req: &CanonicalRelayRequest,
    _model: &str,
) -> Result<Value, GatewayError> {
    let body_obj = req
        .raw_body
        .as_object()
        .ok_or_else(|| GatewayError::bad_request("Udio request body must be a JSON object."))?;

    let prompt = prompt_from_request(req)?;
    let lyrics = read_optional_string(
        body_obj,
        &[
            "lyricInput",
            "lyric_input",
            "custom_lyrics",
            "customLyrics",
            "lyrics",
        ],
    )
    .unwrap_or_default();
    let instrumental = read_optional_bool(body_obj, &["instrumental"]).unwrap_or(false);

    let mut gen_params = body_obj
        .get("gen_params")
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let sampler_options = body_obj
        .get("samplerOptions")
        .or_else(|| body_obj.get("sampler_options"))
        .and_then(|value| value.as_object());
    let lyrics_type = normalize_lyrics_type(body_obj, &gen_params, &lyrics, instrumental);
    let bypass_prompt_optimization = read_optional_bool(
        body_obj,
        &["bypass_prompt_optimization", "bypassPromptOptimization"],
    )
    .or_else(|| read_object_bool(&gen_params, &["bypass_prompt_optimization"]))
    .or(Some(false));
    let seed = read_number_fields(body_obj, &["seed"])
        .or_else(|| sampler_options.and_then(|value| read_number_fields(value, &["seed"])))
        .or_else(|| read_number_fields_from_object(&gen_params, &["seed"]))
        .or(Some(-1.0));
    let song_section_start =
        read_number_fields(body_obj, &["song_section_start", "songSectionStart"])
            .or_else(|| read_number_fields_from_object(&gen_params, &["song_section_start"]))
            .or(Some(0.0));
    let song_section_end = read_number_fields(body_obj, &["song_section_end", "songSectionEnd"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["song_section_end"]))
        .or(Some(1.0));
    let prompt_strength = read_number_fields(body_obj, &["prompt_strength", "promptStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["prompt_strength"]))
        .or(Some(0.5));
    let clarity_strength = read_number_fields(body_obj, &["clarity_strength", "clarityStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["clarity_strength"]))
        .or(Some(0.25));
    let lyrics_strength = read_number_fields(body_obj, &["lyrics_strength", "lyricsStrength"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["lyrics_strength"]))
        .or(Some(0.5));
    let generation_quality =
        read_number_fields(body_obj, &["generation_quality", "generationQuality"])
            .or_else(|| read_number_fields_from_object(&gen_params, &["generation_quality"]))
            .or(Some(0.75));
    let negative_prompt = read_optional_string(body_obj, &["negative_prompt", "negativePrompt"])
        .or_else(|| read_object_string(&gen_params, &["negative_prompt"]))
        .or(Some(String::new()));
    let model_type = read_optional_string(
        body_obj,
        &[
            "model_type",
            "modelType",
            "upstream_model_type",
            "upstreamModelType",
        ],
    )
    .or_else(|| read_object_string(&gen_params, &["model_type"]))
    .or(Some(UDIO_DEFAULT_MODEL_TYPE.to_string()));
    let use_allegro = read_optional_bool(body_obj, &["use_allegro", "useAllegro"])
        .or_else(|| read_object_bool(&gen_params, &["use_allegro"]))
        .or(Some(true));
    let use_style = read_optional_bool(body_obj, &["use_style", "useStyle"])
        .or_else(|| read_object_bool(&gen_params, &["use_style"]))
        .or(Some(false));
    let bpm_enabled = read_optional_bool(body_obj, &["bpm_enabled", "bpmEnabled"])
        .or_else(|| read_object_bool(&gen_params, &["bpm_enabled"]))
        .or(Some(false));
    let bpm = read_number_fields(body_obj, &["bpm"])
        .or_else(|| read_number_fields_from_object(&gen_params, &["bpm"]));
    let num_auto_extend_samples = read_number_fields(
        body_obj,
        &["num_auto_extend_samples", "numAutoExtendSamples"],
    )
    .or_else(|| read_number_fields_from_object(&gen_params, &["num_auto_extend_samples"]))
    .or(Some(3.0));
    let audio_conditioning_path = read_optional_string(
        body_obj,
        &["audio_conditioning_path", "audioConditioningPath"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(value, &["audio_conditioning_path", "audioConditioningPath"])
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_path"]));
    let audio_conditioning_song_id = read_optional_string(
        body_obj,
        &["audio_conditioning_song_id", "audioConditioningSongId"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(
                value,
                &["audio_conditioning_song_id", "audioConditioningSongId"],
            )
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_song_id"]));
    let audio_conditioning_type = read_optional_string(
        body_obj,
        &["audio_conditioning_type", "audioConditioningType"],
    )
    .or_else(|| {
        sampler_options.and_then(|value| {
            read_optional_string(value, &["audio_conditioning_type", "audioConditioningType"])
        })
    })
    .or_else(|| read_object_string(&gen_params, &["audio_conditioning_type"]));
    let crop_start_time = read_number_fields(body_obj, &["crop_start_time", "cropStartTime"])
        .or_else(|| {
            sampler_options
                .and_then(|value| read_number_fields(value, &["crop_start_time", "cropStartTime"]))
        })
        .or_else(|| read_number_fields_from_object(&gen_params, &["crop_start_time"]));

    maybe_insert_string(&mut gen_params, "prompt", Some(prompt));
    maybe_insert_string(&mut gen_params, "lyrics", Some(lyrics.clone()));
    maybe_insert_string(&mut gen_params, "lyrics_type", Some(lyrics_type));
    maybe_insert_bool(
        &mut gen_params,
        "bypass_prompt_optimization",
        bypass_prompt_optimization,
    );
    maybe_insert_number(&mut gen_params, "seed", seed);
    maybe_insert_number(&mut gen_params, "song_section_start", song_section_start);
    maybe_insert_number(&mut gen_params, "song_section_end", song_section_end);
    maybe_insert_number(&mut gen_params, "prompt_strength", prompt_strength);
    maybe_insert_number(&mut gen_params, "clarity_strength", clarity_strength);
    maybe_insert_number(&mut gen_params, "lyrics_strength", lyrics_strength);
    maybe_insert_number(&mut gen_params, "generation_quality", generation_quality);
    maybe_insert_string(&mut gen_params, "negative_prompt", negative_prompt);
    maybe_insert_string(&mut gen_params, "model_type", model_type);
    maybe_insert_bool(&mut gen_params, "use_allegro", use_allegro);
    maybe_insert_bool(&mut gen_params, "use_style", use_style);
    maybe_insert_bool(&mut gen_params, "bpm_enabled", bpm_enabled);
    maybe_insert_number(&mut gen_params, "bpm", bpm);
    maybe_insert_number(
        &mut gen_params,
        "num_auto_extend_samples",
        num_auto_extend_samples,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_path",
        audio_conditioning_path,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_song_id",
        audio_conditioning_song_id,
    );
    maybe_insert_string(
        &mut gen_params,
        "audio_conditioning_type",
        audio_conditioning_type,
    );
    maybe_insert_number(&mut gen_params, "crop_start_time", crop_start_time);

    let mut config = body_obj
        .get("config")
        .and_then(|value| value.as_object().cloned())
        .or_else(|| {
            gen_params
                .get("config")
                .and_then(|value| value.as_object().cloned())
        })
        .unwrap_or_default();
    let config_mode =
        read_optional_string(body_obj, &["mode", "generation_mode", "generationMode"])
            .or_else(|| read_object_string(&config, &["mode"]))
            .or(Some("regular".to_string()));
    maybe_insert_string(&mut config, "mode", config_mode);
    gen_params.insert("config".to_string(), Value::Object(config));

    let mut body = Map::new();
    body.insert("gen_params".to_string(), Value::Object(gen_params));
    let requested_num_songs = read_number_fields(body_obj, &["num_songs", "numSongs", "n"])
        .map(|value| value.clamp(1.0, 2.0));
    if requested_num_songs
        .map(|value| (value.round() as i64) > 1)
        .unwrap_or(false)
    {
        maybe_insert_number(&mut body, "num_songs", requested_num_songs);
    }
    maybe_insert_string(
        &mut body,
        "webhook_url",
        read_optional_string(body_obj, &["webhook_url", "webhookUrl"]),
    );
    maybe_insert_bool(
        &mut body,
        "demoRequest",
        read_optional_bool(body_obj, &["demoRequest", "demo_request"]),
    );
    maybe_insert_string(
        &mut body,
        "captchaToken",
        read_optional_string(body_obj, &["captchaToken", "captcha_token"]),
    );

    Ok(Value::Object(body))
}

fn normalize_lyrics_type(
    body_obj: &Map<String, Value>,
    gen_params: &Map<String, Value>,
    lyrics: &str,
    instrumental: bool,
) -> String {
    if instrumental {
        return "instrumental".to_string();
    }

    let raw = read_optional_string(
        body_obj,
        &["lyrics_type", "lyricsType", "lyrics_mode", "lyricsMode"],
    )
    .or_else(|| read_object_string(gen_params, &["lyrics_type"]));

    match raw
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("instrumental") => "instrumental".to_string(),
        Some("user") | Some("lyricinput") => "user".to_string(),
        Some("generate") | Some("infer") | Some("lyricprompt") => "generate".to_string(),
        Some(other) if !other.is_empty() => other.to_string(),
        _ if !lyrics.trim().is_empty() => "user".to_string(),
        _ => "generate".to_string(),
    }
}

fn maybe_insert_string(target: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if !target.contains_key(key) {
        if let Some(value) = value {
            target.insert(key.to_string(), Value::String(value));
        }
    }
}

fn maybe_insert_bool(target: &mut Map<String, Value>, key: &str, value: Option<bool>) {
    if !target.contains_key(key) {
        if let Some(value) = value {
            target.insert(key.to_string(), Value::Bool(value));
        }
    }
}

fn maybe_insert_number(target: &mut Map<String, Value>, key: &str, value: Option<f64>) {
    if !target.contains_key(key) {
        if let Some(value) = value.and_then(number_from_f64_preserving_integers) {
            target.insert(key.to_string(), value);
        }
    }
}

fn number_from_f64_preserving_integers(value: f64) -> Option<Value> {
    if !value.is_finite() {
        return None;
    }
    if value.fract() == 0.0 {
        if value >= 0.0 && value <= u64::MAX as f64 {
            return Some(Value::Number(Number::from(value as u64)));
        }
        if value >= i64::MIN as f64 && value <= i64::MAX as f64 {
            return Some(Value::Number(Number::from(value as i64)));
        }
    }
    Number::from_f64(value).map(Value::Number)
}
