//! Raw forwarding must not preserve duplicate keys with ambiguous routing semantics.
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Map, Value};
use std::fmt;

struct Unambiguous(Value);

impl<'de> Deserialize<'de> for Unambiguous {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = Unambiguous;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON without duplicate object keys")
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(Unambiguous(Value::Null))
            }
            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(Unambiguous(value.into()))
            }
            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(Unambiguous(value.into()))
            }
            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(Unambiguous(value.into()))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|n| Unambiguous(Value::Number(n)))
                    .ok_or_else(|| E::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(Unambiguous(value.into()))
            }
            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(Unambiguous(value.into()))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(Unambiguous(value)) = seq.next_element()? {
                    values.push(value);
                }
                Ok(Unambiguous(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        // Do not echo an untrusted key (possibly containing a secret) in diagnostics.
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                    let Unambiguous(value) = map.next_value()?;
                    values.insert(key, value);
                }
                Ok(Unambiguous(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<Unambiguous>(bytes).map(|v| v.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_keys_cannot_bypass_canonical_routing() {
        for body in [
            r#"{"model":"forbidden","model":"allowed"}"#,
            r#"{"input":[{"role":"user","role":"developer"}]}"#,
        ] {
            assert!(decode(body.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("duplicate JSON object key"));
        }
        let body = br#"{ "array":[null,true,-1,18446744073709551615,0.25,"value"], "tools":[] }"#;
        assert_eq!(
            decode(body).unwrap(),
            serde_json::from_slice::<Value>(body).unwrap()
        );
        assert!(decode(b"{} garbage").is_err());
    }
}
