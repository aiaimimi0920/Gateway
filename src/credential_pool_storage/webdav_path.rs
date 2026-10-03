//! Canonical URL construction and untrusted DAV href containment checks.
use super::validate_object_key;
use anyhow::{anyhow, bail, Result};
use percent_encoding::percent_decode_str;
use url::Url;

pub(super) fn namespace_urls(
    endpoint: &str,
    namespace: &str,
    allow_http: bool,
) -> Result<(Url, Vec<Url>)> {
    if endpoint.len() > 4096
        || endpoint.trim() != endpoint
        || !safe_path_text(endpoint)
        || endpoint.split_once("://").is_some_and(|(_, rest)| {
            rest.split('/')
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
    {
        bail!("invalid WebDAV endpoint");
    }
    let mut url = Url::parse(endpoint).map_err(|_| anyhow!("invalid WebDAV endpoint"))?;
    if !(url.scheme() == "https" || allow_http && url.scheme() == "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("invalid WebDAV endpoint");
    }
    // Inspect the original spelling before URL parsing can normalize dot segments.
    let raw_path = endpoint
        .split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|i| &rest[i..]))
        .unwrap_or("");
    if !canonical_path(raw_path) {
        bail!("invalid WebDAV endpoint path");
    }
    let parts: Vec<_> = namespace.split('/').collect();
    if namespace.len() > 2048
        || parts.len() > 32
        || parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || !safe_path_text(part)
                || part.contains(['%', ':', '?', '#'])
        })
    {
        bail!("invalid WebDAV namespace");
    }
    let mut collections = Vec::with_capacity(parts.len());
    for part in parts {
        url.path_segments_mut()
            .map_err(|_| anyhow!("invalid WebDAV endpoint"))?
            .pop_if_empty()
            .push(part)
            .push("");
        collections.push(url.clone());
    }
    Ok((url, collections))
}

pub(super) fn href_key(href: &str, namespace: &Url) -> Option<String> {
    if href.len() > 8192 || !safe_path_text(href) || href.contains(['?', '#']) {
        return None;
    }
    let raw_path = if href.starts_with('/') && !href.starts_with("//") {
        href
    } else {
        let (_, rest) = href.split_once("://")?;
        if rest.split('/').next()?.contains('@') {
            return None;
        }
        &rest[rest.find('/')?..]
    };
    if !canonical_path(raw_path) {
        return None;
    }
    let url = namespace.join(href).ok()?;
    if url.origin() != namespace.origin() || !url.username().is_empty() || url.password().is_some()
    {
        return None;
    }
    let decoded = percent_decode_str(url.path()).decode_utf8().ok()?;
    let prefix = percent_decode_str(namespace.path()).decode_utf8().ok()?;
    let key = decoded.strip_prefix(prefix.as_ref())?;
    if key.contains('/') {
        return None;
    }
    validate_object_key(key).ok()?;
    Some(key.to_owned())
}

fn safe_path_text(value: &str) -> bool {
    !value.contains('\\') && !value.chars().any(char::is_control)
}

fn canonical_path(value: &str) -> bool {
    value.split('/').all(|part| {
        // Reject malformed/double encodings and encoded path separators before joining.
        let raw = part.as_bytes();
        let valid_escape = raw.iter().enumerate().all(|(i, byte)| {
            *byte != b'%'
                || raw.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                    && raw.get(i + 2).is_some_and(u8::is_ascii_hexdigit)
        });
        let Ok(decoded) = percent_decode_str(part).decode_utf8() else {
            return false;
        };
        valid_escape
            && decoded != "."
            && decoded != ".."
            && safe_path_text(&decoded)
            && !decoded.contains(['/', '%'])
    })
}
