//! Bounded read-only HTTP exchange; no redirects, generation RPCs, workers or retries.
use super::{failure, session::FlowCookies, wire};
use crate::error::GatewayError;
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderValue};

const ORIGIN: &str = "https://flow.google.com";
const PAGE: &str = "https://flow.google.com/projects";
const RPC: &str = "https://flow.google.com/_/AiSandboxAngularFrontend/data/batchexecute";

fn headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static(crate::protocol::gemini_web::GEMINI_WEB_DEFAULT_USER_AGENT),
    );
    headers.insert("accept", HeaderValue::from_static("*/*"));
    headers.insert(
        "accept-language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    headers
}

async fn body(response: rquest::Response, max: usize) -> Result<(String, HeaderMap), GatewayError> {
    let status = response.status().as_u16();
    if status != 200 {
        return Err(failure(
            if matches!(status, 301..=399 | 401 | 403) {
                "flow_web_login_required"
            } else {
                "flow_web_http_failed"
            },
            "Flow HTTP 只读验证失败；不会跟随登录跳转，请重新登录或稍后重试。",
        ));
    }
    if response
        .content_length()
        .is_some_and(|size| size > max as u64)
    {
        return Err(failure(
            "flow_web_response_limit",
            "Flow HTTP 响应超过大小限制。",
        ));
    }
    let headers = response.headers().clone();
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            failure(
                "flow_web_network_failed",
                "Flow HTTP 读取失败；私人详情已省略。",
            )
        })?;
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(failure(
                "flow_web_response_limit",
                "Flow HTTP 响应超过大小限制。",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        String::from_utf8(bytes)
            .map_err(|_| failure("flow_web_response_invalid", "Flow HTTP 响应编码无效。"))?,
        headers,
    ))
}

pub(super) async fn read_credits(
    client: &rquest::Client,
    cookies: &mut FlowCookies,
) -> Result<u64, GatewayError> {
    let mut request_headers = headers();
    cookies.apply(PAGE, &mut request_headers)?;
    let response = client
        .get(PAGE)
        .headers(request_headers)
        .redirect(rquest::redirect::Policy::none())
        .send()
        .await
        .map_err(|_| {
            failure(
                "flow_web_network_failed",
                "Flow HTTP 连接失败；私人详情已省略。",
            )
        })?;
    let (html, response_headers) = body(response, 8 * 1024 * 1024).await?;
    let boot = wire::bootstrap(&html)?;
    cookies.receive(PAGE, &response_headers)?;
    let mut request_headers = headers();
    cookies.apply(RPC, &mut request_headers)?;
    for (key, value) in [
        ("origin", ORIGIN),
        ("referer", PAGE),
        ("x-same-domain", "1"),
        (
            "content-type",
            "application/x-www-form-urlencoded;charset=UTF-8",
        ),
    ] {
        request_headers.insert(
            rquest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            HeaderValue::from_static(value),
        );
    }
    let response = client
        .post(RPC)
        .headers(request_headers)
        .redirect(rquest::redirect::Policy::none())
        .query(&[
            ("rpcids", wire::CREDITS_RPC),
            ("source-path", "/projects"),
            ("bl", &boot.build),
            ("f.sid", &boot.sid),
            ("hl", "en"),
            ("_reqid", "100001"),
            ("rt", "c"),
        ])
        .body(wire::credits_form(boot.at.as_deref()))
        .send()
        .await
        .map_err(|_| {
            failure(
                "flow_web_network_failed",
                "Flow HTTP 只读 RPC 连接失败；私人详情已省略。",
            )
        })?;
    let (text, response_headers) = body(response, 1024 * 1024).await?;
    let credits = wire::credits(&text)?;
    cookies.receive(RPC, &response_headers)?;
    Ok(credits)
}
