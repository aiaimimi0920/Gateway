//! Compile-time capability and startup policy jointly own the complete /ui surface.
use axum::Router;

pub(super) fn mount<S>(router: Router<S>, enabled: bool) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    #[cfg(feature = "management-ui")]
    if enabled {
        use super::ui;
        use axum::routing::get;
        return router
            .route("/ui", get(ui::redirect_ui_root))
            .route("/ui/", get(ui::serve_ui_index))
            .route("/ui/*path", get(ui::serve_ui_path));
    }
    let _ = enabled;
    router
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn ui_capability_and_setting_never_disable_the_api() {
        for enabled in [false, true] {
            let app = mount(Router::new().route("/api", get(|| async { "ok" })), enabled);
            let response = app
                .clone()
                .oneshot(Request::get("/api").body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            for path in ["/ui", "/ui/", "/ui/routes", "/ui/static/missing.js"] {
                let response = app
                    .clone()
                    .oneshot(Request::get(path).body(Body::empty()).unwrap())
                    .await
                    .unwrap();
                if !enabled || !cfg!(feature = "management-ui") {
                    assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
                } else if path == "/ui" {
                    assert_eq!(response.status(), StatusCode::PERMANENT_REDIRECT);
                } else if path.ends_with("missing.js") {
                    assert_eq!(response.status(), StatusCode::NOT_FOUND);
                } else {
                    assert_eq!(response.status(), StatusCode::OK, "{path}");
                }
            }
        }
    }
}
