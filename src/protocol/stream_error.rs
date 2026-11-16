//! Local protocol failures are not HTTP transport failures. Keep their source
//! chain and historical display text without constructing a client-owned error.
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ProtocolStreamError {
    source: serde_json::Error,
    classified: Option<Box<crate::error::GatewayError>>,
}

impl ProtocolStreamError {
    pub(crate) fn is_upstream_failure(&self) -> bool {
        let message = self.source.to_string();
        self.classified.is_some()
            || message.starts_with("responses_stream_upstream_error:")
            || message.starts_with("bedrock_stream_upstream_error:")
            || message.starts_with("accio_stream_upstream_error:")
            || message.starts_with("kiro_api_key_stream_upstream_error:")
    }
    pub(crate) fn invalid_data(message: impl Into<String>) -> Self {
        Self::from(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )))
    }

    /// Provider owners may attach a safe classification without impersonating a transport error.
    pub(crate) fn classified(
        message: impl Into<String>,
        error: crate::error::GatewayError,
    ) -> Self {
        let mut result = Self::invalid_data(message);
        result.classified = Some(Box::new(error));
        result
    }

    pub(crate) fn gateway_error(&self) -> Option<&crate::error::GatewayError> {
        self.classified.as_deref()
    }
}

impl From<serde_json::Error> for ProtocolStreamError {
    fn from(source: serde_json::Error) -> Self {
        Self {
            source,
            classified: None,
        }
    }
}

impl fmt::Display for ProtocolStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_upstream_failure() {
            return write!(formatter, "upstream stream failed: {}", self.source);
        }
        write!(formatter, "error decoding response body: {}", self.source)
    }
}

impl Error for ProtocolStreamError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// The transport is generic so protocol owners also work with a client whose
/// error type cannot be constructed locally. No variant implies a retry.
#[derive(Debug)]
pub enum StreamError<Transport> {
    Transport(Transport),
    NativeTransport(reqwest::Error),
    Protocol(ProtocolStreamError),
}

impl<Transport> From<ProtocolStreamError> for StreamError<Transport> {
    fn from(error: ProtocolStreamError) -> Self {
        Self::Protocol(error)
    }
}

impl<Transport: fmt::Display> fmt::Display for StreamError<Transport> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(error) => error.fmt(formatter),
            Self::NativeTransport(error) => error.fmt(formatter),
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl<Transport: Error + 'static> Error for StreamError<Transport> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::NativeTransport(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

#[cfg(test)]
#[path = "stream_error_tests.rs"]
mod tests;
