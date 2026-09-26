use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Map, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use super::extract_image_type;
use crate::error::GatewayError;
use crate::protocol::canonical::CanonicalRelayRequest;

const PRODUCER_PUBLIC_ASSET_BASE_URL: &str =
    "https://storage.googleapis.com/producer-app-public/assets";

pub fn build_image_generation_response(
    body: &Value,
    req: &CanonicalRelayRequest,
    model: &str,
    auth_token: Option<&str>,
) -> Result<Value, GatewayError> {
    let body_obj = body.as_object().ok_or_else(|| {
        GatewayError::server_error("Producer.ai image response body must be a JSON object")
            .with_provider("producer_compatible")
            .with_code("producer_invalid_image_response")
    })?;
    let image_id = extract_image_id(body_obj)?;
    let image_type = request_image_type(req).unwrap_or_else(|| "clip".to_string());
    let image_url = extract_image_url(body)
        .or_else(|| derive_image_url_from_auth(auth_token, &image_type, &image_id));

    let created = body_obj
        .get("created")
        .cloned()
        .unwrap_or_else(|| json!(current_unix_timestamp()));

    let mut item = Map::new();
    item.insert("image_id".to_string(), Value::String(image_id.clone()));
    item.insert("type".to_string(), Value::String(image_type.clone()));
    if let Some(url) = image_url.clone() {
        item.insert("url".to_string(), Value::String(url));
    }

    let mut response = Map::new();
    response.insert("created".to_string(), created);
    response.insert("data".to_string(), Value::Array(vec![Value::Object(item)]));
    response.insert(
        "object".to_string(),
        Value::String("image.generation".to_string()),
    );
    response.insert(
        "provider".to_string(),
        Value::String("producer.ai".to_string()),
    );
    response.insert("model".to_string(), Value::String(model.to_string()));
    response.insert("image_id".to_string(), Value::String(image_id));
    response.insert("type".to_string(), Value::String(image_type));
    if image_url.is_none() {
        response.insert("upstream_response".to_string(), body.clone());
    }

    Ok(Value::Object(response))
}

fn extract_image_id(map: &Map<String, Value>) -> Result<String, GatewayError> {
    map.get("image_id")
        .or_else(|| map.get("imageId"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            GatewayError::server_error("Producer.ai image response missing image_id")
                .with_provider("producer_compatible")
                .with_code("producer_missing_image_id")
        })
}

fn request_image_type(req: &CanonicalRelayRequest) -> Option<String> {
    req.raw_body.as_object().and_then(extract_image_type)
}

fn extract_image_url(body: &Value) -> Option<String> {
    if let Some(url) = body
        .as_object()
        .and_then(|value| {
            value
                .get("image_url")
                .or_else(|| value.get("url"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Some(url.to_string());
    }

    body.get("data")
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .and_then(|value| value.as_object())
        .and_then(|item| {
            item.get("url")
                .or_else(|| item.get("image_url"))
                .and_then(|value| value.as_str())
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn derive_image_url_from_auth(
    auth_token: Option<&str>,
    image_type: &str,
    image_id: &str,
) -> Option<String> {
    let token = auth_token?.trim();
    if token.is_empty() {
        return None;
    }
    decode_auth_claims(token)?;
    let legacy_bucket = producer_image_bucket(image_type)?;
    if let ProducerImageBucket::UserScoped(bucket_name) = legacy_bucket {
        let _ = bucket_name;
    }

    // Flow Music keeps Supabase only as its auth issuer; generated image
    // assets now live in the public producer-app-public GCS bucket.
    Some(format!(
        "{PRODUCER_PUBLIC_ASSET_BASE_URL}/{}.jpg",
        image_id.trim()
    ))
}

fn decode_auth_claims(token: &str) -> Option<Value> {
    let mut segments = token.split('.');
    let _header = segments.next()?;
    let payload = segments.next()?;
    let decoded = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

enum ProducerImageBucket {
    UserScoped(&'static str),
    Space,
}

fn producer_image_bucket(image_type: &str) -> Option<ProducerImageBucket> {
    match image_type.trim().to_ascii_lowercase().as_str() {
        "clip" | "song" => Some(ProducerImageBucket::UserScoped("clips")),
        "playlist" => Some(ProducerImageBucket::UserScoped("playlists")),
        "space" => Some(ProducerImageBucket::Space),
        _ => None,
    }
}
