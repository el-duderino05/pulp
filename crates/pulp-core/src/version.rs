// SPDX-License-Identifier: MPL-2.0

/// The file format version this build writes.
pub const FORMAT_VERSION: u32 = 1;
/// The oldest reader version that can read what this build writes.
pub const READER_VERSION: u32 = 1;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    #[error("this file needs reader version {required}, but only {supported} is supported")]
    TooNew { required: u32, supported: u32 },
}

/// Refuses files whose `minReader` is newer than this build understands.
pub fn check_readable(min_reader: u32) -> Result<(), VersionError> {
    if min_reader > READER_VERSION {
        Err(VersionError::TooNew {
            required: min_reader,
            supported: READER_VERSION,
        })
    } else {
        Ok(())
    }
}
