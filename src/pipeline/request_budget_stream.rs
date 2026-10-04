//! Post-handoff timeout terminates the original stream; it never re-enters routing.
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::Stream;
use tokio::time::{sleep_until, Instant, Sleep};

use super::RequestBudget;
use crate::protocol::stream_error::{ProtocolStreamError, StreamError};

pub struct BudgetStream<S> {
    inner: Option<Pin<Box<S>>>,
    deadline: Pin<Box<Sleep>>,
    budget: RequestBudget,
}

impl<S> BudgetStream<S> {
    pub(super) fn new(inner: S, budget: RequestBudget, deadline: Instant) -> Self {
        Self {
            inner: Some(Box::pin(inner)),
            deadline: Box::pin(sleep_until(deadline)),
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
        if this.deadline.as_mut().poll(cx).is_ready() {
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
