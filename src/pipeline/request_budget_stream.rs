//! Post-handoff timeout terminates the original stream; it never re-enters routing.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use tokio::time::{sleep_until, Instant, Sleep};

use super::{terminal_error, RequestBudget};
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};

/// Continuation of exactly one already-counted operation, never admission for another send.
/// Its owner cooperatively closes the original run instead of dropping it at the HTTP handoff.
pub(crate) struct AdmittedStream {
    budget: RequestBudget,
    attempt: u32,
}
impl RequestBudget {
    pub(crate) fn admitted_stream(&self) -> Result<AdmittedStream, crate::error::GatewayError> {
        let mut state = self.0.lock();
        super::check_lifetime(&mut state)?;
        if state.attempts == 0 {
            return Err(terminal_error(
                "stream_not_admitted",
                state.last_error.as_ref(),
            ));
        }
        Ok(AdmittedStream {
            budget: self.clone(),
            attempt: state.attempts,
        })
    }
}
impl AdmittedStream {
    pub(crate) fn check(&self) -> Result<(), crate::error::GatewayError> {
        let mut state = self.budget.0.lock();
        if let Some(reason) = state.stop {
            return Err(terminal_error(reason, state.last_error.as_ref()));
        }
        if state.attempts != self.attempt {
            return Err(terminal_error(
                "admitted_attempt_changed",
                state.last_error.as_ref(),
            ));
        }
        if state.deadline.is_some_and(|at| Instant::now() >= at) {
            state.stop = Some("deadline");
            return Err(terminal_error("deadline", state.last_error.as_ref()));
        }
        Ok(())
    }
}

pub struct BudgetStream<S> {
    inner: Option<Pin<Box<S>>>,
    deadline: Option<Pin<Box<Sleep>>>,
    budget: RequestBudget,
}

impl<S> BudgetStream<S> {
    pub(super) fn new(inner: S, budget: RequestBudget, deadline: Option<Instant>) -> Self {
        Self {
            inner: Some(Box::pin(inner)),
            deadline: deadline.map(|deadline| Box::pin(sleep_until(deadline))),
            budget,
        }
    }
}

impl<S, E> Stream for BudgetStream<S>
where
    S: Stream<Item = Result<Bytes, StreamError<E>>>,
{
    type Item = Result<Bytes, StreamError<E>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.inner.is_none() {
            return Poll::Ready(None);
        }
        if this
            .deadline
            .as_mut()
            .is_some_and(|deadline| deadline.as_mut().poll(cx).is_ready())
        {
            // Release transport before accounting or exposing the terminal frame.
            this.budget.mark_stopped("deadline");
            drop(this.inner.take());
            return Poll::Ready(Some(Err(ProtocolStreamError::invalid_data(
                "budget_exhausted:deadline",
            )
            .into())));
        }
        let result = this.inner.as_mut().unwrap().as_mut().poll_next(cx);
        if matches!(result, Poll::Ready(None) | Poll::Ready(Some(Err(_)))) {
            drop(this.inner.take());
        }
        result
    }
}

impl<S> Drop for BudgetStream<S> {
    fn drop(&mut self) {
        if self.inner.is_some() {
            self.budget.mark_stopped("cancelled");
            drop(self.inner.take());
        }
    }
}

#[cfg(test)]
#[path = "request_budget_stream_tests.rs"]
mod tests;
