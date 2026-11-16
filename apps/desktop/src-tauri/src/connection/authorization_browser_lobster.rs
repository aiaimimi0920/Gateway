//! Only the fixed Lobster portal hash route with a local callback may leave the WebView.
use tauri::Url;
pub(super) fn allowed(url: &Url) -> bool {
    if url.as_str().len() > 8192
        || url.scheme() != "https"
        || url.host_str() != Some("lobsterai.youdao.com")
        || url.port_or_known_default() != Some(443)
        || url.path() != "/portal"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
    {
        return false;
    }
    let Some(query) = url.fragment().and_then(|f| f.strip_prefix("/login?")) else {
        return false;
    };
    let Ok(parsed) = Url::parse(&format!("https://example.invalid/?{query}")) else {
        return false;
    };
    let pairs: Vec<_> = parsed.query_pairs().collect();
    if pairs.len() != 3 {
        return false;
    }
    let mut values = std::collections::HashMap::new();
    for (key, value) in pairs {
        if !matches!(key.as_ref(), "source" | "state" | "redirect_uri")
            || values.insert(key, value).is_some()
        {
            return false;
        }
    }
    if values.get("source").map(|s| s.as_ref()) != Some("electron")
        || values.get("state").is_none_or(|s| {
            s.len() != 43
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        })
    {
        return false;
    }
    let Some(redirect) = values.get("redirect_uri") else {
        return false;
    };
    let Ok(callback) = Url::parse(redirect) else {
        return false;
    };
    callback.as_str() == redirect.as_ref()
        && callback.scheme() == "http"
        && callback.host_str() == Some("127.0.0.1")
        && callback.port().is_some_and(|p| p > 0)
        && callback.path() == "/auth/callback"
        && callback.query().is_none()
        && callback.fragment().is_none()
        && callback.username().is_empty()
        && callback.password().is_none()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn login(redirect: &str) -> Url {
        let mut query = Url::parse("https://example.invalid").unwrap();
        query.query_pairs_mut().extend_pairs([
            ("source", "electron"),
            ("state", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"),
            ("redirect_uri", redirect),
        ]);
        Url::parse(&format!(
            "https://lobsterai.youdao.com/portal#/login?{}",
            query.query().unwrap()
        ))
        .unwrap()
    }
    #[test]
    fn lobsterai_browser_allows_only_fixed_hash_login_with_local_callback() {
        let url = login("http://127.0.0.1:32123/auth/callback");
        assert!(allowed(&url));
        for redirect in [
            "https://example.invalid/callback",
            "http://localhost:1234/auth/callback",
            "http://127.0.0.1:1234/other",
            "http://user@127.0.0.1:1234/auth/callback",
            "http://127.0.0.1:1234/auth/callback?x=1",
        ] {
            assert!(!allowed(&login(redirect)));
        }
        for (from, to) in [
            ("/portal", "/else"),
            ("#/login?", "#/redirect?"),
            ("youdao.com", "youdao.com.evil.invalid"),
            ("https://", "http://"),
            ("source=electron", "source=electron&state=duplicate"),
            ("lobsterai.youdao.com", "user@lobsterai.youdao.com"),
            ("lobsterai.youdao.com", "lobsterai.youdao.com:444"),
        ] {
            assert!(!allowed(
                &Url::parse(&url.as_str().replace(from, to)).unwrap()
            ));
        }
    }
}
