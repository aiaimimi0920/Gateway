//! One logical request owns all candidate/recovery send slots and one deadline.
//! No tasks are spawned: dropping the dispatch or stream cancels the shared owner.
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::time::{sleep_until, Instant};

use crate::error::{FallbackHint, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind, MessageRole};

#[path = "request_budget_stream.rs"]
mod stream;
pub use stream::BudgetStream;

pub const MAX_REQUEST_ATTEMPTS: u32 = 6;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct RequestBudget(Arc<Mutex<State>>);

struct State {
    started_at: Instant,
    deadline: Instant,
    max_attempts: u32,
    attempts: u32,
    stop: Option<&'static str>,
    handed_off: bool,
    last_error: Option<GatewayError>,
}

/// Conservatively disallow replay for create operations or any tool-bearing request.
/// This classifies replay risk, not whether the Gateway itself executes a tool.
pub fn replay_safe(req: &CanonicalRelayRequest) -> bool {
    req.tools.is_empty()
        && req.tool_choice.is_none()
        && !req
            .messages
            .iter()
            .any(|message| !message.tool_calls.is_empty() || message.role == MessageRole::Tool)
        && !matches!(
            req.endpoint_kind,
            EndpointKind::ImagesGenerations
                | EndpointKind::ImagesEdits
                | EndpointKind::MusicGenerations
                | EndpointKind::VideosGenerations
                | EndpointKind::AudioSpeech
                | EndpointKind::AudioTranscriptions
                | EndpointKind::ResearchCreate
        )
}

impl RequestBudget {
    pub fn for_request(req: &CanonicalRelayRequest) -> Self {
        Self::new(
            if replay_safe(req) {
                MAX_REQUEST_ATTEMPTS
            } else {
                1
            },
            REQUEST_TIMEOUT,
        )
    }

    /// Trusted internal limits; inbound headers/body cannot reset this owner.
    pub fn new(max_attempts: u32, timeout: Duration) -> Self {
        let now = Instant::now();
        Self(Arc::new(Mutex::new(State {
            started_at: now,
            deadline: now.checked_add(timeout).unwrap_or(now),
            max_attempts,
            attempts: 0,
            stop: None,
            handed_off: false,
            last_error: None,
        })))
    }

    pub fn attempts(&self) -> u32 {
        self.0.lock().attempts
    }

    pub fn remaining_attempts(&self) -> u32 {
        let state = self.0.lock();
        state.max_attempts.saturating_sub(state.attempts)
    }

    /// Tightening uses the original start, never candidate/recovery time.
    /// Reuse the existing policy field; corrupt persisted policy fails closed.
    pub fn configure(
        &self,
        req: &CanonicalRelayRequest,
        timeout_seconds: Option<i32>,
    ) -> Result<(), GatewayError> {
        if timeout_seconds.is_some_and(|seconds| !(1..=300).contains(&seconds)) {
            return Err(GatewayError::bad_request(
                "totalRequestTimeoutSeconds must be between 1 and 300",
            )
            .with_code("invalid_request_budget_policy"));
        }
        let mut state = self.0.lock();
        if !replay_safe(req) {
            state.max_attempts = state.max_attempts.min(1);
        }
        if let Some(seconds) = timeout_seconds {
            let deadline = state.started_at + Duration::from_secs(seconds as u64);
            state.deadline = state.deadline.min(deadline);
        }
        Ok(())
    }

    pub fn last_upstream_error(&self) -> Option<GatewayError> {
        self.0.lock().last_error.clone()
    }

    pub fn check(&self) -> Result<(), GatewayError> {
        check_state(&mut self.0.lock())
    }

    /// Called only after successful admission, immediately before polling a real send.
    pub fn begin_attempt(&self) -> Result<(), GatewayError> {
        let mut state = self.0.lock();
        check_state(&mut state)?;
        state.attempts += 1;
        Ok(())
    }

    pub fn observe_error(&self, error: &GatewayError) {
        if error.request_budget_stop_reason().is_none() {
            self.0.lock().last_error = Some(error.clone());
        }
    }

    pub fn stopped_reason(&self) -> Option<&'static str> {
        self.0.lock().stop
    }

    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, GatewayError>>,
    ) -> Result<T, GatewayError> {
        self.check()?;
        let mut cancellation = Cancellation {
            budget: self.clone(),
            armed: true,
        };
        let deadline = self.0.lock().deadline;
        let mut future = Box::pin(future);
        let result = tokio::select! {
            biased;
            _ = sleep_until(deadline) => Err(self.stop("deadline")),
            result = future.as_mut() => {
                if Instant::now() >= deadline {
                    // Mark the cause before dropping a late stream/transport and its callbacks.
                    let error = self.stop("deadline");
                    drop(result);
                    Err(error)
                } else {
                    if result.is_ok() {
                        self.0.lock().handed_off = true;
                    }
                    result
                }
            }
        };
        drop(future);
        cancellation.armed = false;
        result
    }

    pub fn constrain_stream<S>(&self, inner: S) -> BudgetStream<S> {
        let deadline = {
            let mut state = self.0.lock();
            state.handed_off = true;
            state.deadline
        };
        BudgetStream::new(inner, self.clone(), deadline)
    }

    fn stop(&self, reason: &'static str) -> GatewayError {
        let mut state = self.0.lock();
        let reason = *state.stop.get_or_insert(reason);
        terminal_error(reason, state.last_error.as_ref())
    }

    fn mark_stopped(&self, reason: &'static str) {
        self.0.lock().stop.get_or_insert(reason);
    }
}

fn check_state(state: &mut State) -> Result<(), GatewayError> {
    if let Some(reason) = state.stop {
        return Err(terminal_error(reason, state.last_error.as_ref()));
    }
    if state.handed_off {
        return Err(terminal_error(
            "response_handed_off",
            state.last_error.as_ref(),
        ));
    }
    let reason = state.stop.or_else(|| {
        if Instant::now() >= state.deadline {
            Some("deadline")
        } else if state.attempts >= state.max_attempts {
            Some("attempt_limit")
        } else {
            None
        }
    });
    if let Some(reason) = reason {
        state.stop = Some(reason);
        Err(terminal_error(reason, state.last_error.as_ref()))
    } else {
        Ok(())
    }
}

fn terminal_error(reason: &'static str, last: Option<&GatewayError>) -> GatewayError {
    // Preserve the upstream identity. A fixed Abort reason and HTTP headers expose
    // the Gateway stop separately; no request/credential data is appended.
    let mut error = last.cloned().unwrap_or_else(|| {
        GatewayError::service_unavailable("request budget exhausted").with_code("budget_exhausted")
    });
    error.retryable = false;
    error.fallback_hint = FallbackHint::Abort {
        reason: format!("budget_exhausted:{reason}"),
    };
    error
}

struct Cancellation {
    budget: RequestBudget,
    armed: bool,
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        if self.armed {
            self.budget.mark_stopped("cancelled");
        }
    }
}
