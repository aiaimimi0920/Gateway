//! Go transport settings and HTTP header serialization, independent of authentication.
use crate::{Error, Result};
use http::{header, HeaderMap, HeaderName, HeaderValue, Request};
use http_body::Body as _;
use wreq_proto::ext::{on_preserve_header, OnPreserveHeaderCallback};

pub(crate) fn options() -> wreq_proto::http2::Http2Options {
    use wreq_proto::http2::{Http2Options, PseudoId, PseudoOrder, SettingId, SettingsOrder};
    Http2Options::builder()
        .initial_window_size(4 * 1024 * 1024)
        .initial_connection_window_size(65535 + 1024 * 1024 * 1024)
        .max_frame_size(16384)
        .max_header_list_size(10 * 1024 * 1024)
        .enable_push(false)
        .header_table_size(None)
        .headers_pseudo_order(
            PseudoOrder::builder()
                .extend([
                    PseudoId::Authority,
                    PseudoId::Method,
                    PseudoId::Path,
                    PseudoId::Scheme,
                ])
                .build(),
        )
        .settings_order(
            SettingsOrder::builder()
                .extend([
                    SettingId::EnablePush,
                    SettingId::InitialWindowSize,
                    SettingId::MaxFrameSize,
                    SettingId::MaxHeaderListSize,
                ])
                .build(),
        )
        .build()
}

pub(crate) fn prepare(request: &mut Request<rquest::Body>, h2: bool) -> Result<()> {
    let size = request.body().size_hint().exact().ok_or(Error::Body)?;
    let uri = request.uri().clone();
    let method = request.method().clone();
    let headers = request.headers_mut();
    let connection = headers.get(header::CONNECTION).cloned();
    let named: Vec<_> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|h| h.split(','))
        .map(|s| s.trim().to_ascii_lowercase())
        .collect();
    let remove: Vec<_> = headers
        .keys()
        .filter(|key| {
            let key = key.as_str();
            key.starts_with("x-gateway-")
                || named.iter().any(|name| name == key)
                || matches!(
                    key,
                    "connection"
                        | "proxy-connection"
                        | "proxy-authorization"
                        | "proxy-authenticate"
                        | "keep-alive"
                        | "upgrade"
                        | "te"
                        | "trailer"
                        | "transfer-encoding"
                        | "content-length"
                )
        })
        .cloned()
        .collect();
    for key in remove {
        headers.remove(key);
    }
    if !h2
        && connection
            .as_ref()
            .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"keep-alive"))
    {
        headers.insert(header::CONNECTION, connection.unwrap());
    }
    if uri.scheme_str() == Some("https") || !headers.contains_key(header::HOST) {
        headers.insert(
            header::HOST,
            HeaderValue::from_str(uri.authority().ok_or(Error::Target)?.as_str())
                .map_err(|_| Error::Target)?,
        );
    }
    if h2 {
        headers.remove(header::HOST);
    }
    if size > 0 || matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
        headers.insert(header::CONTENT_LENGTH, HeaderValue::from(size));
    }
    if !headers.contains_key(header::ACCEPT_ENCODING)
        && !headers.contains_key(header::RANGE)
        && method != http::Method::HEAD
    {
        headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("gzip"));
    }
    headers
        .entry(header::USER_AGENT)
        .or_insert(HeaderValue::from_static(if h2 {
            "Go-http-client/2.0"
        } else {
            "Go-http-client/1.1"
        }));
    // Go emits only the first User-Agent, and an explicit empty first value suppresses it.
    let agent = headers.get(header::USER_AGENT).cloned();
    headers.remove(header::USER_AGENT);
    if let Some(agent) = agent.filter(|value| !value.as_bytes().is_empty()) {
        headers.insert(header::USER_AGENT, agent);
    }
    if h2 {
        let cookies: Vec<_> = headers.get_all(header::COOKIE).iter().cloned().collect();
        headers.remove(header::COOKIE);
        for cookie in cookies {
            let mut remaining = cookie.as_bytes();
            // Match Go's splitter, not a semantic cookie parser: trim only spaces after ';'.
            while let Some(index) = remaining.iter().position(|byte| *byte == b';') {
                let mut value =
                    HeaderValue::from_bytes(&remaining[..index]).map_err(|_| Error::Body)?;
                value.set_sensitive(cookie.is_sensitive());
                headers.append(header::COOKIE, value);
                remaining = &remaining[index + 1..];
                while remaining.first() == Some(&b' ') {
                    remaining = &remaining[1..];
                }
            }
            if !remaining.is_empty() {
                let mut value = HeaderValue::from_bytes(remaining).map_err(|_| Error::Body)?;
                value.set_sensitive(cookie.is_sensitive());
                headers.append(header::COOKIE, value);
            }
        }
        // The dependency's Go encoder controls indexing without disabling Debug redaction.
    } else {
        *request.uri_mut() = uri
            .path_and_query()
            .map_or("/", |v| v.as_str())
            .parse()
            .map_err(|_| Error::Target)?;
    }
    on_preserve_header(request, GoHeaders);
    if h2 {
        request.extensions_mut().insert(http2::GoCompatibility);
    }
    Ok(())
}

struct GoHeaders;
impl OnPreserveHeaderCallback for GoHeaders {
    fn call(&self, headers: &mut HeaderMap) {
        // Ordinary Go HTTP/2 map order is random; content-length is always after that map.
        if let Some(length) = headers.remove(header::CONTENT_LENGTH) {
            headers.insert(header::CONTENT_LENGTH, length);
        }
    }
    fn call_visit(
        &self,
        headers: &mut HeaderMap,
        visit: &mut dyn FnMut(&dyn AsRef<[u8]>, &HeaderValue),
    ) {
        for name in [header::HOST, header::USER_AGENT, header::CONTENT_LENGTH] {
            for value in headers.get_all(&name) {
                visit(&canonical(&name), value);
            }
        }
        let mut names: Vec<_> = headers
            .keys()
            .filter(|k| !matches!(k.as_str(), "host" | "user-agent" | "content-length"))
            .collect();
        names.sort_by_key(|name| canonical(name));
        for name in names {
            for value in headers.get_all(name) {
                visit(&canonical(name), value);
            }
        }
    }
}
fn canonical(name: &HeaderName) -> Vec<u8> {
    let mut upper = true;
    name.as_str()
        .bytes()
        .map(|byte| {
            let result = if upper {
                byte.to_ascii_uppercase()
            } else {
                byte
            };
            upper = byte == b'-';
            result
        })
        .collect()
}
