use axum::body::Body;
use axum::extract::Path;
use axum::http::{
    header::{self, CACHE_CONTROL, CONTENT_SECURITY_POLICY},
    HeaderValue, Response, StatusCode,
};
use axum::response::{IntoResponse, Redirect};
use mime_guess::from_path;
include!(concat!(env!("OUT_DIR"), "/gateway_embedded_ui.rs"));

const INDEX_PATH: &str = "index.html";
const HTML_CACHE_CONTROL: &str = "no-store";
const IMMUTABLE_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const CONTENT_SECURITY_POLICY_VALUE: &str = "default-src 'self'; base-uri 'self'; frame-ancestors 'none'; img-src 'self' data:; font-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' http: https: ws: wss:";
const PERMISSIONS_POLICY_VALUE: &str =
    "accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()";

pub async fn redirect_ui_root() -> Redirect {
    Redirect::permanent("/ui/")
}

pub async fn serve_ui_index() -> Response<Body> {
    serve_index()
}

pub async fn serve_ui_path(Path(path): Path<String>) -> Response<Body> {
    let Some(normalized) = normalize_requested_path(&path) else {
        return not_found();
    };
    if let Some(response) = serve_exact_asset(&normalized) {
        return response;
    }
    if normalized.starts_with("static/") || last_segment_has_dot(&normalized) {
        return not_found();
    }
    serve_index()
}

fn serve_index() -> Response<Body> {
    serve_embedded_asset(INDEX_PATH, true).unwrap_or_else(not_found)
}

fn serve_exact_asset(path: &str) -> Option<Response<Body>> {
    asset_exists(path)
        .then(|| serve_embedded_asset(path, false))
        .flatten()
}

fn serve_embedded_asset(path: &str, html_shell: bool) -> Option<Response<Body>> {
    let asset = EmbeddedUi::get(path)?;
    let mut response = Response::new(Body::from(asset.data.into_owned()));
    *response.status_mut() = StatusCode::OK;

    let content_type = if html_shell {
        "text/html; charset=utf-8".to_string()
    } else {
        from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string()
    };
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).expect("valid embedded asset content-type"),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if html_shell {
            HTML_CACHE_CONTROL
        } else {
            IMMUTABLE_ASSET_CACHE_CONTROL
        }),
    );
    apply_security_headers(&mut response, html_shell);
    Some(response)
}

fn apply_security_headers(response: &mut Response<Body>, html_shell: bool) {
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        header::HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static(PERMISSIONS_POLICY_VALUE),
    );
    if html_shell {
        response.headers_mut().insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CONTENT_SECURITY_POLICY_VALUE),
        );
    }
}

fn asset_exists(path: &str) -> bool {
    EmbeddedUi::iter().any(|candidate| candidate == path)
}

fn normalize_requested_path(path: &str) -> Option<String> {
    let trimmed = path.trim_matches('/');
    if trimmed.is_empty() {
        return Some(INDEX_PATH.to_string());
    }
    let mut segments = Vec::new();
    for segment in trimmed.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.starts_with('.')
            || segment.contains('\\')
        {
            return None;
        }
        segments.push(segment);
    }
    Some(segments.join("/"))
}

fn last_segment_has_dot(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .map(|segment| segment.contains('.'))
        .unwrap_or(false)
}

fn not_found() -> Response<Body> {
    (StatusCode::NOT_FOUND, "Not Found").into_response()
}
