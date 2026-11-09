// SPDX-License-Identifier: MPL-2.0
use pulp_core::{id::Id, text::normalize_text};

#[test]
fn brand_identifiers_are_consistent() {
    use pulp_core::brand::*;
    assert_eq!(MANIFEST_FILE, format!("{}-project.json", LINK_SCHEME));
    assert_eq!(TEMP_SUFFIX, format!(".{}.tmp", PROJECT_ARCHIVE_EXT));
    assert_eq!(APP_NAME.to_lowercase(), LINK_SCHEME);
}

#[test]
fn new_ids_are_26_char_ulids_and_unique() {
    let a = Id::new();
    let b = Id::new();
    assert_eq!(a.as_str().len(), 26);
    assert_ne!(a, b);
    assert_eq!(Id::parse(a.as_str()).unwrap(), a);
}

#[test]
fn ids_sort_by_creation_time() {
    let a = Id::new();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = Id::new();
    assert!(a < b);
}

#[test]
fn parse_rejects_non_ulids_and_accepts_lowercase() {
    assert!(Id::parse("7k2m9q").is_err());
    assert!(Id::parse("").is_err());
    let a = Id::new();
    assert_eq!(Id::parse(&a.as_str().to_lowercase()).unwrap(), a);
}

#[test]
fn short_is_last_six_characters() {
    let a = Id::parse("01JC3Z8K7QW2M4N6P8R0T2V4X6").unwrap();
    assert_eq!(a.short(), "T2V4X6");
}

#[test]
fn normalize_converts_crlf_and_cr_to_lf() {
    assert_eq!(normalize_text("a\r\nb\rc\n"), "a\nb\nc\n");
}

#[test]
fn normalize_composes_decomposed_accents() {
    assert_eq!(normalize_text("Ac\u{0327}a\u{0303}o"), "Ação");
}
