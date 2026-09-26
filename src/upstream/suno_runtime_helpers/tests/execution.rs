use super::*;

#[test]
fn prepare_suno_request_context_trims_base_url_and_preserves_headers() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
    payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".to_string(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".to_string()),
        expires_at: None,
    });
    payload
        .headers
        .insert("cookie".to_string(), "a=b".to_string());
    let mut req = make_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "prompt": "cover art"
    });

    let context = prepare_suno_request_context(&payload, &req, None, false)
        .expect("prepared suno request context");

    assert_eq!(context.prompt, "cover art");
    assert_eq!(context.base_url, "https://studio-api-prod.suno.com");
    assert_eq!(
        context
            .base_headers
            .get("cookie")
            .and_then(|value| value.to_str().ok()),
        Some("a=b")
    );
    assert_eq!(
        context
            .base_headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer sk-test")
    );
}

#[test]
fn prepare_suno_request_context_requires_bearer_for_direct_http_mode() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
    payload
        .headers
        .insert("cookie".to_string(), "a=b".to_string());
    let mut req = make_request(EndpointKind::MusicGenerations);
    req.raw_body = json!({
        "prompt": "neon synth chorus"
    });

    let error = prepare_suno_request_context(&payload, &req, None, false)
        .expect_err("missing bearer should fail");
    assert_eq!(error.http_status, Some(500));
    assert_eq!(error.code.as_deref(), Some("missing_suno_runtime_bearer"));
    assert_eq!(
        error.message.as_str(),
        "Suno requests require a runtime Clerk bearer token from keepalive ensure."
    );
}

#[test]
fn prepare_suno_execution_plan_reads_image_contract() {
    let mut req = make_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "prompt": "cover art",
        "waitCompletion": false,
        "waitTimeoutSecs": 5,
        "pollIntervalMs": 500
    });

    let prepared = prepare_suno_execution_plan(&req, Duration::from_secs(30), false)
        .expect("suno execution plan");

    assert!(prepared.missing_challenge_token);
    assert!(!prepared.wait_completion);
    assert_eq!(prepared.wait_timeout, Duration::from_secs(10));
    assert_eq!(prepared.poll_interval, Duration::from_millis(1_000));
    assert_eq!(prepared.request_timeout, Duration::from_secs(240));
    assert_eq!(prepared.target_asset_kind, "image");
}

#[test]
fn prepare_suno_execution_plan_reads_browser_video_contract() {
    let mut req = make_request(EndpointKind::VideosGenerations);
    req.raw_body = json!({
        "prompt": "cinematic teaser",
        "token": "captcha-ok",
        "wait_timeout_secs": 700,
        "poll_secs": 2
    });

    let prepared = prepare_suno_execution_plan(&req, Duration::from_secs(45), true)
        .expect("suno browser execution plan");

    assert!(!prepared.missing_challenge_token);
    assert!(prepared.wait_completion);
    assert_eq!(prepared.wait_timeout, Duration::from_secs(600));
    assert_eq!(prepared.poll_interval, Duration::from_millis(2_000));
    assert_eq!(prepared.request_timeout, Duration::from_secs(300));
    assert_eq!(prepared.target_asset_kind, "video");
}

#[test]
fn prepare_suno_execution_context_reads_http_contract() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
    payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".to_string(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".to_string()),
        expires_at: None,
    });
    payload
        .headers
        .insert("cookie".to_string(), "a=b".to_string());
    payload.extra_body = Some(HashMap::from([("userTier".to_string(), json!("pro"))]));
    let mut req = make_request(EndpointKind::ImagesGenerations);
    req.raw_body = json!({
        "prompt": "cover art"
    });

    let prepared =
        prepare_suno_execution_context(&payload, &req, None, false, Duration::from_secs(30))
            .expect("suno execution context");

    assert_eq!(prepared.prompt, "cover art");
    assert_eq!(prepared.base_url, "https://studio-api-prod.suno.com");
    assert_eq!(prepared.user_tier.as_deref(), Some("pro"));
    assert_eq!(prepared.execution_plan.target_asset_kind, "image");
    assert_eq!(
        prepared.execution_plan.request_timeout,
        Duration::from_secs(240)
    );
    assert_eq!(
        prepared
            .runtime_headers
            .get("authorization")
            .and_then(|value| value.to_str().ok()),
        Some("Bearer sk-test")
    );
    assert_eq!(
        prepared
            .runtime_headers
            .get("origin")
            .and_then(|value| value.to_str().ok()),
        Some("https://suno.com")
    );
    assert!(prepared.runtime_headers.get("device-id").is_some());
}

