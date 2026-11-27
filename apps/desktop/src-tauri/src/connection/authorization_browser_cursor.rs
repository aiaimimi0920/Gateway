//! Cursor PKCE links contain only public challenges, never tokens or arbitrary redirects.
use tauri::Url;

pub(super) fn allowed(url: &Url) -> bool {
    if url.as_str().len() > 1024
        || url.scheme() != "https"
        || url.host_str() != Some("www.cursor.com")
        || url.port_or_known_default() != Some(443)
        || url.path() != "/loginDeepControl"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    let mut seen = 0u8;
    for (key, value) in url.query_pairs() {
        let (bit, valid) = match key.as_ref() {
            "challenge" => (
                1,
                value.len() == 43
                    && value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')),
            ),
            "uuid" => (2, valid_uuid(&value)),
            "mode" => (4, value == "login"),
            "redirectTarget" => (8, value == "cli"),
            _ => return false,
        };
        if !valid || seen & bit != 0 {
            return false;
        }
        seen |= bit;
    }
    seen == 15
}

fn valid_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            14 => byte == b'4',
            19 => matches!(byte, b'8' | b'9' | b'a' | b'b'),
            _ => byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login() -> String {
        format!(
            "https://www.cursor.com/loginDeepControl?challenge={}&uuid=aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee&mode=login&redirectTarget=cli",
            "x".repeat(43)
        )
    }

    #[test]
    fn cursor_passes_the_actual_native_allowlist_and_keeps_a_single_url_argument() {
        let url = Url::parse(&login()).unwrap();
        assert!(super::super::allowed(&url));
        assert_eq!(
            super::super::browser_command(&url)
                .get_args()
                .last()
                .unwrap(),
            url.as_str()
        );
        assert!(allowed(
            &Url::parse(&login().replace("cursor.com/", "cursor.com:443/")).unwrap()
        ));
    }

    #[test]
    fn cursor_rejects_unsafe_origins_paths_and_secret_or_ambiguous_parameters() {
        let valid = login();
        for (from, to) in [
            ("https://", "http://"),
            ("www.cursor.com", "www.cursor.com.evil.invalid"),
            ("www.cursor.com", "cursor.com"),
            ("www.cursor.com", "user:secret@www.cursor.com"),
            ("www.cursor.com", "www.cursor.com:444"),
            ("/loginDeepControl", "/login"),
            ("/loginDeepControl", "/login%44eepControl"),
            ("mode=login", "mode=other"),
            (
                "redirectTarget=cli",
                "redirectTarget=https://attacker.invalid",
            ),
            ("challenge=", "accessToken="),
            ("&mode=login", ""),
            ("4ccc", "5ccc"),
            ("8ddd", "7ddd"),
            ("aaaaaaaa", "AAAAAAAA"),
            ("aaaaaaaa", "aaaaaaa"),
        ] {
            assert!(!super::super::allowed(
                &Url::parse(&valid.replace(from, to)).unwrap()
            ));
        }
        for suffix in [
            "&accessToken=synthetic",
            "&uuid=aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee",
            "&mode=login",
            "#",
            "#fragment",
            "&redirectTarget=cli",
        ] {
            assert!(!super::super::allowed(
                &Url::parse(&format!("{valid}{suffix}")).unwrap()
            ));
        }
        for challenge in [
            "x".repeat(42),
            "x".repeat(44),
            "x".repeat(1025),
            "%0A".repeat(43),
        ] {
            assert!(!allowed(
                &Url::parse(&valid.replace(&"x".repeat(43), &challenge)).unwrap()
            ));
        }
    }
}
