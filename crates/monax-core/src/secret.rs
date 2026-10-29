//! [`Secret`]: a password or token that never shows in Debug, Display,
//! errors or logs.
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;

/// A secret string. JSON: a plain string (`writeOnly` in the schema).
/// Debug and Display print `***`; read it only through [`Secret::expose`].
/// A malformed value fails with "invalid password value", never echoing it.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The secret itself: pass it on, never format it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Secret {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)
            .map(Self)
            .map_err(|_| D::Error::custom("invalid password value"))
    }
}

impl JsonSchema for Secret {
    fn schema_name() -> Cow<'static, str> {
        "Secret".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "string", "writeOnly": true })
    }
}

/// `deserialize_with` for a list of optional secrets: a malformed list
/// fails with "invalid passwords value", never echoing what it held.
pub(crate) fn list<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Option<Secret>>, D::Error> {
    Vec::deserialize(d).map_err(|_| {
        D::Error::custom("invalid passwords value: expected a list of strings or nulls")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_everywhere_but_expose_and_json() {
        let s = Secret::from("x-Sesame");
        assert_eq!(
            (format!("{s:?}"), s.to_string()),
            ("***".into(), "***".into())
        );
        assert_eq!(s.expose(), "x-Sesame");
        assert_eq!(serde_json::to_string(&s).unwrap(), "\"x-Sesame\"");
        let e = serde_json::from_str::<Secret>("[\"x-Sesame\"]").unwrap_err();
        assert!(e.to_string().starts_with("invalid password value"), "{e}");
    }
}
