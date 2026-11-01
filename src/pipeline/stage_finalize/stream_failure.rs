//! Audit cause is separate from refund/usage settlement and never invents an upstream failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamFailureCause {
    Upstream,
    Deadline,
    Shutdown,
    ClientDisconnected,
    ConsumerClosed,
}

impl StreamFailureCause {
    pub(crate) fn from_observation(stop: Option<&str>, upstream_failed: bool) -> Self {
        match stop {
            Some("deadline") => Self::Deadline,
            Some("shutdown") => Self::Shutdown,
            Some("caller_cancelled") => Self::ClientDisconnected,
            Some("cancelled") => Self::ConsumerClosed,
            _ if upstream_failed => Self::Upstream,
            _ => Self::ConsumerClosed,
        }
    }

    pub(super) fn status(self) -> &'static str {
        if self == Self::Upstream || self == Self::Deadline {
            "failed"
        } else {
            "cancelled"
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Upstream => "upstream_interrupted",
            Self::Deadline => "deadline",
            Self::Shutdown => "gateway_shutdown",
            Self::ClientDisconnected => "client_disconnected",
            // Drop proves consumption stopped, not why the user/client stopped it.
            Self::ConsumerClosed => "stream_consumer_closed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_not_an_upstream_failure_or_an_invented_disconnect() {
        for (stop, upstream, expected, status) in [
            (None, true, StreamFailureCause::Upstream, "failed"),
            (
                Some("deadline"),
                true,
                StreamFailureCause::Deadline,
                "failed",
            ),
            (
                Some("shutdown"),
                true,
                StreamFailureCause::Shutdown,
                "cancelled",
            ),
            (
                Some("caller_cancelled"),
                false,
                StreamFailureCause::ClientDisconnected,
                "cancelled",
            ),
            (
                Some("cancelled"),
                false,
                StreamFailureCause::ConsumerClosed,
                "cancelled",
            ),
            (None, false, StreamFailureCause::ConsumerClosed, "cancelled"),
        ] {
            let cause = StreamFailureCause::from_observation(stop, upstream);
            assert_eq!(cause, expected);
            assert_eq!(cause.status(), status);
        }
    }
}
