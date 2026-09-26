//! Secret-key taxonomy keeps credential values distinct from non-secret metadata.

pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let tokens = key_tokens(key);
    let compact = tokens.join("");
    if is_non_secret_metadata_key(&compact) {
        return false;
    }

    let exact = matches!(
        compact.as_str(),
        "authorization"
            | "proxyauthorization"
            | "cookie"
            | "setcookie"
            | "apikey"
            | "appkey"
            | "accesskey"
            | "secretkey"
            | "privatekey"
            | "token"
            | "session"
            | "sessionid"
            | "sessionkey"
            | "sapisid"
            | "secret"
            | "password"
            | "passwd"
            | "jwt"
            | "csrf"
            | "csrftoken"
            | "xsrf"
            | "xsrftoken"
    );
    let contains_secret_word = tokens
        .iter()
        .any(|token| matches!(token.as_str(), "secret" | "password" | "passwd"));
    let authorization_value = tokens.iter().any(|token| token == "authorization")
        && tokens.last().is_some_and(|token| {
            matches!(
                token.as_str(),
                "authorization" | "header" | "value" | "token"
            )
        });
    let sensitive_key_pair = tokens.windows(2).any(|pair| {
        matches!(
            (pair[0].as_str(), pair[1].as_str()),
            ("api", "key")
                | ("app", "key")
                | ("access", "key")
                | ("secret", "key")
                | ("private", "key")
        )
    });
    let token_value = tokens.iter().any(|token| token == "token")
        && tokens
            .last()
            .is_some_and(|token| matches!(token.as_str(), "token" | "value" | "secret"));
    let session_value = tokens.iter().any(|token| token == "session")
        && tokens.last().is_some_and(|token| {
            matches!(
                token.as_str(),
                "session" | "id" | "key" | "token" | "cookie" | "secret" | "value"
            )
        });
    let cookie_composite = tokens.iter().any(|token| token == "cookie")
        && tokens
            .last()
            .is_some_and(|token| matches!(token.as_str(), "value" | "header" | "token" | "secret"));

    exact
        || contains_secret_word
        || authorization_value
        || sensitive_key_pair
        || token_value
        || session_value
        || cookie_composite
        || compact.contains("awssecretaccesskey")
        || compact.contains("clientsecret")
        || compact.contains("authorizationheadervalue")
        || compact.ends_with("authorization")
        || compact.ends_with("cookie")
        || compact.ends_with("apikey")
        || compact.ends_with("token")
        || compact.ends_with("session")
        || compact.ends_with("secret")
        || compact.ends_with("password")
        || compact.ends_with("jwt")
}

fn key_tokens(key: &str) -> Vec<String> {
    let characters: Vec<char> = key.chars().collect();
    let mut tokens = Vec::new();
    let mut current = String::new();

    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_ascii_alphanumeric() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        let previous = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous));
        let next = characters.get(index + 1);
        let camel_boundary = character.is_ascii_uppercase()
            && !current.is_empty()
            && (previous.is_some_and(|previous| {
                previous.is_ascii_lowercase() || previous.is_ascii_digit()
            }) || (previous.is_some_and(|previous| previous.is_ascii_uppercase())
                && next.is_some_and(|next| next.is_ascii_lowercase())));
        if camel_boundary {
            tokens.push(std::mem::take(&mut current));
        }
        current.push(character.to_ascii_lowercase());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn is_non_secret_metadata_key(compact: &str) -> bool {
    matches!(
        compact,
        "maxtokens"
            | "maxoutputtokens"
            | "maxcompletiontokens"
            | "inputtokens"
            | "outputtokens"
            | "tokencount"
            | "tokentype"
            | "tokenlimit"
            | "sessiontimeout"
    ) || compact.starts_with("tokenexpires")
        || compact.ends_with("tokenexpires")
        || compact.ends_with("tokenexpiresinsecs")
        || compact.ends_with("tokenendpoint")
        || compact.ends_with("tokencount")
        || compact.ends_with("sessiontimeout")
}