#[test]
fn prepare_suno_execution_context_reads_browser_contract() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
    payload.session_auth = Some(crate::credential_runtime::SessionAuthConfig {
        transport: "bearer".to_string(),
        primary_cookie_name: None,
        secondary_cookie_name: None,
        header_name: Some("authorization".to_string()),
        expires_at: None,
    });
    payload
        .headers
        .insert("cookie".to_string(), "a=b".to_string());
    let mut req = make_request(EndpointKind::VideosGenerations);
    req.raw_body = json!({
        "prompt": "cinematic teaser",
        "token": "captcha-ok"
    });

    let prepared =
        prepare_suno_execution_context(&payload, &req, None, true, Duration::from_secs(45))
            .expect("suno browser execution context");

    assert_eq!(prepared.execution_plan.target_asset_kind, "video");
    assert_eq!(
        prepared.execution_plan.request_timeout,
        Duration::from_secs(300)
    );
    assert!(!prepared.execution_plan.missing_challenge_token);
    assert_eq!(
        prepared
            .runtime_headers
            .get("referer")
            .and_then(|value| value.to_str().ok()),
        Some("https://suno.com/")
    );
    assert!(prepared.runtime_headers.get("browser-token").is_some());
}

#[test]
fn browser_backed_context_allows_cdp_session_to_own_auth() {
    let mut payload = make_payload("suno_compatible", "https://studio-api-prod.suno.com/");
    payload.api_key = String::new();
    payload.headers.clear();
    payload.session_auth = None;
    let mut req = make_request(EndpointKind::MusicGenerations);
    req.raw_body = json!({ "prompt": "Paris instrumental" });

    let prepared =
        prepare_suno_execution_context(&payload, &req, None, true, Duration::from_secs(45))
            .expect("browser session owns auth");

    assert!(prepared.runtime_headers.get("cookie").is_none());
    assert!(prepared.runtime_headers.get("authorization").is_none());
}

#[test]
fn build_suno_browser_executor_payload_preserves_runtime_header_and_wait_contract() {
    let mut runtime_headers = HeaderMap::new();
    runtime_headers.insert("cookie", HeaderValue::from_static("a=b"));
    runtime_headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer token-123"),
    );
    runtime_headers.insert("user-agent", HeaderValue::from_static("suno-agent"));
    runtime_headers.insert("accept-language", HeaderValue::from_static("en-US"));
    runtime_headers.insert("origin", HeaderValue::from_static("https://suno.com"));
    runtime_headers.insert(
        "referer",
        HeaderValue::from_static("https://suno.com/create"),
    );
    runtime_headers.insert("device-id", HeaderValue::from_static("device-1"));
    runtime_headers.insert("browser-token", HeaderValue::from_static("browser-1"));
    runtime_headers.insert("referring-pathname", HeaderValue::from_static("/create"));
    runtime_headers.insert(
        "referring-origin",
        HeaderValue::from_static("https://suno.com"),
    );

    let mut req = make_request(EndpointKind::VideosGenerations);
    req.raw_body = json!({
        "prompt": "cinematic stage clip",
        "wait_completion": false,
    });

    let payload = build_suno_browser_executor_payload(
        "https://studio-api-prod.suno.com",
        &runtime_headers,
        &req,
        "video",
        std::time::Duration::from_secs(150),
        std::time::Duration::from_secs(3),
        std::time::Duration::from_secs(300),
        Some("C:/browser/chrome.exe".to_string()),
    );

    assert_eq!(payload["baseUrl"], "https://studio-api-prod.suno.com");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["requestBody"]["prompt"], "cinematic stage clip");
    assert_eq!(payload["targetAssetKind"], "video");
    assert_eq!(payload["waitCompletion"], false);
    assert_eq!(payload["waitTimeoutMs"], 150_000u64);
    assert_eq!(payload["pollIntervalMs"], 3_000u64);
    assert_eq!(payload["timeoutMs"], 300_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "suno-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://suno.com");
    assert_eq!(payload["referer"], "https://suno.com/create");
    assert_eq!(payload["deviceId"], "device-1");
    assert_eq!(payload["browserToken"], "browser-1");
    assert_eq!(payload["referringPathname"], "/create");
    assert_eq!(payload["referringOrigin"], "https://suno.com");
}

