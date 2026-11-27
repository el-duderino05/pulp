// SPDX-License-Identifier: MPL-2.0
use pulp_core::library::LibraryError;
use pulp_core::project::ProjectError;
use pulp_core::storage::{ErrorCause, StorageError};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ErrorCode {
    NotALibrary,
    TooNew,
    PermissionDenied,
    Locked,
    DiskFull,
    NotFound,
    Exists,
    Invalid,
    LibraryExists,
    NotAProject,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: ErrorCode,
    pub detail: String,
}

impl AppError {
    pub fn other(detail: impl Into<String>) -> AppError {
        AppError {
            code: ErrorCode::Other,
            detail: detail.into(),
        }
    }
}

fn cause_code(e: &StorageError) -> ErrorCode {
    match e.cause() {
        ErrorCause::NotFound => ErrorCode::NotFound,
        ErrorCause::Locked => ErrorCode::Locked,
        ErrorCause::DiskFull => ErrorCode::DiskFull,
        ErrorCause::PermissionDenied => ErrorCode::PermissionDenied,
        ErrorCause::Io => ErrorCode::Other,
    }
}

impl From<StorageError> for AppError {
    fn from(e: StorageError) -> AppError {
        AppError {
            code: cause_code(&e),
            detail: e.to_string(),
        }
    }
}

impl From<LibraryError> for AppError {
    fn from(e: LibraryError) -> AppError {
        let code = match &e {
            LibraryError::NotALibrary(_) => ErrorCode::NotALibrary,
            LibraryError::Version(_) => ErrorCode::TooNew,
            LibraryError::Storage(s) => cause_code(s),
            LibraryError::Invalid(_) => ErrorCode::Invalid,
        };
        AppError {
            code,
            detail: e.to_string(),
        }
    }
}

impl From<ProjectError> for AppError {
    fn from(e: ProjectError) -> AppError {
        let code = match &e {
            ProjectError::Version(_) => ErrorCode::TooNew,
            ProjectError::Storage(s) => cause_code(s),
            ProjectError::Exists(_) => ErrorCode::Exists,
            _ => ErrorCode::Other,
        };
        AppError {
            code,
            detail: e.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulp_core::storage::RelPath;
    use pulp_core::version::VersionError;

    #[test]
    fn storage_causes_map_to_codes() {
        let p = RelPath::root();
        let s = |e: StorageError| AppError::from(LibraryError::Storage(e)).code;
        assert_eq!(
            s(StorageError::NotFound(p.clone(), String::new())),
            ErrorCode::NotFound
        );
        assert_eq!(
            s(StorageError::Locked(p.clone(), String::new())),
            ErrorCode::Locked
        );
        assert_eq!(
            s(StorageError::DiskFull(p.clone(), String::new())),
            ErrorCode::DiskFull
        );
        assert_eq!(
            s(StorageError::PermissionDenied(p.clone(), String::new())),
            ErrorCode::PermissionDenied
        );
        assert_eq!(
            s(StorageError::Io(p.clone(), String::new())),
            ErrorCode::Other
        );
        let too_new = || VersionError::TooNew {
            required: 9,
            supported: 1,
        };
        assert_eq!(
            AppError::from(LibraryError::Version(too_new())).code,
            ErrorCode::TooNew
        );
        assert_eq!(
            AppError::from(ProjectError::Version(too_new())).code,
            ErrorCode::TooNew
        );
        assert_eq!(
            AppError::from(ProjectError::Exists(p)).code,
            ErrorCode::Exists
        );
        assert_eq!(
            AppError::from(LibraryError::Invalid("x".into())).code,
            ErrorCode::Invalid
        );
        assert_eq!(
            AppError::from(LibraryError::NotALibrary("x".into())).code,
            ErrorCode::NotALibrary
        );
        assert_eq!(
            AppError::from(ProjectError::AlreadyAProject).code,
            ErrorCode::Other
        );
    }
}
