//! Parsed and malformed URL redaction use the same secret-classification boundary.

use super::classification::is_sensitive_key;
use super::SecretPatchError;
use url::Url;

pub(crate) fn redact_url_value(value: &str) -> String {
    let Ok(mut url) = Url::parse(value) else {
        return if raw_url_may_contain_secret(value) {
            "<redacted-url>".to_string()
        } else {
            value.to_string()
        };
    };

    let has_userinfo = !url.username().is_empty() || url.password().is_some();
    let has_fragment = url.fragment().is_some();
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    let has_sensitive_query = pairs.iter().any(|(key, _)| is_sensitive_key(key));
    if !has_userinfo && !has_sensitive_query && !has_fragment {
        return value.to_string();
    }

    if has_userinfo {
        let _ = url.set_password(None);
        let _ = url.set_username("");
    }
    if has_fragment {
        url.set_fragment(None);
    }
    if has_sensitive_query {
        let safe_pairs = pairs
            .iter()
            .filter(|(key, _)| !is_sensitive_key(key))
            .map(|(key, value)| (key.as_str(), value.as_str()));
        url.query_pairs_mut().clear().extend_pairs(safe_pairs);
        if pairs.iter().all(|(key, _)| is_sensitive_key(key)) {
            url.set_query(None);
        }
    }
    url.into()
}

pub(super) fn ensure_url_redacted(value: &str, path: &str) -> Result<(), SecretPatchError> {
    let unsafe_url = Url::parse(value).map_or_else(
        |_| raw_url_may_contain_secret(value),
        |url| {
            !url.username().is_empty()
                || url.password().is_some()
                || url
                    .query_pairs()
                    .any(|(key, _)| is_sensitive_key(key.as_ref()))
                || url.fragment().is_some()
        },
    );
    if unsafe_url {
        return Err(SecretPatchError::new(
            "secret_document_embedded_value",
            Some(path.to_string()),
            "draft URL still contains userinfo, a sensitive query parameter, or a fragment",
        ));
    }
    Ok(())
}

fn raw_url_may_contain_secret(value: &str) -> bool {
    let authority_has_userinfo = value
        .split_once("://")
        .map(|(_, remainder)| {
            remainder
                .split(['/', '?', '#'])
                .next()
                .is_some_and(|authority| authority.contains('@'))
        })
        .unwrap_or(false);
    let query_has_secret = value
        .split_once('?')
        .map(|(_, query)| query.split('#').next().unwrap_or(query))
        .into_iter()
        .flat_map(|query| url::form_urlencoded::parse(query.as_bytes()))
        .any(|(key, _)| is_sensitive_key(key.as_ref()));
    authority_has_userinfo || query_has_secret || value.split_once('#').is_some()
}
