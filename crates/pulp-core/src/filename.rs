// SPDX-License-Identifier: MPL-2.0
use std::collections::HashSet;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Longest file-name stem, in characters, including any `-N` suffix.
pub const MAX_SLUG_CHARS: usize = 60;

const FALLBACK: &str = "untitled";

/// Letters that do not decompose under NFD, mapped to their plain spelling.
fn special_latin(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{DF}' | '\u{1E9E}' => "ss",
        '\u{E6}' | '\u{C6}' => "ae",
        '\u{153}' | '\u{152}' => "oe",
        '\u{F8}' | '\u{D8}' => "o",
        '\u{142}' | '\u{141}' => "l",
        '\u{111}' | '\u{110}' => "d",
        '\u{F0}' | '\u{D0}' => "d",
        '\u{FE}' | '\u{DE}' => "th",
        _ => return None,
    })
}

fn is_variation_selector(c: char) -> bool {
    matches!(c, '\u{FE00}'..='\u{FE0F}' | '\u{E0100}'..='\u{E01EF}')
}

fn is_latin(c: char) -> bool {
    matches!(c, '\u{41}'..='\u{24F}')
}

fn is_reserved(name: &str) -> bool {
    if matches!(name, "con" | "prn" | "aux" | "nul") {
        return true;
    }
    let mut chars = name.chars();
    let (a, b, c, d) = (chars.next(), chars.next(), chars.next(), chars.next());
    if chars.next().is_some() {
        return false;
    }
    let prefix: String = [a, b, c].into_iter().flatten().collect();
    (prefix == "com" || prefix == "lpt")
        && matches!(d, Some('1'..='9' | '\u{B9}' | '\u{B2}' | '\u{B3}'))
}

/// Cuts `s` to at most `max` characters without splitting a letter from its
/// combining marks, then trims trailing dashes.
fn cap(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut end = chars.len().min(max);
    if end < chars.len() {
        while end > 0 && is_combining_mark(chars[end]) {
            end -= 1;
        }
    }
    chars[..end]
        .iter()
        .collect::<String>()
        .trim_end_matches('-')
        .to_string()
}

/// Turns a title into a safe file-name stem.
pub fn slugify(title: &str) -> String {
    let mapped: String = title
        .chars()
        .flat_map(|c| match special_latin(c) {
            Some(s) => s.chars().collect::<Vec<_>>(),
            None => vec![c],
        })
        .collect();

    let mut out = String::new();
    let mut pending_dash = false;
    // The most recent non-mark character, and whether it was kept.
    let mut base: Option<(char, bool)> = None;
    for c in mapped.nfd() {
        if is_combining_mark(c) {
            let keep = !is_variation_selector(c)
                && matches!(base, Some((b, true)) if b.is_alphabetic() && !is_latin(b));
            if keep {
                out.push(c);
            }
            continue;
        }
        if c.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.extend(c.to_lowercase());
            base = Some((c, true));
        } else {
            pending_dash = true;
            base = Some((c, false));
        }
    }

    let composed: String = out.nfc().collect();
    let slug = cap(composed.trim_matches(['-', '.']), MAX_SLUG_CHARS);
    if slug.is_empty() {
        FALLBACK.to_string()
    } else if is_reserved(&slug) {
        format!("{slug}-file")
    } else {
        slug
    }
}

fn fold(name: &str) -> String {
    name.nfc().collect::<String>().to_uppercase().to_lowercase()
}

/// Returns `slug.ext`, or `slug-2.ext`, `slug-3.ext`... when the name is
/// already in `existing` (compared case-insensitively after NFC). The part
/// before the extension never exceeds `MAX_SLUG_CHARS`. An empty `ext` gives
/// a name with no dot.
pub fn unique_name(slug: &str, ext: &str, existing: &[String]) -> String {
    let taken: HashSet<String> = existing.iter().map(|n| fold(n)).collect();
    let mut n = 1u32;
    loop {
        let suffix = if n == 1 {
            String::new()
        } else {
            format!("-{n}")
        };
        let room = MAX_SLUG_CHARS - suffix.chars().count();
        let stem = format!("{}{suffix}", cap(slug, room));
        let name = if ext.is_empty() {
            stem
        } else {
            format!("{stem}.{ext}")
        };
        if !taken.contains(&fold(&name)) {
            return name;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variation_selectors_do_not_leave_invisible_names() {
        assert_eq!(slugify("❤️"), "untitled");
        assert_eq!(slugify("1️⃣"), "1");
    }

    #[test]
    fn stacked_marks_follow_their_base() {
        assert_eq!(slugify("Việt Nam"), "viet-nam");
    }

    #[test]
    fn a_lone_combining_mark_is_untitled() {
        assert_eq!(slugify("\u{301}"), "untitled");
        assert_eq!(slugify("\u{301}\u{302}"), "untitled");
    }

    #[test]
    fn superscript_com_and_lpt_are_reserved() {
        assert_eq!(slugify("COM¹"), "com¹-file");
        assert_eq!(slugify("lpt²"), "lpt²-file");
        assert_eq!(slugify("com³"), "com³-file");
    }

    #[test]
    fn suffix_fits_inside_the_cap_without_double_dash() {
        let slug = format!("{}-bb", "a".repeat(57));
        assert_eq!(slug.chars().count(), 60);
        let existing = vec![format!("{slug}.md")];
        let name = unique_name(&slug, "md", &existing);
        let stem = name.strip_suffix(".md").unwrap();
        assert!(stem.chars().count() <= MAX_SLUG_CHARS);
        assert!(!name.contains("--"));
        assert_eq!(stem, format!("{}-2", "a".repeat(57)));

        let full = "a".repeat(60);
        let name = unique_name(&full, "md", &[format!("{full}.md")]);
        assert_eq!(name, format!("{}-2.md", "a".repeat(58)));
    }

    #[test]
    fn final_and_plain_sigma_compare_equal() {
        assert_eq!(
            unique_name("λογος", "md", &["λογοσ.md".into()]),
            "λογος-2.md"
        );
    }

    #[test]
    fn empty_extension_has_no_dot() {
        assert_eq!(unique_name("ana", "", &[]), "ana");
        assert_eq!(unique_name("ana", "", &["ANA".into()]), "ana-2");
    }
}
