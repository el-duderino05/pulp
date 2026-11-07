// SPDX-License-Identifier: MPL-2.0
//! The published JSON Schemas, embedded and compiled once.
use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaKind {
    Manifest,
    Library,
    ItemHeader,
}

static MANIFEST: LazyLock<Validator> =
    LazyLock::new(|| compile(include_str!("../schemas/manifest.schema.json")));
static LIBRARY: LazyLock<Validator> =
    LazyLock::new(|| compile(include_str!("../schemas/library.schema.json")));
static ITEM_HEADER: LazyLock<Validator> =
    LazyLock::new(|| compile(include_str!("../schemas/item-header.schema.json")));

fn compile(source: &str) -> Validator {
    let schema: Value = serde_json::from_str(source).expect("an embedded schema is valid JSON");
    jsonschema::draft202012::new(&schema).expect("an embedded schema compiles")
}

/// Checks `value` against the schema for `kind`. Errors are readable strings.
pub fn validate(kind: SchemaKind, value: &Value) -> Result<(), Vec<String>> {
    let validator = match kind {
        SchemaKind::Manifest => &*MANIFEST,
        SchemaKind::Library => &*LIBRARY,
        SchemaKind::ItemHeader => &*ITEM_HEADER,
    };
    let errors: Vec<String> = validator
        .iter_errors(value)
        .map(|e| {
            let at = e.instance_path().to_string();
            if at.is_empty() {
                e.to_string()
            } else {
                format!("{at}: {e}")
            }
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn header() -> Value {
        json!({
            "format": 1, "minReader": 1,
            "id": "01JC3Z8K7QW2M4N6P8R0T2V4X6",
            "type": "document", "title": "Arrival",
            "labels": [], "synopsis": "", "created": "2026-11-06T17:00:00Z"
        })
    }

    fn library() -> Value {
        json!({
            "format": 1, "minReader": 1,
            "author": { "id": "01JC3Z8K7QW2M4N6P8R0T2V4X6", "name": "Gonçalo" },
            "settings": {}
        })
    }

    #[test]
    fn header_without_kind_and_status_is_accepted() {
        assert!(validate(SchemaKind::ItemHeader, &header()).is_ok());
    }

    #[test]
    fn header_with_kind_status_and_unknown_keys_is_accepted() {
        let mut h = header();
        h["kind"] = json!("stage-play");
        h["status"] = json!("Draft");
        h["futureKey"] = json!(1);
        assert!(validate(SchemaKind::ItemHeader, &h).is_ok());
    }

    #[test]
    fn header_without_labels_is_rejected() {
        let mut h = header();
        h.as_object_mut().unwrap().remove("labels");
        let errs = validate(SchemaKind::ItemHeader, &h).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("labels")), "{errs:?}");
    }

    #[test]
    fn header_with_unknown_type_or_kind_is_rejected() {
        let mut h = header();
        h["type"] = json!("whiteboard");
        assert!(validate(SchemaKind::ItemHeader, &h).is_err());
        let mut h = header();
        h["kind"] = json!("opera");
        assert!(validate(SchemaKind::ItemHeader, &h).is_err());
    }

    #[test]
    fn lowercase_ids_are_accepted_and_malformed_ones_are_not() {
        let mut h = header();
        h["id"] = json!("01jc3z8k7qw2m4n6p8r0t2v4x6");
        assert!(validate(SchemaKind::ItemHeader, &h).is_ok());
        h["id"] = json!("8ZZZZZZZZZZZZZZZZZZZZZZZZZ");
        assert!(validate(SchemaKind::ItemHeader, &h).is_err());
        h["id"] = json!("short");
        assert!(validate(SchemaKind::ItemHeader, &h).is_err());
    }

    #[test]
    fn library_accepts_the_spec_example() {
        assert!(validate(SchemaKind::Library, &library()).is_ok());
    }

    #[test]
    fn library_without_author_is_rejected() {
        let mut l = library();
        l.as_object_mut().unwrap().remove("author");
        let errs = validate(SchemaKind::Library, &l).unwrap_err();
        assert!(errs.iter().any(|e| e.contains("author")), "{errs:?}");
    }
}
