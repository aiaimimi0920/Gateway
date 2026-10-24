//! Non-navigating presentation requests. This is not a native command/IPC endpoint.
use super::{panel, panel_request::Layout, ConnectionState};
use tauri::{http, Manager, UriSchemeContext, Url, Wry};

fn parse(
    label: &str,
    allowed_origin: Option<&str>,
    request: &http::Request<Vec<u8>>,
) -> Option<panel::Request> {
    let origin = request.headers().get(http::header::ORIGIN)?.to_str().ok()?;
    if label != "gateway-console"
        || allowed_origin != Some(origin)
        || origin == "null"
        || request.method() != http::Method::GET
        || !request.body().is_empty()
        || request.uri().to_string().len() > 2048
    {
        return None;
    }
    // Wry restores the Windows HTTPS workaround URL to this exact custom scheme.
    let url = Url::parse(&request.uri().to_string()).ok()?;
    if url.scheme() != "gateway-settings"
        || url.host_str() != Some("localhost")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    match url.path() {
        "/hide" if url.query().is_none() => Some(panel::Request::Hide),
        "/show" => {
            let mut canonical = Url::parse("gateway-window://settings/show").ok()?;
            canonical.set_query(url.query());
            Layout::parse(&canonical).map(panel::Request::Show)
        }
        _ => None,
    }
}

pub fn handle(
    context: UriSchemeContext<'_, Wry>,
    request: http::Request<Vec<u8>>,
) -> http::Response<Vec<u8>> {
    let app = context.app_handle();
    let origin = app
        .state::<ConnectionState>()
        .origin
        .lock()
        .ok()
        .and_then(|o| o.clone());
    let command = parse(context.webview_label(), origin.as_deref(), &request);
    let mut response = http::Response::builder().header(http::header::CACHE_CONTROL, "no-store");
    if let Some(command) = command {
        panel::enqueue(app, command);
        response = response
            .status(204)
            .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, origin.unwrap())
            .header(http::header::VARY, "Origin");
    } else {
        response = response.status(403);
    }
    response
        .body(Vec::new())
        .expect("fixed presentation response")
}

#[cfg(test)]
mod tests {
    use super::*;
    const ORIGIN: &str = "https://gateway.example";
    fn request(path: &str) -> http::Request<Vec<u8>> {
        http::Request::builder()
            .uri(format!("gateway-settings://localhost{path}"))
            .header("Origin", ORIGIN)
            .body(Vec::new())
            .unwrap()
    }
    fn show() -> http::Request<Vec<u8>> {
        let mut url = Url::parse("gateway-settings://localhost/show").unwrap();
        url.query_pairs_mut().append_pair("layout", r#"{"category":"backend","x":210,"y":160,"width":900,"height":600,"theme":"light","locale":"zh-CN"}"#);
        http::Request::builder()
            .uri(url.as_str())
            .header("Origin", ORIGIN)
            .body(Vec::new())
            .unwrap()
    }
    #[test]
    fn accepts_only_current_console_presentation() {
        assert!(matches!(
            parse("gateway-console", Some(ORIGIN), &request("/hide")),
            Some(panel::Request::Hide)
        ));
        assert!(matches!(
            parse("gateway-console", Some(ORIGIN), &show()),
            Some(panel::Request::Show(_))
        ));
        for label in ["main", panel::LABEL, "unknown"] {
            assert!(parse(label, Some(ORIGIN), &show()).is_none());
        }
        for origin in [
            None,
            Some("http://gateway.example"),
            Some("https://other.example"),
        ] {
            assert!(parse("gateway-console", origin, &show()).is_none());
        }
    }
    #[test]
    fn rejects_missing_null_origin_body_and_non_get_requests() {
        let mut input = show();
        input.headers_mut().remove("Origin");
        assert!(parse("gateway-console", Some(ORIGIN), &input).is_none());
        input
            .headers_mut()
            .insert("Origin", "null".parse().unwrap());
        assert!(parse("gateway-console", Some("null"), &input).is_none());
        let mut input = show();
        input.body_mut().push(1);
        assert!(parse("gateway-console", Some(ORIGIN), &input).is_none());
        for method in [
            http::Method::POST,
            http::Method::OPTIONS,
            http::Method::DELETE,
        ] {
            let mut input = show();
            *input.method_mut() = method;
            assert!(parse("gateway-console", Some(ORIGIN), &input).is_none());
        }
    }
    #[test]
    fn rejects_extra_fields_paths_and_wrong_schemes() {
        for path in [
            "/save_profile",
            "/hide?command=quit",
            "/show?layout={}",
            "/show?layout={}&path=C:/secret",
        ] {
            assert!(parse("gateway-console", Some(ORIGIN), &request(path)).is_none());
        }
        for url in [
            "https://localhost/hide",
            "gateway-settings://other/hide",
            "gateway-settings://localhost:42/hide",
        ] {
            let mut input = request("/hide");
            *input.uri_mut() = url.parse().unwrap();
            assert!(parse("gateway-console", Some(ORIGIN), &input).is_none());
        }
        assert!(parse(
            "gateway-console",
            Some(ORIGIN),
            &request(&format!("/show?layout={}", "x".repeat(2048)))
        )
        .is_none());
    }
}
