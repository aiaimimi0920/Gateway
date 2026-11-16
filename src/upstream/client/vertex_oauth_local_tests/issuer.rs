//! The issuer never forwards; each supported grant records one exact form and returns synthetic material.
use super::*;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
type Forms = Arc<Mutex<Vec<HashMap<String, String>>>>;
pub(super) struct Issuer {
    pub endpoint: String,
    pub forms: Forms,
    task: Option<tokio::task::JoinHandle<()>>,
}
impl Issuer {
    pub async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
        let forms = Forms::default();
        let app = Router::new()
            .route("/token", post(exchange))
            .with_state(forms.clone());
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self {
            endpoint,
            forms,
            task: Some(task),
        }
    }
    pub fn counts(&self) -> (usize, usize) {
        let forms = self.forms.lock().unwrap();
        (
            forms
                .iter()
                .filter(|f| f.get("grant_type").map(String::as_str) == Some("authorization_code"))
                .count(),
            forms
                .iter()
                .filter(|f| f.get("grant_type").map(String::as_str) == Some("refresh_token"))
                .count(),
        )
    }
    pub fn assert_grants(&self) {
        let forms = self.forms.lock().unwrap();
        assert_eq!(forms.len(), 3);
        for (index, form) in forms.iter().enumerate() {
            assert_eq!(
                form.get("client_id").unwrap(),
                if index == 2 {
                    "synthetic-public-client"
                } else {
                    CLIENT
                }
            );
            if index == 2 {
                assert!(!form.contains_key("client_secret"));
            } else {
                assert_eq!(form.get("client_secret").unwrap(), SECRET);
            }
            if index == 1 {
                assert_eq!(form.get("refresh_token").unwrap(), "synthetic-refresh-1");
                assert!(!form.contains_key("code"));
            } else {
                assert_eq!(form.get("code").unwrap(), "synthetic-code");
                assert!(!form.get("code_verifier").unwrap().is_empty());
                let redirect = url::Url::parse(form.get("redirect_uri").unwrap()).unwrap();
                assert_eq!(redirect.host_str(), Some("127.0.0.1"));
                assert!(redirect.port().is_some());
            }
        }
    }
    pub async fn stop(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    }
}
impl Drop for Issuer {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}
async fn exchange(
    State(forms): State<Forms>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let mut forms = forms.lock().unwrap();
    let grant = form.get("grant_type").map(String::as_str).unwrap_or("");
    let (access, refresh, expiry, rt_expiry) = match grant {
        "authorization_code" => {
            let next = forms
                .iter()
                .filter(|f| f.get("grant_type").map(String::as_str) == Some("authorization_code"))
                .count()
                + 1;
            (
                format!("synthetic-access-{next}"),
                format!("synthetic-refresh-{next}"),
                3600,
                7200,
            )
        }
        "refresh_token" => (REFRESHED_ACCESS.into(), REFRESHED_TOKEN.into(), 86400, 6000),
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    forms.push(form);
    Ok(Json(
        json!({"access_token":access,"refresh_token":refresh,"token_type":"Bearer",
        "expires_in":expiry,"refresh_token_expires_in":rt_expiry,"scope":crate::protocol::vertex_oauth::CLOUD_PLATFORM_SCOPE}),
    ))
}
