use std::collections::HashMap;

use rquest::header::HeaderMap;

use crate::protocol::chatgpt::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::headers::build_upstream_headers_with;

use super::common::insert_header_map_value;
use super::web_reverse::ChatGptWebRequestContext;

pub fn build_bootstrap_headers(
    request_context: &ChatGptWebRequestContext,
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
) -> HeaderMap {
    let mut headers = build_upstream_headers_with(payload, extra_headers);
    if !headers.contains_key("user-agent") {
        insert_header_map_value(&mut headers, "User-Agent", &request_context.user_agent);
    }
    if !headers.contains_key("accept-language") {
        insert_header_map_value(
            &mut headers,
            "Accept-Language",
            &request_context.accept_language,
        );
    }
    insert_header_map_value(
        &mut headers,
        "Accept",
        "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    );
    insert_header_map_value(&mut headers, "Sec-Fetch-Dest", "document");
    insert_header_map_value(&mut headers, "Sec-Fetch-Mode", "navigate");
    insert_header_map_value(&mut headers, "Sec-Fetch-Site", "none");
    insert_header_map_value(&mut headers, "Sec-Fetch-User", "?1");
    insert_header_map_value(&mut headers, "Upgrade-Insecure-Requests", "1");
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Mobile",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_MOBILE,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Platform",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_PLATFORM,
    );
    headers
}

pub fn build_request_headers(
    request_context: &ChatGptWebRequestContext,
    payload: &ProviderAccountPayload,
    extra_headers: Option<&HashMap<String, String>>,
    target_path: &str,
    accept: &str,
    content_type: Option<&str>,
) -> HeaderMap {
    let mut headers = build_upstream_headers_with(payload, extra_headers);
    insert_header_map_value(&mut headers, "User-Agent", &request_context.user_agent);
    insert_header_map_value(
        &mut headers,
        "Accept-Language",
        &request_context.accept_language,
    );
    insert_header_map_value(&mut headers, "Origin", &request_context.origin);
    insert_header_map_value(&mut headers, "Referer", &request_context.referer);
    insert_header_map_value(&mut headers, "Accept", accept);
    insert_header_map_value(&mut headers, "Cache-Control", "no-cache");
    insert_header_map_value(&mut headers, "Pragma", "no-cache");
    insert_header_map_value(&mut headers, "Priority", "u=1, i");
    insert_header_map_value(&mut headers, "Sec-Fetch-Dest", "empty");
    insert_header_map_value(&mut headers, "Sec-Fetch-Mode", "cors");
    insert_header_map_value(&mut headers, "Sec-Fetch-Site", "same-origin");
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Arch",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_ARCH,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Bitness",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_BITNESS,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Full-Version",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Full-Version-List",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_FULL_VERSION_LIST,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Mobile",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_MOBILE,
    );
    insert_header_map_value(&mut headers, "Sec-CH-UA-Model", "\"\"");
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Platform",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_PLATFORM,
    );
    insert_header_map_value(
        &mut headers,
        "Sec-CH-UA-Platform-Version",
        surface::CHATGPT_WEB_DEFAULT_SEC_CH_UA_PLATFORM_VERSION,
    );
    insert_header_map_value(&mut headers, "OAI-Device-Id", &request_context.device_id);
    insert_header_map_value(&mut headers, "OAI-Session-Id", &request_context.session_id);
    insert_header_map_value(&mut headers, "OAI-Language", &request_context.language_code);
    insert_header_map_value(
        &mut headers,
        "OAI-Client-Version",
        &request_context.client_version,
    );
    insert_header_map_value(
        &mut headers,
        "OAI-Client-Build-Number",
        &request_context.client_build_number,
    );
    insert_header_map_value(&mut headers, "X-OpenAI-Target-Path", target_path);
    insert_header_map_value(&mut headers, "X-OpenAI-Target-Route", target_path);
    if let Some(content_type) = content_type {
        insert_header_map_value(&mut headers, "Content-Type", content_type);
    }
    headers
}
