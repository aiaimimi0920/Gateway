//! The DB Accio steward consumes only database-owned grants, never route YAML or hosted cache IDs.
mod due;
mod lifecycle;
mod received;
mod send;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod server_fixture;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod server_flow;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
pub(crate) mod server_peer;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod server_runtime;
#[cfg(all(test, feature = "line-accio-web-reverse-api"))]
mod server_tests;
mod sweep;
pub(crate) use due::is_due;
pub(crate) use lifecycle::{join, spawn};
pub(crate) use sweep::Worker;

fn failed(code: &'static str) -> crate::error::GatewayError {
    crate::error::GatewayError::service_unavailable(
        "Accio database credential refresh did not complete",
    )
    .with_code(code)
}
