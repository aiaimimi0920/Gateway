use rquest::header::HeaderMap;

use crate::protocol::gemini_canvas;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::insert_header_map_value;
use crate::upstream::gemini_canvas_request_headers::{
    apply_browser_fetch_client_hints, apply_gemini_canvas_browser_validation_headers,
};
use crate::upstream::gemini_canvas_runtime_helpers::origin_from_url;

#[cfg(test)]
pub(crate) fn build_gemini_canvas_direct_http_image_fetch_headers(
    payload: &ProviderAccountPayload,
    request_url: &str,
    page_origin: &str,
    page_referer: &str,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_header_map_value(
        &mut headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(
        &mut headers,
        "accept",
        "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8",
    );
    insert_header_map_value(
        &mut headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    insert_header_map_value(&mut headers, "referer", page_referer);
    apply_browser_fetch_client_hints(&mut headers, request_url, page_origin);
    insert_header_map_value(&mut headers, "sec-fetch-dest", "image");
    insert_header_map_value(&mut headers, "sec-fetch-mode", "no-cors");
    let target_origin = origin_from_url(request_url).unwrap_or_default();
    let site = if target_origin.eq_ignore_ascii_case(page_origin.trim()) {
        "same-origin"
    } else {
        "cross-site"
    };
    insert_header_map_value(&mut headers, "sec-fetch-site", site);
    headers.remove(rquest::header::ORIGIN);
    apply_gemini_canvas_browser_validation_headers(&mut headers);
    headers
}

pub(crate) fn build_gemini_canvas_direct_http_media_fetch_headers(
    payload: &ProviderAccountPayload,
    request_url: &str,
    page_origin: &str,
    page_referer: &str,
    asset_kind_hint: Option<&str>,
) -> HeaderMap {
    let mut headers = HeaderMap::new();
    insert_header_map_value(
        &mut headers,
        "user-agent",
        gemini_canvas::GEMINI_CANVAS_BROWSER_USER_AGENT,
    );
    insert_header_map_value(&mut headers, "accept", "*/*");
    insert_header_map_value(
        &mut headers,
        "accept-language",
        &gemini_canvas::locale_from_payload(payload),
    );
    insert_header_map_value(&mut headers, "referer", page_referer);
    apply_browser_fetch_client_hints(&mut headers, request_url, page_origin);
    let fetch_dest = match asset_kind_hint.map(|value| value.trim().to_ascii_lowercase()) {
        Some(kind) if kind == "audio" => "audio",
        Some(kind) if kind == "video" => "video",
        _ => "empty",
    };
    insert_header_map_value(&mut headers, "sec-fetch-dest", fetch_dest);
    insert_header_map_value(&mut headers, "sec-fetch-mode", "no-cors");
    let target_origin = origin_from_url(request_url).unwrap_or_default();
    let site = if target_origin.eq_ignore_ascii_case(page_origin.trim()) {
        "same-origin"
    } else {
        "cross-site"
    };
    insert_header_map_value(&mut headers, "sec-fetch-site", site);
    headers.remove(rquest::header::ORIGIN);
    apply_gemini_canvas_browser_validation_headers(&mut headers);
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use crate::routing::candidate::ProviderAccountPayload;
    use crate::upstream::header_map_helpers::header_map_string;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    #[test]
    fn build_gemini_canvas_direct_http_image_fetch_headers_preserve_image_contract() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let headers = build_gemini_canvas_direct_http_image_fetch_headers(
            &payload,
            "https://gemini.google.com/files/image.png",
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
        );

        assert_eq!(
            header_map_string(&headers, "accept").as_deref(),
            Some("image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8")
        );
        assert_eq!(
            header_map_string(&headers, "referer").as_deref(),
            Some("https://gemini.google.com/share/abc")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-dest").as_deref(),
            Some("image")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-mode").as_deref(),
            Some("no-cors")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-site").as_deref(),
            Some("same-origin")
        );
        assert!(header_map_string(&headers, "origin").is_none());
    }

    #[test]
    fn build_gemini_canvas_direct_http_media_fetch_headers_map_audio_cross_site_contract() {
        let payload = make_payload("gemini_canvas_compatible", "https://gemini.google.com");
        let headers = build_gemini_canvas_direct_http_media_fetch_headers(
            &payload,
            "https://lh3.googleusercontent.com/gg-dl/audio.mp3",
            "https://gemini.google.com",
            "https://gemini.google.com/share/abc",
            Some("audio"),
        );

        assert_eq!(
            header_map_string(&headers, "accept").as_deref(),
            Some("*/*")
        );
        assert_eq!(
            header_map_string(&headers, "referer").as_deref(),
            Some("https://gemini.google.com/share/abc")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-dest").as_deref(),
            Some("audio")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-mode").as_deref(),
            Some("no-cors")
        );
        assert_eq!(
            header_map_string(&headers, "sec-fetch-site").as_deref(),
            Some("cross-site")
        );
        assert!(header_map_string(&headers, "origin").is_none());
    }
}
