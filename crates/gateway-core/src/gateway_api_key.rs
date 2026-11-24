use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;
const CANONICAL_GATEWAY_PROJECT_API_KEY_PREFIX: &str = "neuro_";
const LEGACY_GATEWAY_PROJECT_API_KEY_PREFIX: &str = "new_api_";
const CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX: &str = "nl_";
const LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX: &str = "nl_bundle_";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedGatewayProjectApiKey {
    pub token: String,
    pub api_key_id: String,
    pub signature: String,
    pub public_key_prefix: String,
}

pub fn build_gateway_project_api_key(
    api_key_id: &str,
    project_id: &str,
    tenant_id: &str,
    secret: &str,
) -> String {
    build_gateway_project_api_key_with_prefix(api_key_id, project_id, tenant_id, secret, "neuro")
}

pub fn build_gateway_project_api_key_with_prefix(
    api_key_id: &str,
    project_id: &str,
    tenant_id: &str,
    secret: &str,
    public_key_prefix: &str,
) -> String {
    let signature = create_gateway_api_key_signature(api_key_id, project_id, tenant_id, secret);
    match normalize_gateway_project_api_key_prefix(public_key_prefix).as_str() {
        "neuro" => {
            let encoded_id = URL_SAFE_NO_PAD.encode(api_key_id.as_bytes());
            format!("{CANONICAL_GATEWAY_PROJECT_API_KEY_PREFIX}{encoded_id}.{signature}")
        }
        "new_api" => {
            let encoded_id = URL_SAFE_NO_PAD.encode(api_key_id.as_bytes());
            format!("{LEGACY_GATEWAY_PROJECT_API_KEY_PREFIX}{encoded_id}.{signature}")
        }
        custom_prefix => format!("{custom_prefix}{api_key_id}.{signature}"),
    }
}

pub fn normalize_bundle_scoped_gateway_project_api_key_prefix(
    public_key_prefix: &str,
) -> Option<String> {
    let normalized = public_key_prefix.trim().to_ascii_lowercase();
    if let Some(remainder) = normalized.strip_prefix(LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX) {
        let bundle_id = remainder.strip_suffix('_')?;
        if bundle_id.is_empty() {
            return None;
        }
        return Some(format!(
            "{LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX}{bundle_id}_"
        ));
    }

    let remainder = normalized.strip_prefix(CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX)?;
    let mut parts = remainder.splitn(3, '_');
    let cost_type = parts.next()?;
    let bundle_id = parts.next()?;
    let trailing = parts.next()?;
    if !matches!(cost_type, "tk" | "tm" | "rq") || bundle_id.is_empty() || !trailing.is_empty() {
        return None;
    }
    Some(format!(
        "{CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX}{cost_type}_{bundle_id}_"
    ))
}

pub fn parse_gateway_project_api_key(
    raw_token: impl AsRef<str>,
) -> Option<ParsedGatewayProjectApiKey> {
    let token = raw_token.as_ref().trim();
    if let Some(body) = token.strip_prefix(CANONICAL_GATEWAY_PROJECT_API_KEY_PREFIX) {
        parse_legacy_gateway_project_api_key(token, body, "neuro")
    } else if let Some(body) = token.strip_prefix(LEGACY_GATEWAY_PROJECT_API_KEY_PREFIX) {
        parse_legacy_gateway_project_api_key(token, body, "new_api")
    } else if token.starts_with(LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX)
        || token.starts_with(CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX)
    {
        parse_bundle_gateway_project_api_key(token)
    } else {
        None
    }
}

pub fn verify_gateway_project_api_key(
    token: impl AsRef<str>,
    api_key_id: &str,
    project_id: &str,
    tenant_id: &str,
    secret: &str,
    expected_public_key_prefix: &str,
) -> bool {
    let Some(parsed) = parse_gateway_project_api_key(token) else {
        return false;
    };
    if parsed.api_key_id != api_key_id {
        return false;
    }
    if !gateway_project_api_key_prefix_matches(
        &parsed.public_key_prefix,
        normalize_gateway_project_api_key_prefix(expected_public_key_prefix).as_str(),
    ) {
        return false;
    }

    let expected = create_gateway_api_key_signature(api_key_id, project_id, tenant_id, secret);
    constant_time_eq(parsed.signature.as_bytes(), expected.as_bytes())
}

fn normalize_gateway_project_api_key_prefix(public_key_prefix: &str) -> String {
    let normalized = public_key_prefix.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "neuro" => "neuro".to_string(),
        "new_api" => "new_api".to_string(),
        _ => normalize_bundle_scoped_gateway_project_api_key_prefix(public_key_prefix)
            .unwrap_or(normalized),
    }
}

fn gateway_project_api_key_prefix_matches(actual: &str, expected: &str) -> bool {
    if expected == "neuro" {
        actual == "neuro" || actual == "new_api"
    } else {
        actual == expected
    }
}

fn parse_legacy_gateway_project_api_key(
    token: &str,
    body: &str,
    public_key_prefix: &str,
) -> Option<ParsedGatewayProjectApiKey> {
    let delimiter_index = body.find('.')?;
    if delimiter_index == 0 || delimiter_index == body.len() - 1 {
        return None;
    }

    let encoded_id = &body[..delimiter_index];
    let signature = body[delimiter_index + 1..].trim();
    if signature.is_empty() {
        return None;
    }

    let api_key_id = String::from_utf8(URL_SAFE_NO_PAD.decode(encoded_id).ok()?)
        .ok()?
        .trim()
        .to_string();
    if api_key_id.is_empty() {
        return None;
    }

    Some(ParsedGatewayProjectApiKey {
        token: token.to_string(),
        api_key_id,
        signature: signature.to_string(),
        public_key_prefix: public_key_prefix.to_string(),
    })
}

