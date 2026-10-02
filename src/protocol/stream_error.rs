//! Local protocol failures are not HTTP transport failures. Keep their source
//! chain and historical display text without constructing a client-owned error.
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct ProtocolStreamError {
    source: serde_json::Error,
}

impl ProtocolStreamError {
    pub(crate) fn invalid_data(message: impl Into<String>) -> Self {
        Self::from(serde_json::Error::io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            message.into(),
        )))
    }
}

impl From<serde_json::Error> for ProtocolStreamError {
    fn from(source: serde_json::Error) -> Self {
        Self { source }
    }
}

impl fmt::Display for ProtocolStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error decoding response body: {}", self.source)
    }
}

impl Error for ProtocolStreamError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// The transport is generic so protocol owners also work with a client whose
/// error type cannot be constructed locally. Neither variant implies a retry.
#[derive(Debug)]
pub enum StreamError<Transport> {
    Transport(Transport),
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
            Self::Protocol(error) => error.fmt(formatter),
        }
    }
}

impl<Transport: Error + 'static> Error for StreamError<Transport> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Transport(error) => Some(error),
            Self::Protocol(error) => Some(error),
        }
    }
}

#[cfg(test)]
#[path = "stream_error_tests.rs"]
mod tests;
