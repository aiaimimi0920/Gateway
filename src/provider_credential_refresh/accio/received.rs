//! At most one confirmed HTTP result waits in memory; storage retries never re-enter HTTP.
use crate::console::refresh_grants::{GrantRecord, GrantResult};

pub(super) struct Received {
    pub sending: GrantRecord,
    pub result: GrantResult,
}
