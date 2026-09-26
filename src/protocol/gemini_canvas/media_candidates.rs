use serde_json::Value;

use super::{
    media_mime::{infer_audio_mime_type, infer_image_mime_type, infer_video_mime_type},
    media_url_candidates::extract_media_asset_from_value_url_scan,
    types::*,
};

pub(super) fn extract_media_assets_from_candidate_data(
    value: &Value,
    operation: GeminiCanvasMediaOperation,
) -> Option<Vec<GeminiCanvasMediaAsset>> {
    match operation {
        GeminiCanvasMediaOperation::Image => extract_image_assets_from_candidate_data(value),
        GeminiCanvasMediaOperation::Music => {
            extract_music_asset_from_candidate_data(value).map(|asset| vec![asset])
        }
        GeminiCanvasMediaOperation::Video => {
            extract_video_asset_from_candidate_data(value).map(|asset| vec![asset])
        }
    }
}

fn extract_image_assets_from_candidate_data(value: &Value) -> Option<Vec<GeminiCanvasMediaAsset>> {
    let mut assets = Vec::new();
    if let Some(gen_images) = get_nested_value(value, &[12, 7, 0])
        .or_else(|| get_nested_value(value, &[7, 0]))
        .and_then(Value::as_array)
    {
        for gen_img_data in gen_images {
            if let Some(asset) = extract_image_asset_from_generated_image_data(gen_img_data) {
                assets.push(asset);
            }
        }
    }

    if let Some(image_to_image) = value
        .as_array()
        .and_then(|items| items.get(12))
        .and_then(|item| item.as_array())
        .and_then(|items| items.first())
        .and_then(|first| first.get("8"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_array)
        .or_else(|| {
            value
                .as_array()
                .and_then(|items| items.first())
                .and_then(|first| first.get("8"))
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_array)
        })
    {
        for gen_img_data in image_to_image {
            if let Some(asset) = extract_image_asset_from_generated_image_data(gen_img_data) {
                assets.push(asset);
            }
        }
    }

    if assets.is_empty() {
        None
    } else {
        Some(assets)
    }
}

pub(super) fn extract_video_asset_from_candidate_data(
    value: &Value,
) -> Option<GeminiCanvasMediaAsset> {
    get_nested_value(value, &[12, 59, 0, 0, 0])
        .or_else(|| get_nested_value(value, &[59, 0, 0, 0]))
        .and_then(extract_video_asset_from_video_info)
        .or_else(|| {
            extract_media_asset_from_value_url_scan(value, GeminiCanvasMediaOperation::Video)
        })
}

pub(super) fn extract_music_asset_from_candidate_data(
    value: &Value,
) -> Option<GeminiCanvasMediaAsset> {
    let direct_path = get_nested_value(value, &[12, 86])
        .or_else(|| get_nested_value(value, &[86]))
        .or_else(|| get_nested_value(value, &[12, 87]))
        .or_else(|| get_nested_value(value, &[87]));
    if let Some(media_data) = direct_path {
        let mp3_list = get_nested_value(media_data, &[0, 1, 7])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mp4_list = get_nested_value(media_data, &[1, 1, 7])
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mp3_url = mp3_list
            .get(1)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);
        let mp4_url = mp4_list
            .get(1)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);

        if let Some(url) = mp3_url {
            return Some(GeminiCanvasMediaAsset {
                kind: "audio".to_string(),
                mime_type: infer_audio_mime_type(&url),
                url,
                download_token: None,
                body_base64: None,
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }
        if let Some(url) = mp4_url {
            return Some(GeminiCanvasMediaAsset {
                kind: "video".to_string(),
                mime_type: infer_video_mime_type(&url),
                url,
                download_token: None,
                body_base64: None,
                alt: None,
                width: None,
                height: None,
                duration_seconds: None,
            });
        }
    }

    extract_media_asset_from_value_url_scan(value, GeminiCanvasMediaOperation::Music)
}

fn extract_image_asset_from_generated_image_data(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    let url = get_nested_value(value, &[0, 3, 3])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())?
        .to_string();
    let download_token = get_nested_value(value, &[0, 3, 5])
        .or_else(|| get_nested_value(value, &[0, 3, 4]))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let alt = get_nested_value(value, &[0, 3, 2])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);

    Some(GeminiCanvasMediaAsset {
        kind: "image".to_string(),
        mime_type: infer_image_mime_type(&url),
        url,
        download_token,
        body_base64: None,
        alt,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn extract_video_asset_from_video_info(value: &Value) -> Option<GeminiCanvasMediaAsset> {
    let urls = get_nested_value(value, &[0, 7])?.as_array()?;
    let thumbnail = urls
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(str::to_string);
    let url = urls
        .get(1)
        .or_else(|| urls.first())?
        .as_str()
        .map(str::trim)
        .filter(|entry| !entry.is_empty())?
        .to_string();

    Some(GeminiCanvasMediaAsset {
        kind: "video".to_string(),
        mime_type: infer_video_mime_type(&url),
        url,
        download_token: None,
        body_base64: None,
        alt: thumbnail,
        width: None,
        height: None,
        duration_seconds: None,
    })
}

fn get_nested_value<'a>(value: &'a Value, path: &[usize]) -> Option<&'a Value> {
    let mut current = value;
    for index in path {
        if let Some(items) = current.as_array() {
            current = items.get(*index)?;
            continue;
        }
        if let Some(map) = current.as_object() {
            current = map.get(&index.to_string())?;
            continue;
        }
        return None;
    }
    Some(current)
}
