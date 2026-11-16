//! Carry transport-owned peer context to legacy management handlers without trusting headers.
use crate::{
    console::{ConsoleAuthRuntime, ConsoleRequestContext},
    error::GatewayError,
};
use axum::{
    extract::{ConnectInfo, Request},
    middleware::Next,
    response::Response,
};
use std::net::SocketAddr;

tokio::task_local! {
    static PEER_CONTEXT: Option<ConsoleRequestContext>;
}

pub(super) async fn scope(request: Request, next: Next) -> Response {
    let context = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|peer| {
            let origin = request
                .headers()
                .get("origin")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("local");
            ConsoleRequestContext::new(peer.0.ip(), origin.to_owned())
        });
    PEER_CONTEXT.scope(context, next.run(request)).await
}

pub(crate) fn authenticate(auth: &ConsoleAuthRuntime, token: &str) -> Result<(), GatewayError> {
    PEER_CONTEXT
        .try_with(|context| {
            let context = context
                .as_ref()
                .ok_or_else(|| GatewayError::unauthorized("Trusted peer context is required"))?;
            auth.authenticate_management_token(context, token)
                .map(|_| ())
        })
        .map_err(|_| GatewayError::unauthorized("Trusted peer context is required"))?
}
