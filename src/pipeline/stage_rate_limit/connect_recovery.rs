//! Short connector incidents are request-local, same-account and never detached tasks.
use crate::error::GatewayError;
use parking_lot::Mutex;
use std::time::Duration;
use tokio::time::Instant;

pub(super) const MAX_RETRIES: usize = 4;
const ADMISSION_WINDOW: Duration = Duration::from_secs(30);

pub(super) struct ConnectRecovery {
    started: Instant,
    last: Mutex<Option<GatewayError>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test(start_paused = true)]
    async fn recovery_admission_window_expires_even_without_a_request_deadline() {
        let recovery = ConnectRecovery::new();
        let error = GatewayError::service_unavailable("fixture network failure");
        assert!(recovery.remember(&error));
        tokio::time::advance(Duration::from_secs(30)).await;
        assert!(!recovery.remember(&error));
        assert_eq!(recovery.check().unwrap_err().message, error.message);
    }
}

impl ConnectRecovery {
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            last: Mutex::new(None),
        }
    }

    pub(super) fn remember(&self, error: &GatewayError) -> bool {
        *self.last.lock() = Some(error.clone());
        self.started.elapsed() < ADMISSION_WINDOW
    }

    pub(super) fn check(&self) -> Result<(), GatewayError> {
        if self.started.elapsed() >= ADMISSION_WINDOW {
            if let Some(error) = self.last.lock().clone() {
                return Err(error);
            }
        }
        Ok(())
    }
}
