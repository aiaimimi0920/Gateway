//! Immutable, instance-owned profiles. Pools and trust remain separate for each client.
use rama_boring::{
    error::ErrorStack,
    ssl::{SslContextBuilder, SslRef},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Cpa,
    /// Measured Windows Antigravity hub 2.15.0, CL 983516863; no ALPN.
    AntigravityHub215,
}
impl Profile {
    pub(crate) fn context(self, context: &mut SslContextBuilder) -> Result<(), ErrorStack> {
        match self {
            Self::Cpa => gateway_cpa_tls_profile::configure_context(context),
            Self::AntigravityHub215 => crate::antigravity_tls::context(context),
        }
    }
    pub(crate) fn connection(self, ssl: &mut SslRef) -> Result<(), ErrorStack> {
        match self {
            Self::Cpa => gateway_cpa_tls_profile::configure_connection(ssl),
            Self::AntigravityHub215 => crate::antigravity_tls::connection(ssl),
        }
    }
}
