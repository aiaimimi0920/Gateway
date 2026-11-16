//! Scheduling uses the persisted absolute expiration, not a recreated lifetime after restart.
use crate::db::provider_credentials::refresh::Snapshot;
use std::time::Duration;
use time::OffsetDateTime;

pub(crate) fn is_due(snapshot: &Snapshot, now: OffsetDateTime, margin: Duration) -> bool {
    let expiry = snapshot
        .request
        .material
        .expires_at
        .as_deref()
        .expect("DB Accio snapshot has absolute expiration");
    let expiry = OffsetDateTime::parse(expiry, &time::format_description::well_known::Rfc3339)
        .expect("DB Accio snapshot has validated expiration");
    expiry <= now || (expiry - now).unsigned_abs() <= margin
}
