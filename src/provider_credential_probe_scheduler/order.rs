//! Bounded per-runtime round-robin admission, so short-interval plans cannot starve later plans.
use crate::state::AppState;
use std::sync::atomic::{AtomicUsize, Ordering};

pub(super) struct SweepOrder<'a> {
    cursor: &'a AtomicUsize,
    offset: usize,
    visited: usize,
    length: usize,
}

impl<'a> SweepOrder<'a> {
    pub(super) fn new(state: &'a AppState, length: usize) -> Self {
        let cursor = &state.lifecycle.credential_probe_cursor;
        let offset = cursor.load(Ordering::Relaxed);
        Self {
            cursor,
            offset: if length == 0 { 0 } else { offset % length },
            visited: 0,
            length,
        }
    }
    pub(super) fn offset(&self) -> usize {
        self.offset
    }
    pub(super) fn advance(&mut self) {
        self.visited += 1;
    }
}

impl Drop for SweepOrder<'_> {
    fn drop(&mut self) {
        if self.length > 0 {
            self.cursor.store(
                (self.offset + self.visited) % self.length,
                Ordering::Relaxed,
            );
        }
    }
}
