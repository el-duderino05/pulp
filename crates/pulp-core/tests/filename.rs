// SPDX-License-Identifier: MPL-2.0
use pulp_core::filename::{slugify, unique_name};

#[test]
fn latin_titles_lose_accents_and_punctuation() {
    assert_eq!(slugify("Capítulo 1 — A Chegada"), "capitulo-1-a-chegada");
    assert_eq!(slugify("Ação & Reação"), "acao-reacao");
    assert_eq!(slugify("Chapter 1: The Storm!"), "chapter-1-the-storm");
    assert_eq!(slugify("Straße"), "strasse");
    assert_eq!(slugify("Œuvre"), "oeuvre");
    assert_eq!(slugify("Łódź"), "lodz");
}

#[test]
fn other_scripts_keep_their_letters() {
    assert_eq!(slugify("Война и мир"), "война-и-мир");
    assert_eq!(slugify("東京物語"), "東京物語");
    assert_eq!(slugify("नमस्ते दुनिया"), "नमस्ते-दुनिया");
    assert_eq!(slugify("Café Москва"), "cafe-москва");
}

#[test]
fn unusable_titles_become_untitled() {
    assert_eq!(slugify(""), "untitled");
    assert_eq!(slugify("   "), "untitled");
    assert_eq!(slugify(":?*<>|"), "untitled");
    assert_eq!(slugify("🙂🙂"), "untitled");
}

#[test]
fn windows_reserved_names_are_avoided() {
    assert_eq!(slugify("CON"), "con-file");
    assert_eq!(slugify("aux"), "aux-file");
    assert_eq!(slugify("COM1"), "com1-file");
}

#[test]
fn long_titles_are_capped_without_trailing_dash() {
    let s = slugify(&"word ".repeat(40));
    assert!(s.chars().count() <= 60);
    assert!(!s.ends_with('-'));
}

#[test]
fn unique_name_adds_suffix_ignoring_case() {
    assert_eq!(unique_name("ana", "md", &[]), "ana.md");
    assert_eq!(unique_name("ana", "md", &["Ana.md".into()]), "ana-2.md");
    assert_eq!(
        unique_name("ana", "md", &["ana.md".into(), "ANA-2.md".into()]),
        "ana-3.md"
    );
}
