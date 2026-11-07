// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, version::*};

const SAMPLE: &str = "---\nformat: 1\nminReader: 1\nid: 01JC3Z8K7QW2M4N6P8R0T2V4X6\ntype: document\nkind: prose\ntitle: Arrival\nstatus: Draft\nlabels: []\nsynopsis: ''\ncreated: 2026-11-06T17:00:00Z\nfutureKey: keep me\nanotherKey:\n- 1\n- 2\n---\nBody text.\n";

#[test]
fn parse_reads_known_fields_and_body() {
    let f = parse_markdown_item(SAMPLE).unwrap();
    assert_eq!(f.header.title, "Arrival");
    assert_eq!(f.header.item_type, ItemType::Document);
    assert_eq!(f.header.kind, Some(DocKind::Prose));
    assert_eq!(f.header.status.as_deref(), Some("Draft"));
    assert_eq!(f.body, "Body text.\n");
}

#[test]
fn render_keeps_unknown_keys_in_order_after_known_keys() {
    let out = render_markdown_item(&parse_markdown_item(SAMPLE).unwrap());
    let pos = |k: &str| out.find(k).unwrap();
    assert!(pos("format:") < pos("minReader:"));
    assert!(pos("minReader:") < pos("id:"));
    assert!(pos("title:") < pos("created:"));
    assert!(pos("created:") < pos("futureKey:"));
    assert!(pos("futureKey:") < pos("anotherKey:"));
    assert!(out.contains("keep me"));
    assert!(out.ends_with("---\nBody text.\n"));
}

#[test]
fn render_parse_round_trip_is_stable() {
    let once = render_markdown_item(&parse_markdown_item(SAMPLE).unwrap());
    let twice = render_markdown_item(&parse_markdown_item(&once).unwrap());
    assert_eq!(once, twice);
}

#[test]
fn missing_or_broken_headers_are_reported() {
    assert!(matches!(
        parse_markdown_item("Just text"),
        Err(HeaderError::Missing)
    ));
    assert!(matches!(
        parse_markdown_item("---\ntitle: [unclosed\n---\n"),
        Err(HeaderError::Invalid(_))
    ));
}

#[test]
fn new_document_header_has_current_versions() {
    let h = ItemHeader::new_document("Arrival", DocKind::Prose);
    assert_eq!(h.format, FORMAT_VERSION);
    assert_eq!(h.min_reader, READER_VERSION);
    assert!(h.created.ends_with('Z'));
}

#[test]
fn newer_reader_versions_are_refused() {
    assert!(check_readable(1).is_ok());
    assert!(matches!(
        check_readable(2),
        Err(VersionError::TooNew {
            required: 2,
            supported: 1
        })
    ));
}
