// Focused compatibility tests are kept outside the immutable upstream archive.
#[cfg(test)]
mod tests {
    use crate::hpack::{Encoder, Header};
    use bytes::BytesMut;
    use http::header::{HeaderName, HeaderValue};

    fn field(name: Option<&str>, value: &'static str) -> Header<Option<HeaderName>> {
        Header::Field {
            name: name.map(|name| HeaderName::from_bytes(name.as_bytes()).unwrap()),
            value: HeaderValue::from_static(value),
        }
    }

    #[test]
    fn go_equal_length_huffman_stays_literal_and_repeated_cookie_is_indexed() {
        let mut encoder = Encoder::default();
        let mut first = BytesMut::new();
        encoder.encode_go([field(Some("cookie"), "b=2")], &mut first);
        assert_eq!(&first[..], &[0x60, 3, b'b', b'=', b'2']);
        let mut next = BytesMut::new();
        encoder.encode_go([field(Some("cookie"), "b=2")], &mut next);
        assert_eq!(&next[..], &[0xbe]);
    }

    #[test]
    fn go_indexes_each_value_of_a_repeated_header() {
        let mut encoder = Encoder::default();
        encoder.encode_go(
            [field(Some("cookie"), "a=1"), field(None, "b=2")],
            &mut BytesMut::new(),
        );
        let mut next = BytesMut::new();
        encoder.encode_go(
            [field(Some("cookie"), "a=1"), field(None, "b=2")],
            &mut next,
        );
        assert_eq!(&next[..], &[0xbf, 0xbe]);
    }

    #[test]
    fn unmarked_encoder_retains_original_policy() {
        let mut encoder = Encoder::default();
        let mut first = BytesMut::new();
        let mut next = BytesMut::new();
        encoder.encode([field(Some("cookie"), "b=2")], &mut first);
        encoder.encode([field(Some("cookie"), "b=2")], &mut next);
        assert_eq!(first, next);
        assert_ne!(&first[..], &[0x60, 3, b'b', b'=', b'2']);
    }

    #[test]
    fn go_indexes_redacted_values_without_clearing_debug_protection() {
        let mut encoder = Encoder::default();
        let mut header = field(Some("authorization"), "synthetic");
        if let Header::Field { value, .. } = &mut header {
            value.set_sensitive(true);
        }
        let mut encoded = BytesMut::new();
        encoder.encode_go([header.clone()], &mut encoded);
        assert_eq!(encoded[0] & 0xc0, 0x40);
        assert!(!format!("{encoder:?}").contains("synthetic"));
        let mut second = BytesMut::new();
        encoder.encode_go([header], &mut second);
        assert_eq!(&second[..], &[0xbe]);
    }
}
