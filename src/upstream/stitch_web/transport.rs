//! Fixed Nemo HTTPS transport. Only server-issued XSRF refresh on read-only listing can retry once.
use super::{failure, session::StitchCookies, wire, ORIGIN};
use crate::error::GatewayError;
use futures::StreamExt;
use rquest::header::{HeaderMap, HeaderValue};
use serde_json::{json, Value};

pub(super) struct StitchHttp<'a> {
    client: &'a rquest::Client,
    pub cookies: StitchCookies,
    boot: wire::Bootstrap,
    counter: u32,
}

async fn body(response: rquest::Response, max: usize) -> Result<(String, HeaderMap), GatewayError> {
    let status = response.status().as_u16();
    if status != 200 {
        return Err(failure(
            if matches!(status, 300..=399 | 401 | 403) {
                "stitch_web_login_required"
            } else {
                "stitch_web_http_failed"
            },
            "Stitch HTTP 请求未通过；私人响应已省略，不会自动重试。",
        ));
    }
    if response.content_length().is_some_and(|n| n > max as u64) {
        return Err(failure(
            "stitch_web_response_limit",
            "Stitch 响应超过大小限制。",
        ));
    }
    let headers = response.headers().clone();
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|_| failure("stitch_web_network_failed", "Stitch HTTP 响应读取失败。"))?;
        if bytes.len().saturating_add(chunk.len()) > max {
            return Err(failure(
                "stitch_web_response_limit",
                "Stitch 响应超过大小限制。",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((
        String::from_utf8(bytes)
            .map_err(|_| failure("stitch_web_response_invalid", "Stitch 响应不是有效 UTF-8。"))?,
        headers,
    ))
}

fn headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert("user-agent",HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36"));
    h.insert(
        "accept-language",
        HeaderValue::from_static("en-US,en;q=0.9"),
    );
    h
}

impl<'a> StitchHttp<'a> {
    pub(super) async fn open(
        client: &'a rquest::Client,
        mut cookies: StitchCookies,
    ) -> Result<Self, GatewayError> {
        let mut h = headers();
        cookies.apply(ORIGIN, &mut h)?;
        let response = client
            .get(ORIGIN)
            .headers(h)
            .redirect(rquest::redirect::Policy::none())
            .send()
            .await
            .map_err(|_| failure("stitch_web_network_failed", "Stitch HTTP 页面连接失败。"))?;
        let (html, h) = body(response, 8 * 1024 * 1024).await?;
        cookies.receive(ORIGIN, &h)?;
        let mut owner = Self {
            client,
            cookies,
            boot: wire::bootstrap(&html)?,
            counter: 100001,
        };
        // The current product accepts LIST; its legacy user-info RPC can reject a valid session.
        let projects = owner.rpc(wire::LIST, &json!([]), "/").await?;
        if !projects.is_array() {
            return Err(failure(
                "stitch_web_auth_invalid",
                "Stitch 未返回有效的项目列表。",
            ));
        }
        Ok(owner)
    }

    pub(super) async fn rpc(
        &mut self,
        rpc: &str,
        params: &Value,
        source: &str,
    ) -> Result<Value, GatewayError> {
        let text = self.exchange(rpc, params, source).await?;
        match wire::decode(&text, rpc) {
            Ok(value) => Ok(value),
            Err(error) => {
                let Some(next) = wire::read_only_refresh(&text, rpc, &self.boot.at) else {
                    return Err(error);
                };
                self.boot.at = next;
                let text = self.exchange(rpc, params, source).await?;
                wire::decode(&text, rpc)
            }
        }
    }

    async fn exchange(
        &mut self,
        rpc: &str,
        params: &Value,
        source: &str,
    ) -> Result<String, GatewayError> {
        let address = format!("{ORIGIN}/_/Nemo/data/batchexecute");
        let mut h = headers();
        self.cookies.apply(&address, &mut h)?;
        for (key, value) in [
            ("origin", ORIGIN),
            ("referer", ORIGIN),
            ("x-same-domain", "1"),
            (
                "content-type",
                "application/x-www-form-urlencoded;charset=UTF-8",
            ),
        ] {
            h.insert(
                rquest::header::HeaderName::from_bytes(key.as_bytes()).unwrap(),
                HeaderValue::from_static(value),
            );
        }
        self.counter = self.counter.saturating_add(100000);
        let response = self
            .client
            .post(&address)
            .headers(h)
            .redirect(rquest::redirect::Policy::none())
            .query(&[
                ("rpcids", rpc),
                ("source-path", source),
                ("f.sid", &self.boot.sid),
                ("bl", &self.boot.build),
                ("hl", "en"),
                ("_reqid", &self.counter.to_string()),
                ("rt", "c"),
            ])
            .body(wire::form(rpc, params, &self.boot.at))
            .send()
            .await
            .map_err(|_| {
                failure(
                    "stitch_web_network_failed",
                    "Stitch RPC 连接失败；生成不会自动重放。",
                )
            })?;
        let (text, h) = body(response, 4 * 1024 * 1024).await?;
        self.cookies.receive(&address, &h)?;
        Ok(text)
    }
}
