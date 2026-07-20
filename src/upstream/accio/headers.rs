use crate::routing::candidate::ProviderAccountPayload;
use rquest::header::{HeaderMap, HeaderValue};

const ACCIO_MIN_SUPPORTED_VERSION: &str = "0.5.9";

fn insert_header(map: &mut HeaderMap, name: &'static str, value: &str) {
    if let Ok(parsed) = HeaderValue::from_str(value) {
        map.insert(name, parsed);
    }
}

fn parse_version_triplet(value: &str) -> (u64, u64, u64) {
    let mut parts = value
        .split('.')
        .map(str::trim)
        .map(|segment| segment.parse::<u64>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

pub fn coerce_supported_version(version: &str) -> String {
    let trimmed = version.trim();
    if trimmed.is_empty() {
        return ACCIO_MIN_SUPPORTED_VERSION.to_string();
    }
    if parse_version_triplet(trimmed) < parse_version_triplet(ACCIO_MIN_SUPPORTED_VERSION) {
        return ACCIO_MIN_SUPPORTED_VERSION.to_string();
    }
    trimmed.to_string()
}

pub fn effective_version(payload: &ProviderAccountPayload) -> String {
    let configured = payload
        .headers
        .get("version")
        .or_else(|| payload.headers.get("x-app-version"))
        .map(String::as_str)
        .unwrap_or(ACCIO_MIN_SUPPORTED_VERSION);
    coerce_supported_version(configured)
}

pub fn apply_runtime_headers(payload: &ProviderAccountPayload, map: &mut HeaderMap) {
    insert_header(map, "accept", "text/event-stream");
    insert_header(map, "user-agent", "node");
    let language = payload
        .headers
        .get("x-language")
        .or_else(|| payload.headers.get("language"))
        .map(String::as_str)
        .unwrap_or("zh-CN");
    insert_header(map, "accept-language", language);
    insert_header(map, "sec-fetch-mode", "cors");

    let version = effective_version(payload);
    insert_header(map, "version", version.as_str());
    insert_header(map, "x-app-version", version.as_str());
    insert_header(map, "x-language", language);

    let os = payload
        .headers
        .get("x-os")
        .or_else(|| payload.headers.get("os"))
        .map(String::as_str)
        .unwrap_or("win32");
    insert_header(map, "x-os", os);

    if let Some(app_key) = payload
        .headers
        .get("appKey")
        .or_else(|| payload.headers.get("app_key"))
        .map(String::as_str)
    {
        insert_header(map, "appKey", app_key);
    }

    if let Some(utdid) = payload
        .headers
        .get("utdid")
        .or_else(|| payload.headers.get("x-utdid"))
        .map(String::as_str)
    {
        insert_header(map, "utdid", utdid);
    }

    if let Some(cookie_header) = payload
        .headers
        .get("Cookie")
        .or_else(|| payload.headers.get("cookie"))
        .map(String::as_str)
    {
        insert_header(map, "cookie", cookie_header);
    }

    if let Some(cna) = payload
        .headers
        .get("x-cna")
        .or_else(|| payload.headers.get("cna"))
        .map(String::as_str)
    {
        insert_header(map, "x-cna", cna);
    }
}
