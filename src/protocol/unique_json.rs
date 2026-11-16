//! Reject ambiguous external JSON before it can select credentials, issuers or model protocols.
use serde::de::{Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Number, Value};

struct Unique(Value);
struct Json;
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Json)
    }
}
impl<'de> Visitor<'de> for Json {
    type Value = Unique;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON without duplicate object members")
    }
    fn visit_bool<E>(self, value: bool) -> Result<Unique, E> {
        Ok(Unique(Value::Bool(value)))
    }
    fn visit_i64<E>(self, value: i64) -> Result<Unique, E> {
        Ok(Unique(Value::Number(value.into())))
    }
    fn visit_u64<E>(self, value: u64) -> Result<Unique, E> {
        Ok(Unique(Value::Number(value.into())))
    }
    fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Unique, E> {
        Number::from_f64(value)
            .map(|n| Unique(Value::Number(n)))
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }
    fn visit_str<E>(self, value: &str) -> Result<Unique, E> {
        Ok(Unique(Value::String(value.into())))
    }
    fn visit_string<E>(self, value: String) -> Result<Unique, E> {
        Ok(Unique(Value::String(value)))
    }
    fn visit_none<E>(self) -> Result<Unique, E> {
        Ok(Unique(Value::Null))
    }
    fn visit_unit<E>(self) -> Result<Unique, E> {
        Ok(Unique(Value::Null))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Unique, A::Error> {
        let mut values = Vec::new();
        while let Some(Unique(value)) = seq.next_element()? {
            values.push(value);
        }
        Ok(Unique(Value::Array(values)))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Unique, A::Error> {
        let mut values = Map::new();
        while let Some((key, Unique(value))) = map.next_entry::<String, Unique>()? {
            if values.insert(key, value).is_some() {
                return Err(serde::de::Error::custom("duplicate JSON member"));
            }
        }
        Ok(Unique(Value::Object(values)))
    }
}

pub(crate) fn parse(bytes: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<Unique>(bytes).map(|value| value.0)
}
