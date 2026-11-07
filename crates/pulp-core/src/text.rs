// SPDX-License-Identifier: MPL-2.0
use unicode_normalization::UnicodeNormalization;

/// Normalises text to LF line endings and Unicode NFC.
pub fn normalize_text(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect()
}
