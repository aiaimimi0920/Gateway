//! Strict typed envelopes reject duplicate token fields; expiry is absolute, never a new TTL.
use super::{valid_atom, valid_cookie, OAuthError};
use serde::Deserialize;
use serde_json::value::RawValue;
use time::{format_description::well_known::Rfc3339, OffsetDateTime, UtcOffset};

/// Validated opaque grant material, not a JWT/principal assertion, account ID, quota or utdid.
pub struct TokenMaterial {
    access: String,
    refresh: String,
    expires_at: OffsetDateTime,
    sgcookie: Option<String>,
}
#[derive(Deserialize)]
struct Envelope {
    success: bool,
    data: Option<Box<RawValue>>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Tokens {
    access_token: String,
    refresh_token: String,
    expires_at: Box<RawValue>,
    sgcookie: Option<String>,
}

impl TokenMaterial {
    pub fn access_token(&self) -> &str {
        &self.access
    }
    pub fn refresh_token(&self) -> &str {
        &self.refresh
    }
    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }
    pub fn sgcookie(&self) -> Option<&str> {
        self.sgcookie.as_deref()
    }

    pub(super) fn parse(
        bytes: &[u8],
        previous_sgcookie: Option<String>,
    ) -> Result<Self, OAuthError> {
        let invalid = || OAuthError::InvalidResponse;
        let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        if !envelope.success {
            return Err(OAuthError::Rejected);
        }
        let tokens: Tokens = serde_json::from_str(envelope.data.ok_or_else(invalid)?.get())
            .map_err(|_| invalid())?;
        if !valid_atom(&tokens.access_token, 16_384)
            || !valid_atom(&tokens.refresh_token, 16_384)
            || tokens
                .sgcookie
                .as_deref()
                .is_some_and(|cookie| !valid_cookie(cookie))
        {
            return Err(invalid());
        }
        let expires_at = expiry(&tokens.expires_at).ok_or_else(invalid)?;
        Ok(Self {
            access: tokens.access_token,
            refresh: tokens.refresh_token,
            expires_at,
            sgcookie: tokens.sgcookie.or(previous_sgcookie),
        })
    }
}

pub(crate) fn expiry(raw: &RawValue) -> Option<OffsetDateTime> {
    let text = if raw.get().starts_with('"') {
        serde_json::from_str::<String>(raw.get()).ok()?
    } else {
        raw.get().to_owned()
    };
    // An explicit timezone is mandatory for dates. No local-time or default lifetime fallback.
    let at = if text.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        numeric_expiry(&text)?
    } else {
        OffsetDateTime::parse(&text, &Rfc3339).ok()?
    };
    (at.unix_timestamp_nanos() > 0).then(|| at.to_offset(UtcOffset::UTC))
}

fn numeric_expiry(text: &str) -> Option<OffsetDateTime> {
    if text.is_empty() || text.len() > 32 {
        return None;
    }
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty()
        || (text.contains('.') && fraction.is_empty())
        || fraction.len() > 9
        || !fraction.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let whole: i128 = whole.parse().ok()?;
    let part: i128 = if fraction.is_empty() {
        0
    } else {
        fraction.parse().ok()?
    };
    let scale = if whole > 1_000_000_000_000 || (whole == 1_000_000_000_000 && part > 0) {
        1_000_000 // Milliseconds according to the pinned wire reference.
    } else {
        1_000_000_000
    };
    // Exact decimal arithmetic; truncating sub-nanoseconds can never extend authorization.
    let nanos = whole
        .checked_mul(scale)?
        .checked_add(part.checked_mul(scale)? / 10i128.pow(fraction.len() as u32))?;
    OffsetDateTime::from_unix_timestamp_nanos(nanos).ok()
}
impl std::fmt::Debug for TokenMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AccioTokenMaterial { material: <redacted>, identity: unverified }")
    }
}
