use std::collections::HashMap;

use rquest::header::HeaderMap;

use crate::protocol::gemini::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_request_headers::apply_browser_fetch_client_hints;
use crate::upstream::headers::build_upstream_headers_with;

pub fn build_headers(
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
    model: &str,
) -> HeaderMap {
    let mut headers = build_upstream_headers_with(payload, extra_headers);
    let origin = payload.base_url.trim_end_matches('/').to_string();
    if !headers.contains_key("user-agent") {
        insert_header_map_value(
            &mut headers,
            "user-agent",
            surface::GEMINI_WEB_DEFAULT_USER_AGENT,
        );
    }
    if !headers.contains_key("accept-language") {
        insert_header_map_value(
            &mut headers,
            "accept-language",
            surface::GEMINI_WEB_DEFAULT_ACCEPT_LANGUAGE,
        );
    }
    if !headers.contains_key("origin") {
        insert_header_map_value(&mut headers, "origin", &origin);
    }
    if !headers.contains_key("referer") {
        insert_header_map_value(&mut headers, "referer", &format!("{origin}/"));
    }
    apply_browser_fetch_client_hints(&mut headers, &origin, &origin);
    if !headers.contains_key("x-same-domain") {
        insert_header_map_value(
            &mut headers,
            "x-same-domain",
            surface::GEMINI_WEB_DEFAULT_SAME_DOMAIN_HEADER,
        );
    }
    insert_header_map_value(
        &mut headers,
        "content-type",
        "application/x-www-form-urlencoded;charset=utf-8",
    );
    for (name, value) in surface::extract_model_headers(payload.extra_body.as_ref(), model) {
        if !headers.contains_key(name.as_str()) {
            insert_header_map_value(&mut headers, &name, &value);
        }
    }
    if !headers.contains_key(surface::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY) {
        let request_context = format!("[\"{}\",1]", uuid::Uuid::new_v4());
        insert_header_map_value(
            &mut headers,
            surface::GEMINI_WEB_REQUEST_CONTEXT_HEADER_KEY,
            &request_context,
        );
    }
    headers
}