fn parse_bundle_gateway_project_api_key(token: &str) -> Option<ParsedGatewayProjectApiKey> {
    let delimiter_index = token.rfind('.')?;
    if delimiter_index == 0 || delimiter_index == token.len() - 1 {
        return None;
    }

    let body = &token[..delimiter_index];
    let signature = token[delimiter_index + 1..].trim();
    if signature.is_empty() {
        return None;
    }

    let (public_key_prefix, api_key_id) = if let Some(remainder) =
        body.strip_prefix(LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX)
    {
        let bundle_delimiter = remainder.find('_')?;
        let bundle_id = remainder[..bundle_delimiter].trim();
        let api_key_id = remainder[bundle_delimiter + 1..].trim();
        if bundle_id.is_empty() || api_key_id.is_empty() {
            return None;
        }
        (
            format!("{LEGACY_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX}{bundle_id}_"),
            api_key_id.to_string(),
        )
    } else {
        let remainder = body.strip_prefix(CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX)?;
        let mut parts = remainder.splitn(3, '_');
        let cost_type = parts.next()?;
        let bundle_id = parts.next()?;
        let api_key_id = parts.next()?.trim();
        if !matches!(cost_type, "tk" | "tm" | "rq") || bundle_id.is_empty() || api_key_id.is_empty()
        {
            return None;
        }
        (
            format!("{CUSTOM_BUNDLE_GATEWAY_PROJECT_API_KEY_PREFIX}{cost_type}_{bundle_id}_"),
            api_key_id.to_string(),
        )
    };

    Some(ParsedGatewayProjectApiKey {
        token: token.to_string(),
        api_key_id,
        signature: signature.to_string(),
        public_key_prefix,
    })
}

fn create_gateway_api_key_signature(
    api_key_id: &str,
    project_id: &str,
    tenant_id: &str,
    secret: &str,
) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(format!("{api_key_id}:{project_id}:{tenant_id}").as_bytes());
    let digest = mac.finalize().into_bytes();
    URL_SAFE_NO_PAD.encode(digest)[..32].to_string()
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_api_key_round_trip_matches_typescript_shape() {
        let token = build_gateway_project_api_key("api-key-1", "project-1", "tenant-1", "secret");
        assert!(token.starts_with("neuro_"));

        let parsed = parse_gateway_project_api_key(&token).expect("parse");
        assert_eq!(parsed.api_key_id, "api-key-1");
        assert!(verify_gateway_project_api_key(
            &token,
            "api-key-1",
            "project-1",
            "tenant-1",
            "secret",
            "neuro"
        ));
    }

    #[test]
    fn project_api_key_rejects_wrong_project_context() {
        let token = build_gateway_project_api_key("api-key-1", "project-1", "tenant-1", "secret");
        assert!(!verify_gateway_project_api_key(
            &token,
            "api-key-1",
            "project-2",
            "tenant-1",
            "secret",
            "neuro"
        ));
    }

    #[test]
    fn project_api_key_accepts_legacy_new_api_prefix() {
        let token = build_gateway_project_api_key("api-key-1", "project-1", "tenant-1", "secret");
        let legacy = token.replacen("neuro_", "new_api_", 1);
        let parsed = parse_gateway_project_api_key(&legacy).expect("legacy parse");
        assert_eq!(parsed.api_key_id, "api-key-1");
        assert!(verify_gateway_project_api_key(
            &legacy,
            "api-key-1",
            "project-1",
            "tenant-1",
            "secret",
            "neuro"
        ));
    }

    #[test]
    fn bundle_prefixed_project_api_key_round_trip() {
        let token = build_gateway_project_api_key_with_prefix(
            "6e1735ef-0562-46ef-be93-2a8e0bc255e8",
            "project-1",
            "tenant-1",
            "secret",
            "nl_tk_6e1735ef-0562-46ef-be93-2a8e0bc255e8_",
        );
        assert!(token.starts_with("nl_tk_6e1735ef-0562-46ef-be93-2a8e0bc255e8_"));
        let parsed = parse_gateway_project_api_key(&token).expect("bundle parse");
        assert_eq!(parsed.api_key_id, "6e1735ef-0562-46ef-be93-2a8e0bc255e8");
        assert!(verify_gateway_project_api_key(
            &token,
            "6e1735ef-0562-46ef-be93-2a8e0bc255e8",
            "project-1",
            "tenant-1",
            "secret",
            "nl_tk_6e1735ef-0562-46ef-be93-2a8e0bc255e8_",
        ));
    }

    #[test]
    fn bundle_prefixed_project_api_key_keeps_legacy_namespace_compatible() {
        let token = build_gateway_project_api_key_with_prefix(
            "6e1735ef-0562-46ef-be93-2a8e0bc255e8",
            "project-1",
            "tenant-1",
            "secret",
            "nl_bundle_6e1735ef-0562-46ef-be93-2a8e0bc255e8_",
        );
        let parsed = parse_gateway_project_api_key(&token).expect("legacy bundle parse");
        assert_eq!(
            parsed.public_key_prefix,
            "nl_bundle_6e1735ef-0562-46ef-be93-2a8e0bc255e8_"
        );
        assert!(verify_gateway_project_api_key(
            &token,
            "6e1735ef-0562-46ef-be93-2a8e0bc255e8",
            "project-1",
            "tenant-1",
            "secret",
            "nl_bundle_6e1735ef-0562-46ef-be93-2a8e0bc255e8_",
        ));
    }
}
