use rquest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

use crate::routing::candidate::ProviderAccountPayload;

fn insert_header(map: &mut HeaderMap, key: &str, value: &str) {
    if let (Ok(name), Ok(header_value)) = (
        rquest::header::HeaderName::from_bytes(key.as_bytes()),
        HeaderValue::from_str(value),
    ) {
        map.insert(name, header_value);
    }
}

pub fn apply_runtime_headers(payload: &ProviderAccountPayload, map: &mut HeaderMap) {
    let session_auth = payload.session_auth.as_ref();
    let transport = session_auth
        .map(|auth| auth.transport.as_str())
        .unwrap_or("cookie");
    let primary_cookie_name = session_auth
        .and_then(|auth| auth.primary_cookie_name.as_deref())
        .unwrap_or("sso");
    let secondary_cookie_name = session_auth
        .and_then(|auth| auth.secondary_cookie_name.as_deref())
        .unwrap_or("sso-rw");
    let header_name = session_auth
        .and_then(|auth| auth.header_name.as_deref())
        .unwrap_or("authorization");

    match transport {
        "bearer" => {
            let bearer = format!("Bearer {}", payload.api_key);
            if let Ok(value) = HeaderValue::from_str(&bearer) {
                map.insert(AUTHORIZATION, value);
            }
        }
        "header" => insert_header(map, header_name, &payload.api_key),
        _ => {
            let cookie_value = format!(
                "{primary_cookie_name}={}; {secondary_cookie_name}={}",
                payload.api_key, payload.api_key
            );
            insert_header(map, "cookie", &cookie_value);
        }
    }

    insert_header(map, "x-xai-request-id", &uuid::Uuid::new_v4().to_string());
}
