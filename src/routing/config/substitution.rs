//! Environment substitution and unresolved optional keepalive handling.

use super::*;

/// Perform `${VAR}` environment-variable substitution in a string value.
/// Unknown variables are left as-is.
pub(crate) fn subst_env(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        if let Some(end) = rest.find('}') {
            let var_name = &rest[..end];
            rest = &rest[end + 1..];
            match std::env::var(var_name) {
                Ok(val) => result.push_str(&val),
                Err(_) => {
                    // Leave the placeholder intact so the operator can notice.
                    result.push_str("${");
                    result.push_str(var_name);
                    result.push('}');
                }
            }
        } else {
            // Unclosed `${` — emit literally and stop.
            result.push_str("${");
            result.push_str(rest);
            rest = "";
        }
    }
    result.push_str(rest);
    result
}

/// Apply env substitution to every string field in a `ProviderCredentialYaml`.
fn subst_credential(mut c: ProviderCredentialYaml) -> ProviderCredentialYaml {
    if let Some(ref mut url) = c.base_url {
        *url = subst_env(url);
    }
    if let Some(ref mut key) = c.api_key {
        *key = subst_env(key);
    }
    if let Some(ref mut token) = c.auth_token {
        *token = subst_env(token);
    }
    c.headers = c
        .headers
        .into_iter()
        .map(|(k, v)| (k, subst_env(&v)))
        .collect();
    if let Some(ref mut session) = c.session_auth {
        if let Some(ref mut name) = session.primary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut name) = session.secondary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut expires_at) = session.expires_at {
            *expires_at = subst_env(expires_at);
        }
    }
    if let Some(ref mut expires_at) = c.expires_at {
        *expires_at = subst_env(expires_at);
    }
    if let Some(ref mut object_key) = c.runtime_state_object_key {
        *object_key = subst_env(object_key);
    }
    if let Some(ref mut account_name) = c.account_name {
        *account_name = subst_env(account_name);
    }
    if let Some(ref mut keepalive) = c.keepalive {
        keepalive.service_url = subst_env(&keepalive.service_url);
        if let Some(ref mut path) = keepalive.ensure_path {
            *path = subst_env(path);
        }
        if let Some(ref mut token) = keepalive.auth_token {
            *token = subst_env(token);
        }
    }
    disable_unresolved_keepalive(&mut c.keepalive);
    c
}

/// Apply env substitution to every string field in a `ProviderConfigYaml`.
pub(super) fn subst_provider(mut p: ProviderConfigYaml) -> ProviderConfigYaml {
    p.base_url = subst_env(&p.base_url);
    p.api_key = subst_env(&p.api_key);
    if let Some(ref mut token) = p.auth_token {
        *token = subst_env(token);
    }
    if let Some(ref mut password) = p.credential_storage_password {
        *password = subst_env(password);
    }
    let new_headers: HashMap<String, String> = p
        .headers
        .into_iter()
        .map(|(k, v)| (k, subst_env(&v)))
        .collect();
    p.headers = new_headers;
    if let Some(ref mut session) = p.session_auth {
        if let Some(ref mut name) = session.primary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut name) = session.secondary_cookie_name {
            *name = subst_env(name);
        }
        if let Some(ref mut expires_at) = session.expires_at {
            *expires_at = subst_env(expires_at);
        }
    }
    if let Some(ref mut expires_at) = p.expires_at {
        *expires_at = subst_env(expires_at);
    }
    if let Some(ref mut object_key) = p.runtime_state_object_key {
        *object_key = subst_env(object_key);
    }
    if let Some(ref mut account_name) = p.account_name {
        *account_name = subst_env(account_name);
    }
    if let Some(ref mut keepalive) = p.keepalive {
        keepalive.service_url = subst_env(&keepalive.service_url);
        if let Some(ref mut path) = keepalive.ensure_path {
            *path = subst_env(path);
        }
        if let Some(ref mut token) = keepalive.auth_token {
            *token = subst_env(token);
        }
    }
    p.credentials = p.credentials.into_iter().map(subst_credential).collect();
    disable_unresolved_keepalive(&mut p.keepalive);
    p
}

fn disable_unresolved_keepalive(keepalive: &mut Option<KeepaliveConfig>) {
    let should_disable = keepalive.as_ref().is_some_and(|config| {
        let service_url = config.service_url.trim();
        service_url.is_empty() || service_url.contains("${")
    });
    if should_disable {
        *keepalive = None;
    }
}
