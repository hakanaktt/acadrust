//! JSON values for glTF import.
//!
//! Parsing is `serde_json`. [`JsonValue`] keeps the small lookup API the
//! importer uses.

use crate::error::{DxfError, Result};

/// A JSON value.
#[derive(Debug, Clone)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Parse a JSON string into a [`JsonValue`].
    pub fn parse(input: &str) -> Result<JsonValue> {
        let value = serde_json::from_str(input)
            .map_err(|error| DxfError::ImportError(format!("JSON parse error: {error}")))?;
        Ok(from_serde(value))
    }

    /// Get a value by object key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Get a value by array index.
    pub fn at(&self, index: usize) -> Option<&JsonValue> {
        match self {
            JsonValue::Array(arr) => arr.get(index),
            _ => None,
        }
    }

    /// Get as string slice.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Get as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as usize.
    pub fn as_usize(&self) -> Option<usize> {
        match self {
            JsonValue::Number(n) => {
                let v = *n;
                if v >= 0.0 && v == (v as usize as f64) {
                    Some(v as usize)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Get as array.
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Get as object pairs.
    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// Get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

fn from_serde(value: serde_json::Value) -> JsonValue {
    match value {
        serde_json::Value::Null => JsonValue::Null,
        serde_json::Value::Bool(value) => JsonValue::Bool(value),
        serde_json::Value::Number(value) => JsonValue::Number(value.as_f64().unwrap_or(f64::NAN)),
        serde_json::Value::String(value) => JsonValue::Str(value),
        serde_json::Value::Array(values) => {
            JsonValue::Array(values.into_iter().map(from_serde).collect())
        }
        serde_json::Value::Object(values) => JsonValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, from_serde(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_number() {
        let v = JsonValue::parse("42").unwrap();
        assert_eq!(v.as_f64(), Some(42.0));
    }

    /// A 17-significant-digit decimal parses bit-exact. The JSON float
    /// round-trip feature matters: without it serde_json's fast parser is
    /// up to 1 ulp off such numbers, which shifted coordinates through a
    /// JSON manifest round-trip (survey point 4039.2756306090996 came
    /// back 4039.275630609099).
    #[test]
    fn a_long_decimal_parses_bit_exact() {
        let v = JsonValue::parse(r#"{"x": 4039.2756306090996}"#).unwrap();
        let expected: f64 = "4039.2756306090996".parse().unwrap();
        assert_eq!(v.get("x").unwrap().as_f64().unwrap().to_bits(), expected.to_bits());
    }

    #[test]
    fn test_parse_string() {
        let v = JsonValue::parse(r#""hello""#).unwrap();
        assert_eq!(v.as_str(), Some("hello"));
    }

    #[test]
    fn test_parse_object() {
        let v = JsonValue::parse(r#"{"a": 1, "b": "two"}"#).unwrap();
        assert_eq!(v.get("a").unwrap().as_f64(), Some(1.0));
        assert_eq!(v.get("b").unwrap().as_str(), Some("two"));
    }

    #[test]
    fn test_parse_array() {
        let v = JsonValue::parse("[1, 2, 3]").unwrap();
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_f64(), Some(1.0));
    }

    #[test]
    fn test_parse_nested() {
        let v = JsonValue::parse(r#"{"meshes": [{"name": "m", "primitives": []}]}"#).unwrap();
        let meshes = v.get("meshes").unwrap().as_array().unwrap();
        assert_eq!(meshes[0].get("name").unwrap().as_str(), Some("m"));
    }

    #[test]
    fn test_parse_recursion_limit() {
        let mut s = String::new();
        for _ in 0..200 {
            s.push('[');
        }
        for _ in 0..200 {
            s.push(']');
        }
        let res = JsonValue::parse(&s);
        assert!(res.is_err());
        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("recursion limit exceeded"),
            "Unexpected error: {}",
            err
        );
    }
}
