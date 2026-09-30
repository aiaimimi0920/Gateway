// A fresh installation has a testing default; explicit configuration and an
// existing persisted administrator always retain ownership of authentication.
pub(super) fn resolve_management_token(
    configured: Option<String>,
    persisted_admin: bool,
) -> Option<String> {
    match configured {
        Some(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        None if persisted_admin => None,
        None => Some("11011101".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_management_token;

    #[test]
    fn default_does_not_replace_existing_administrators_or_explicit_policy() {
        assert_eq!(
            resolve_management_token(None, false).as_deref(),
            Some("11011101")
        );
        assert_eq!(resolve_management_token(None, true), None);
        assert_eq!(
            resolve_management_token(Some(" custom ".into()), true).as_deref(),
            Some("custom")
        );
        assert_eq!(resolve_management_token(Some(" ".into()), false), None);
    }
}
