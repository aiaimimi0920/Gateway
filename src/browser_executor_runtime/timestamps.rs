pub(super) fn now_rfc3339() -> String {
    use time::format_description::well_known::Rfc3339;

    time::OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "9999-12-31T23:59:59Z".to_string())
}

pub(super) fn future_rfc3339(offset_seconds: u64) -> Result<String, crate::error::GatewayError> {
    use crate::error::GatewayError;
    use time::format_description::well_known::Rfc3339;

    let invalid_ttl = || GatewayError::bad_request("leaseTtlSeconds exceeds the timestamp range");
    let seconds = i64::try_from(offset_seconds.max(1)).map_err(|_| invalid_ttl())?;
    let expires_at = time::OffsetDateTime::now_utc()
        .checked_add(time::Duration::seconds(seconds))
        .ok_or_else(invalid_ttl)?;
    expires_at.format(&Rfc3339).map_err(|_| invalid_ttl())
}
