// SPDX-License-Identifier: MPL-2.0
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ulid::Ulid;

#[derive(Debug, thiserror::Error)]
#[error("invalid id: {0:?}")]
pub struct IdError(String);

/// A ULID: 26 characters, Crockford base32, stored uppercase.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Id(String);

impl Id {
    // No `Default`: a default value that is a new random id would mislead.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Id {
        Id(Ulid::generate().to_string())
    }

    pub fn parse(s: &str) -> Result<Id, IdError> {
        Ulid::from_string(s)
            .map(|u| Id(u.to_string()))
            .map_err(|_| IdError(s.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The last six characters, for compact display.
    pub fn short(&self) -> &str {
        &self.0[self.0.len() - 6..]
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Id, D::Error> {
        let s = String::deserialize(deserializer)?;
        Id::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_as_a_plain_string() {
        let id = Id::parse("01JC3Z8K7QW2M4N6P8R0T2V4X6").unwrap();
        assert_eq!(
            serde_json::to_string(&id).unwrap(),
            "\"01JC3Z8K7QW2M4N6P8R0T2V4X6\""
        );
    }

    #[test]
    fn deserialises_through_parse() {
        let id: Id = serde_json::from_str("\"01JC3Z8K7QW2M4N6P8R0T2V4X6\"").unwrap();
        assert_eq!(id.as_str(), "01JC3Z8K7QW2M4N6P8R0T2V4X6");
    }

    #[test]
    fn deserialising_lowercase_stores_uppercase() {
        let id: Id = serde_json::from_str("\"01jc3z8k7qw2m4n6p8r0t2v4x6\"").unwrap();
        assert_eq!(id.as_str(), "01JC3Z8K7QW2M4N6P8R0T2V4X6");
    }

    #[test]
    fn deserialising_a_bad_id_fails() {
        assert!(serde_json::from_str::<Id>("\"7k2m9q\"").is_err());
        assert!(serde_json::from_str::<Id>("\"\"").is_err());
        assert!(serde_json::from_str::<Id>("42").is_err());
    }
}
