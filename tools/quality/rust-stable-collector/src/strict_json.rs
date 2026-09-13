//! The authenticated JSON domain cannot silently discard duplicate keys.
use anyhow::Result;
use serde::{
    de::{self, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::{Map, Value};
use std::fmt;

struct Strict<const FLOATS: bool>(Value);
impl<'de, const FLOATS: bool> Deserialize<'de> for Strict<FLOATS> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        struct Json<const FLOATS: bool>;
        impl<'de, const FLOATS: bool> Visitor<'de> for Json<FLOATS> {
            type Value = Strict<FLOATS>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str(if FLOATS {
                    "JSON without duplicate keys"
                } else {
                    "JSON without duplicate keys or floating point values"
                })
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Strict<FLOATS>, E> {
                Ok(Strict(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Strict<FLOATS>, E> {
                Ok(Strict(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Strict<FLOATS>, E> {
                Ok(Strict(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<Strict<FLOATS>, E> {
                if !FLOATS {
                    return Err(de::Error::custom("floating point values forbidden"));
                }
                serde_json::Number::from_f64(v)
                    .map(|n| Strict(Value::Number(n)))
                    .ok_or_else(|| de::Error::custom("non-finite JSON number"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Strict<FLOATS>, E> {
                Ok(Strict(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Strict<FLOATS>, E> {
                Ok(Strict(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Strict<FLOATS>, A::Error> {
                let mut values = Vec::new();
                while let Some(Strict(v)) = seq.next_element::<Strict<FLOATS>>()? {
                    values.push(v);
                }
                Ok(Strict(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Strict<FLOATS>, A::Error> {
                let mut values = Map::new();
                while let Some((key, Strict(value))) = map.next_entry::<String, Strict<FLOATS>>()? {
                    if values.insert(key.clone(), value).is_some() {
                        return Err(de::Error::custom(format!("duplicate key: {key}")));
                    }
                }
                Ok(Strict(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(Json::<FLOATS>)
    }
}
pub fn parse(bytes: &[u8]) -> Result<Value> {
    Ok(serde_json::from_slice::<Strict<false>>(bytes)?.0)
}

/// LLVM exports contain floating point summary percentages. They are never
/// imported into the integer-only authenticated Core metric domain.
pub fn parse_coverage(bytes: &[u8]) -> Result<Value> {
    Ok(serde_json::from_slice::<Strict<true>>(bytes)?.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_nested_fields_and_noninteger_metrics_fail() {
        for raw in [
            r#"{"a":{"x":1,"x":2}}"#,
            r#"{"metric":1.5}"#,
            r#"{"x":1} {}"#,
        ] {
            assert!(parse(raw.as_bytes()).is_err());
        }
        assert!(parse_coverage(br#"{"x":1,"x":2}"#).is_err());
        assert_eq!(
            parse_coverage(br#"{"percent":1.5}"#).unwrap()["percent"],
            1.5
        );
        assert_eq!(parse(br#"{"x":[1,true,null,"text"]}"#).unwrap()["x"][0], 1);
    }
}
