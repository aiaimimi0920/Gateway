// ---------------------------------------------------------------------------
// http/extractors.rs — custom axum extractors
// ---------------------------------------------------------------------------

use axum::{
    async_trait,
    extract::{FromRequest, FromRequestParts},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde_json::Value;

use crate::error::GatewayError;

// ---------------------------------------------------------------------------
// BearerToken extractor
// ---------------------------------------------------------------------------

/// Extracts the bearer token from the `Authorization: Bearer <token>` header.
///
/// Returns a 401 error if the header is absent or does not use the Bearer scheme.
pub struct BearerToken(pub String);

/// Rejection returned when bearer token extraction fails.
#[derive(Debug)]
pub struct BearerTokenRejection(GatewayError);

impl IntoResponse for BearerTokenRejection {
    fn into_response(self) -> Response {
        self.0.into_response()
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = BearerTokenRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok());

        match header {
            Some(value) if value.starts_with("Bearer ") => {
                let token = value["Bearer ".len()..].to_string();
                Ok(BearerToken(token))
            }
            _ => Err(BearerTokenRejection(GatewayError::unauthorized(
                "Missing or invalid Authorization header (expected: Bearer <token>)",
            ))),
        }
    }
}

/// Lenient variant that returns `None` instead of an error when no token is present.
pub struct OptionalBearerToken(pub Option<String>);

#[async_trait]
impl<S> FromRequestParts<S> for OptionalBearerToken
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(str::to_string);

        Ok(OptionalBearerToken(token))
    }
}

// ---------------------------------------------------------------------------
// JsonBody extractor — wraps axum::Json to return GatewayError on rejection
// ---------------------------------------------------------------------------

/// A wrapper around [`axum::Json`] that converts extraction failures (including
/// body-too-large rejections from [`RequestBodyLimitLayer`]) into JSON
/// [`GatewayError`] responses instead of plain-text axum defaults.
pub struct JsonBody(pub Value);

#[async_trait]
impl<S> FromRequest<S> for JsonBody
where
    S: Send + Sync,
{
    type Rejection = GatewayError;

    async fn from_request(req: axum::extract::Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<Value>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(JsonBody(value)),
            Err(rejection) => {
                let message = rejection.body_text();
                // Distinguish payload-too-large from other JSON errors.
                if message.contains("length limit")
                    || message.contains("payload too large")
                    || message.contains("Content length limit")
                {
                    Err(GatewayError {
                        kind: crate::error::ErrorKind::BadRequest,
                        message: "Request body too large".to_string(),
                        code: Some("request_too_large".to_string()),
                        http_status: Some(413),
                        retryable: false,
                        fallback_hint: crate::error::FallbackHint::Abort {
                            reason: "Reduce request body size.".to_string(),
                        },
                        provider_name: None,
                    })
                } else {
                    Err(GatewayError::bad_request(format!(
                        "Invalid JSON body: {}",
                        message
                    )))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{header::AUTHORIZATION, HeaderValue, Request};

    fn parts_with_auth(value: &str) -> Parts {
        let mut req = Request::new(());
        req.headers_mut()
            .insert(AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        req.into_parts().0
    }

    fn parts_without_auth() -> Parts {
        Request::new(()).into_parts().0
    }

    #[tokio::test]
    async fn bearer_token_extracted_successfully() {
        let mut parts = parts_with_auth("Bearer my-secret-token");
        let result = BearerToken::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "my-secret-token");
    }

    #[tokio::test]
    async fn bearer_token_rejects_missing_header() {
        let mut parts = parts_without_auth();
        let result = BearerToken::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn bearer_token_rejects_non_bearer_scheme() {
        let mut parts = parts_with_auth("Basic dXNlcjpwYXNz");
        let result = BearerToken::from_request_parts(&mut parts, &()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn optional_bearer_token_returns_some_when_present() {
        let mut parts = parts_with_auth("Bearer optional-token");
        let result = OptionalBearerToken::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, Some("optional-token".to_string()));
    }

    #[tokio::test]
    async fn optional_bearer_token_returns_none_when_absent() {
        let mut parts = parts_without_auth();
        let result = OptionalBearerToken::from_request_parts(&mut parts, &()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, None);
    }
}
