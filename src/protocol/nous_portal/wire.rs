//! Reject ambiguous duplicate fields before interpreting OAuth or JWT objects.
use super::OAuthError;
use serde::Deserialize;
use serde_json::Value;

pub(super) fn object(bytes: &[u8]) -> Result<Value, OAuthError> {
    struct Object(Value);
    impl<'de> Deserialize<'de> for Object {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            deserialize_object(deserializer).map(Object)
        }
    }
    serde_json::from_slice::<Object>(bytes)
        .map(|v| v.0)
        .map_err(|_| OAuthError::InvalidResponse)
}
pub(super) fn deserialize_object<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Value, D::Error> {
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = Value;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a unique-field Nous object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, mut map: M) -> Result<Value, M::Error> {
            let mut fields = serde_json::Map::new();
            while let Some((key, value)) = map.next_entry::<String, Value>()? {
                if fields.insert(key, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate Nous field"));
                }
            }
            Ok(Value::Object(fields))
        }
    }
    deserializer.deserialize_map(Visitor)
}