#[test]
fn build_suno_browser_worker_input_preserves_runtime_header_and_wait_contract() {
    let mut runtime_headers = HeaderMap::new();
    runtime_headers.insert("cookie", HeaderValue::from_static("a=b"));
    runtime_headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer token-123"),
    );
    runtime_headers.insert("user-agent", HeaderValue::from_static("suno-agent"));
    runtime_headers.insert("accept-language", HeaderValue::from_static("en-US"));
    runtime_headers.insert("origin", HeaderValue::from_static("https://suno.com"));
    runtime_headers.insert(
        "referer",
        HeaderValue::from_static("https://suno.com/create"),
    );
    runtime_headers.insert("device-id", HeaderValue::from_static("device-1"));
    runtime_headers.insert("browser-token", HeaderValue::from_static("browser-1"));
    runtime_headers.insert("referring-pathname", HeaderValue::from_static("/create"));
    runtime_headers.insert(
        "referring-origin",
        HeaderValue::from_static("https://suno.com"),
    );
    let request_body = json!({ "prompt": "cinematic stage clip" });

    let input = build_suno_browser_worker_input(
        "https://studio-api-prod.suno.com",
        &runtime_headers,
        &request_body,
        "video",
        false,
        std::time::Duration::from_secs(150),
        std::time::Duration::from_secs(3),
        std::time::Duration::from_secs(300),
        Some("C:/browser/chrome.exe".to_string()),
    );
    let payload = serde_json::to_value(&input).expect("suno worker input");

    assert_eq!(payload["baseUrl"], "https://studio-api-prod.suno.com");
    assert_eq!(payload["cookieHeader"], "a=b");
    assert_eq!(payload["authToken"], "token-123");
    assert_eq!(payload["requestBody"]["prompt"], "cinematic stage clip");
    assert_eq!(payload["targetAssetKind"], "video");
    assert_eq!(payload["waitCompletion"], false);
    assert_eq!(payload["waitTimeoutMs"], 150_000u64);
    assert_eq!(payload["pollIntervalMs"], 3_000u64);
    assert_eq!(payload["timeoutMs"], 300_000u64);
    assert_eq!(payload["browserExecutablePath"], "C:/browser/chrome.exe");
    assert_eq!(payload["userAgent"], "suno-agent");
    assert_eq!(payload["acceptLanguage"], "en-US");
    assert_eq!(payload["origin"], "https://suno.com");
    assert_eq!(payload["referer"], "https://suno.com/create");
    assert_eq!(payload["deviceId"], "device-1");
    assert_eq!(payload["browserToken"], "browser-1");
    assert_eq!(payload["referringPathname"], "/create");
    assert_eq!(payload["referringOrigin"], "https://suno.com");
}

#[test]
fn prepare_suno_browser_executor_service_input_preserves_runtime_and_wait_contract() {
    let input = json!({
        "baseUrl": "https://studio-api-prod.suno.com",
        "cookieHeader": "a=b",
        "authToken": "token-123",
        "requestBody": { "prompt": "cinematic stage clip" },
        "targetAssetKind": "video",
        "waitCompletion": false,
        "waitTimeoutMs": 150_000u64,
        "pollIntervalMs": 3_000u64,
        "timeoutMs": 300_000u64,
        "userAgent": "suno-agent",
        "acceptLanguage": "en-US",
        "origin": "https://suno.com",
        "referer": "https://suno.com/create",
        "deviceId": "device-1",
        "browserToken": "browser-1",
        "referringPathname": "/create",
        "referringOrigin": "https://suno.com"
    });

    let prepared = prepare_suno_browser_executor_service_input(&input).expect("suno service input");

    assert_eq!(prepared.base_url, "https://studio-api-prod.suno.com");
    assert_eq!(prepared.request_body["prompt"], "cinematic stage clip");
    assert_eq!(prepared.target_asset_kind, "video");
    assert!(!prepared.wait_completion);
    assert_eq!(prepared.wait_timeout, std::time::Duration::from_secs(150));
    assert_eq!(prepared.poll_interval, std::time::Duration::from_secs(3));
    assert_eq!(prepared.timeout, std::time::Duration::from_secs(300));
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "authorization")
            .as_deref(),
        Some("Bearer token-123")
    );
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "cookie")
            .as_deref(),
        Some("a=b")
    );
    assert_eq!(
        crate::upstream::header_map_helpers::header_map_string(&prepared.headers, "browser-token")
            .as_deref(),
        Some("browser-1")
    );
}

#[test]
fn prepare_suno_browser_executor_service_input_requires_request_body_contract() {
    let input = json!({
        "baseUrl": "https://studio-api-prod.suno.com"
    });

    let error = prepare_suno_browser_executor_service_input(&input)
        .expect_err("missing request body should fail");
    assert_eq!(
        error.code.as_deref(),
        Some("browser_executor_missing_request_body")
    );
    assert_eq!(error.http_status, Some(400));
}
