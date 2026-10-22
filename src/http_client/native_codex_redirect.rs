//! Follow native same-origin redirects without granting arbitrary upstream egress.
pub(super) fn policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        if attempt.previous().len() >= 10 {
            return attempt.error("native redirect limit reached");
        }
        // The configured provider remains the authority. A redirect cannot switch
        // host/port/scheme, leak explicit headers, or downgrade HTTPS to HTTP.
        match attempt.previous().first() {
            Some(original) if original.origin() == attempt.url().origin() => attempt.follow(),
            _ => attempt.stop(),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::StatusCode, response::Redirect, routing::post, Router};

    #[tokio::test]
    async fn native_redirect_retains_post_body_and_stays_on_configured_origin() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let other = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let foreign = format!("http://{}/forbidden", other.local_addr().unwrap());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            let app = Router::new()
                .route("/start", post(|| async { Redirect::temporary("/finish") }))
                .route("/finish", post(|body: String| async move { body }))
                .route(
                    "/foreign",
                    post(move || {
                        let foreign = foreign.clone();
                        async move { Redirect::temporary(&foreign) }
                    }),
                )
                .route("/loop", post(|| async { Redirect::temporary("/loop") }));
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .await
                .unwrap();
        });
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(policy())
            .build()
            .unwrap();
        let response = client
            .post(format!("http://{address}/start"))
            .body("original body")
            .send()
            .await
            .unwrap();
        let status = response.status();
        let body = response.text().await.unwrap();
        let blocked = client
            .post(format!("http://{address}/foreign"))
            .send()
            .await
            .unwrap()
            .status();
        let foreign_request =
            tokio::time::timeout(std::time::Duration::from_millis(50), other.accept()).await;
        let looping = client.post(format!("http://{address}/loop")).send().await;
        drop(client);
        let _ = stop.send(());
        task.await.unwrap();
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "original body");
        assert_eq!(blocked, StatusCode::TEMPORARY_REDIRECT);
        assert!(foreign_request.is_err(), "cross-origin egress was allowed");
        assert!(looping.unwrap_err().is_redirect());
    }
}
