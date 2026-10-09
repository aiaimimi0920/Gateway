//! Native error envelope while preserving gateway status and retry/budget headers.
use crate::error::GatewayError;
use axum::{
    body::Body,
    response::{IntoResponse, Response},
};

pub struct DashScopeError(pub GatewayError);
impl From<GatewayError> for DashScopeError {
    fn from(value: GatewayError) -> Self {
        Self(value)
    }
}
impl IntoResponse for DashScopeError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "code": self.0.code.clone().unwrap_or_else(|| format!("{:?}",self.0.kind)),
            "message": self.0.message,
            "request_id": uuid::Uuid::new_v4().to_string(),
        });
        let mut response = self.0.into_response();
        *response.body_mut() = Body::from(body.to_string());
        response
    }
}
