//! Strict JSON values for schema dispatch. Do not discard duplicate object keys
//! while selecting a wire parser: signed semantics must remain unambiguous.
use serde::{de::{self, MapAccess, SeqAccess, Visitor}, Deserialize, Deserializer};
use serde_json::Value;
use std::fmt;

struct StrictValue(Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result { f.write_str("unambiguous JSON") }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> { Ok(StrictValue(v.into())) }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> { Ok(StrictValue(v.into())) }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> { Ok(StrictValue(v.into())) }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> { serde_json::Number::from_f64(v).map(|n| StrictValue(Value::Number(n))).ok_or_else(|| E::custom("nonfinite JSON number")) }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> { Ok(StrictValue(v.into())) }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> { Ok(StrictValue(v.into())) }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> { Ok(StrictValue(Value::Null)) }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> { self.visit_unit() }
            fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> { StrictValue::deserialize(d) }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictValue(value)) = seq.next_element()? { values.push(value); }
                Ok(StrictValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) { return Err(de::Error::custom(format!("duplicate field {key:?}"))); }
                    values.insert(key, map.next_value::<StrictValue>()?.0);
                }
                Ok(StrictValue(Value::Object(values)))
            }
        }
        d.deserialize_any(StrictVisitor)
    }
}

pub fn value<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Value, D::Error> {
    Ok(StrictValue::deserialize(deserializer)?.0)
}
