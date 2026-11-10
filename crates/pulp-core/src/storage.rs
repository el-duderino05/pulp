// SPDX-License-Identifier: MPL-2.0
//! Storage layer: relative paths, atomic save, retries and error classes.

use crate::brand::TEMP_SUFFIX;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub const SAVE_RETRIES: u32 = 5;
pub const RETRY_DELAY: Duration = Duration::from_millis(200);
/// A leftover temp file is only cleared once it is at least this old.
pub const STALE_TEMP_AGE: Duration = Duration::from_secs(60);

/// A path relative to the storage root, always `/`-separated. The empty path
/// is the root itself.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RelPath(String);

impl RelPath {
    /// Validates and normalises `s`: `\` becomes `/`; empty, absolute, drive,
    /// stream (`:`), NUL, empty/`.`/`..` segments are all rejected.
    pub fn new(s: &str) -> Result<RelPath, StorageError> {
        let norm = s.replace('\\', "/");
        let bad = |why: &str| {
            StorageError::Io(
                RelPath::root(),
                format!("invalid relative path {s:?}: {why}"),
            )
        };
        if norm.is_empty() {
            return Err(bad("empty"));
        }
        if norm.starts_with('/') {
            return Err(bad("absolute"));
        }
        if norm.contains(':') || norm.contains('\0') {
            return Err(bad("contains ':' or NUL"));
        }
        for seg in norm.split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                return Err(bad("empty, '.' or '..' segment"));
            }
        }
        Ok(RelPath(norm))
    }

    pub fn root() -> RelPath {
        RelPath(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Appends `name`, converting `\` to `/` and dropping empty, `.` and `..`
    /// segments, so the result can never leave `self`.
    pub fn join(&self, name: &str) -> RelPath {
        let mut out = self.0.clone();
        for seg in name.replace('\\', "/").split('/') {
            if seg.is_empty() || seg == "." || seg == ".." {
                continue;
            }
            if !out.is_empty() {
                out.push('/');
            }
            out.push_str(seg);
        }
        RelPath(out)
    }

    pub fn parent(&self) -> Option<RelPath> {
        if self.0.is_empty() {
            return None;
        }
        Some(match self.0.rfind('/') {
            Some(i) => RelPath(self.0[..i].to_string()),
            None => RelPath::root(),
        })
    }

    pub fn file_name(&self) -> &str {
        match self.0.rfind('/') {
            Some(i) => &self.0[i + 1..],
            None => &self.0,
        }
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for RelPath {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RelPath {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<RelPath, D::Error> {
        let s = String::deserialize(d)?;
        RelPath::new(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub modified: SystemTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCause {
    NotFound,
    Locked,
    DiskFull,
    PermissionDenied,
    Io,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("not found: {0}: {1}")]
    NotFound(RelPath, String),
    #[error("locked by another program: {0}: {1}")]
    Locked(RelPath, String),
    #[error("disk full: {0}: {1}")]
    DiskFull(RelPath, String),
    #[error("permission denied: {0}: {1}")]
    PermissionDenied(RelPath, String),
    #[error("i/o error: {0}: {1}")]
    Io(RelPath, String),
}

impl StorageError {
    pub fn cause(&self) -> ErrorCause {
        match self {
            StorageError::NotFound(..) => ErrorCause::NotFound,
            StorageError::Locked(..) => ErrorCause::Locked,
            StorageError::DiskFull(..) => ErrorCause::DiskFull,
            StorageError::PermissionDenied(..) => ErrorCause::PermissionDenied,
            StorageError::Io(..) => ErrorCause::Io,
        }
    }

    fn new(cause: ErrorCause, p: &RelPath, msg: String) -> StorageError {
        let p = p.clone();
        match cause {
            ErrorCause::NotFound => StorageError::NotFound(p, msg),
            ErrorCause::Locked => StorageError::Locked(p, msg),
            ErrorCause::DiskFull => StorageError::DiskFull(p, msg),
            ErrorCause::PermissionDenied => StorageError::PermissionDenied(p, msg),
            ErrorCause::Io => StorageError::Io(p, msg),
        }
    }
}

pub trait Storage {
    fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError>;
    /// Precondition: one writer at a time per path; the app guarantees this by
    /// serialising calls per project. A leftover temp file is cleared only
    /// when older than `STALE_TEMP_AGE`, so a concurrent writer's live temp
    /// file is never mistaken for a crash leftover and deleted.
    fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError>;
    fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError>;
    fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError>;
    fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), StorageError>;
    fn remove_file(&self, p: &RelPath) -> Result<(), StorageError>;
    /// Removes a folder only if it is empty; a folder with anything in it is
    /// left alone and an error is returned.
    fn remove_dir(&self, p: &RelPath) -> Result<(), StorageError>;
    fn exists(&self, p: &RelPath) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    Unix,
}

pub fn classify_os_error(code: i32, os: Os) -> ErrorCause {
    match os {
        Os::Windows => match code {
            32 | 33 => ErrorCause::Locked,
            112 | 39 => ErrorCause::DiskFull,
            5 => ErrorCause::PermissionDenied,
            2 | 3 => ErrorCause::NotFound,
            _ => ErrorCause::Io,
        },
        Os::Unix => match code {
            28 => ErrorCause::DiskFull,
            1 | 13 => ErrorCause::PermissionDenied,
            11 | 26 => ErrorCause::Locked,
            2 => ErrorCause::NotFound,
            _ => ErrorCause::Io,
        },
    }
}

/// `~$<file_name>` + the brand's temp suffix.
pub fn temp_name_for(file_name: &str) -> String {
    format!("~${file_name}{TEMP_SUFFIX}")
}

pub fn is_temp_name(name: &str) -> bool {
    name.starts_with("~$") && name.ends_with(TEMP_SUFFIX)
}

/// Runs `op` up to `attempts` times in total (at least once), sleeping
/// `delay` between tries. Only `Locked` and `Io` errors are retried, and
/// there is no sleep after the final failure.
pub fn with_retry<T>(
    attempts: u32,
    delay: Duration,
    mut op: impl FnMut() -> Result<T, StorageError>,
) -> Result<T, StorageError> {
    let attempts = attempts.max(1);
    let mut tried = 0;
    loop {
        tried += 1;
        match op() {
            Ok(v) => return Ok(v),
            Err(e) => {
                let retryable = matches!(e.cause(), ErrorCause::Locked | ErrorCause::Io);
                if !retryable || tried >= attempts {
                    return Err(e);
                }
                std::thread::sleep(delay);
            }
        }
    }
}

/// The OS this build runs on, for error classification.
const CURRENT_OS: Os = if cfg!(windows) { Os::Windows } else { Os::Unix };

/// Whether a failed rename is retried. Only Windows code 5 ("access
/// denied"), which antivirus and search indexers cause for a moment while
/// they hold a file open. Every other error behaves as before.
pub fn rename_is_retried(code: Option<i32>, os: Os) -> bool {
    os == Os::Windows && code == Some(5)
}

fn map_io_as(p: &RelPath, e: &io::Error, os: Os) -> StorageError {
    let cause = match e.raw_os_error() {
        Some(code) => classify_os_error(code, os),
        None => match e.kind() {
            io::ErrorKind::NotFound => ErrorCause::NotFound,
            io::ErrorKind::PermissionDenied => ErrorCause::PermissionDenied,
            _ => ErrorCause::Io,
        },
    };
    StorageError::new(cause, p, e.to_string())
}

/// Runs the rename `op` up to `attempts` times in total, sleeping `delay`
/// between tries, but only while it fails in a way `rename_is_retried`
/// accepts. The final error is classified as usual, so a lasting code 5
/// is still reported as `PermissionDenied`.
pub(crate) fn rename_with_retry(
    p: &RelPath,
    os: Os,
    attempts: u32,
    delay: Duration,
    mut op: impl FnMut() -> io::Result<()>,
) -> Result<(), StorageError> {
    let attempts = attempts.max(1);
    let mut tried = 0;
    loop {
        tried += 1;
        match op() {
            Ok(()) => return Ok(()),
            Err(e) => {
                if !rename_is_retried(e.raw_os_error(), os) || tried >= attempts {
                    return Err(map_io_as(p, &e, os));
                }
                std::thread::sleep(delay);
            }
        }
    }
}

pub struct FsStorage {
    root: PathBuf,
}

impl FsStorage {
    pub fn new(root: impl Into<PathBuf>) -> FsStorage {
        FsStorage { root: root.into() }
    }

    fn resolve(&self, p: &RelPath) -> Result<PathBuf, StorageError> {
        if cfg!(windows) && p.as_str().contains(':') {
            return Err(StorageError::Io(
                p.clone(),
                "path contains ':', which is not allowed here".into(),
            ));
        }
        let mut out = self.root.clone();
        for seg in p.as_str().split('/').filter(|s| !s.is_empty()) {
            out.push(seg);
        }
        Ok(out)
    }

    fn map_io(p: &RelPath, e: &io::Error) -> StorageError {
        map_io_as(p, e, CURRENT_OS)
    }

    /// Creates the temp file exclusively. A leftover regular file is removed
    /// once and creation retried only if its modified time is readable, in
    /// the past, and at least `STALE_TEMP_AGE` old. Anything else (a fresh or
    /// future-dated file, a directory, a symlink) is left alone and the
    /// original error returned.
    fn create_temp(path: &std::path::Path) -> io::Result<fs::File> {
        let create = || OpenOptions::new().write(true).create_new(true).open(path);
        match create() {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                let stale = fs::symlink_metadata(path).ok().is_some_and(|m| {
                    m.is_file()
                        && m.modified()
                            .ok()
                            .and_then(|t| SystemTime::now().duration_since(t).ok())
                            .is_some_and(|age| age >= STALE_TEMP_AGE)
                });
                if stale {
                    fs::remove_file(path)?;
                    create()
                } else {
                    Err(e)
                }
            }
            other => other,
        }
    }

    fn write_once(
        p: &RelPath,
        target: &std::path::Path,
        tmp: &std::path::Path,
        data: &[u8],
    ) -> Result<(), StorageError> {
        let mut file = Self::create_temp(tmp).map_err(|e| Self::map_io(p, &e))?;
        // From here the temp file is ours; remove it on any failure.
        let result = file
            .write_all(data)
            .and_then(|()| file.sync_all())
            .map_err(|e| Self::map_io(p, &e))
            .and_then(|()| {
                // Close the handle first: Windows cannot rename an open file.
                drop(file);
                // A save keeps the permission mode of the file it replaces
                // (R69). Best effort: the bytes matter more than the mode.
                #[cfg(unix)]
                {
                    if let Ok(meta) = fs::metadata(target) {
                        let _ = fs::set_permissions(tmp, meta.permissions());
                    }
                }
                // Code 5 is retried here; the outer save loop never retries
                // PermissionDenied, so the two loops do not multiply.
                rename_with_retry(p, CURRENT_OS, SAVE_RETRIES + 1, RETRY_DELAY, || {
                    fs::rename(tmp, target)
                })
            });
        if result.is_err() {
            let _ = fs::remove_file(tmp);
        }
        result
    }
}

impl Storage for FsStorage {
    fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError> {
        fs::read(self.resolve(p)?).map_err(|e| Self::map_io(p, &e))
    }

    fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError> {
        let parent = p.parent().ok_or_else(|| {
            StorageError::Io(p.clone(), "cannot write to the storage root".into())
        })?;
        let target = self.resolve(p)?;
        let tmp = self.resolve(&parent)?.join(temp_name_for(p.file_name()));
        with_retry(SAVE_RETRIES + 1, RETRY_DELAY, || {
            Self::write_once(p, &target, &tmp, data)
        })
    }

    fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError> {
        let rd = fs::read_dir(self.resolve(dir)?).map_err(|e| Self::map_io(dir, &e))?;
        let mut out = Vec::new();
        for entry in rd {
            let entry = entry.map_err(|e| Self::map_io(dir, &e))?;
            let is_dir = entry
                .file_type()
                .map_err(|e| Self::map_io(dir, &e))?
                .is_dir();
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            out.push(Entry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_dir,
                modified,
            });
        }
        Ok(out)
    }

    fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError> {
        fs::create_dir_all(self.resolve(p)?).map_err(|e| Self::map_io(p, &e))
    }

    fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), StorageError> {
        let (src, dst) = (self.resolve(from)?, self.resolve(to)?);
        rename_with_retry(from, CURRENT_OS, SAVE_RETRIES + 1, RETRY_DELAY, || {
            fs::rename(&src, &dst)
        })
    }

    fn remove_file(&self, p: &RelPath) -> Result<(), StorageError> {
        fs::remove_file(self.resolve(p)?).map_err(|e| Self::map_io(p, &e))
    }

    fn remove_dir(&self, p: &RelPath) -> Result<(), StorageError> {
        fs::remove_dir(self.resolve(p)?).map_err(|e| Self::map_io(p, &e))
    }

    fn exists(&self, p: &RelPath) -> bool {
        self.resolve(p).map(|path| path.exists()).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::TEMP_SUFFIX;
    use std::cell::Cell;
    use std::time::Duration;

    fn tmp() -> (tempfile::TempDir, FsStorage) {
        let d = tempfile::tempdir().unwrap();
        let s = FsStorage::new(d.path());
        (d, s)
    }

    fn names(s: &FsStorage) -> Vec<String> {
        let mut v: Vec<_> = s
            .list(&RelPath::root())
            .unwrap()
            .into_iter()
            .map(|e| e.name)
            .collect();
        v.sort();
        v
    }

    #[test]
    fn stale_leftover_temp_file_is_replaced_and_save_succeeds() {
        let (d, s) = tmp();
        let p = RelPath::new("doc.md").unwrap();
        s.write_atomic(&p, b"original").unwrap();
        let tmp_path = d.path().join(temp_name_for("doc.md"));
        std::fs::write(&tmp_path, b"stale").unwrap();
        set_modified(&tmp_path, SystemTime::now() - Duration::from_secs(3600));
        s.write_atomic(&p, b"new").unwrap();
        assert_eq!(s.read(&p).unwrap(), b"new");
        assert_eq!(names(&s), vec!["doc.md".to_string()]);
    }

    fn set_modified(path: &std::path::Path, when: SystemTime) {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn fresh_leftover_temp_file_is_not_touched() {
        let (d, s) = tmp();
        let p = RelPath::new("doc.md").unwrap();
        s.write_atomic(&p, b"original").unwrap();
        let tmp_path = d.path().join(temp_name_for("doc.md"));
        std::fs::write(&tmp_path, b"live").unwrap();
        let start = std::time::Instant::now();
        assert!(s.write_atomic(&p, b"new").is_err());
        assert!(start.elapsed() >= Duration::from_millis(900));
        assert_eq!(s.read(&p).unwrap(), b"original");
        assert_eq!(std::fs::read(&tmp_path).unwrap(), b"live");
    }

    #[test]
    fn future_dated_temp_file_is_not_touched() {
        let (d, s) = tmp();
        let p = RelPath::new("doc.md").unwrap();
        s.write_atomic(&p, b"original").unwrap();
        let tmp_path = d.path().join(temp_name_for("doc.md"));
        std::fs::write(&tmp_path, b"live").unwrap();
        set_modified(&tmp_path, SystemTime::now() + Duration::from_secs(3600));
        assert!(s.write_atomic(&p, b"new").is_err());
        assert_eq!(s.read(&p).unwrap(), b"original");
        assert_eq!(std::fs::read(&tmp_path).unwrap(), b"live");
    }

    #[test]
    fn leftover_temp_directory_survives_and_save_fails() {
        let (d, s) = tmp();
        let p = RelPath::new("doc.md").unwrap();
        s.write_atomic(&p, b"original").unwrap();
        let blocker = d.path().join(temp_name_for("doc.md"));
        std::fs::create_dir(&blocker).unwrap();
        std::fs::write(blocker.join("inner.txt"), b"keep").unwrap();
        assert!(s.write_atomic(&p, b"new").is_err());
        assert!(blocker.is_dir());
        assert_eq!(std::fs::read(blocker.join("inner.txt")).unwrap(), b"keep");
        assert_eq!(s.read(&p).unwrap(), b"original");
    }

    #[test]
    fn failure_after_temp_exists_cleans_up_and_keeps_target() {
        let (d, s) = tmp();
        let target = d.path().join("doc.md");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("inner.txt"), b"keep").unwrap();
        let p = RelPath::new("doc.md").unwrap();
        assert!(s.write_atomic(&p, b"new").is_err());
        assert_eq!(names(&s), vec!["doc.md".to_string()]);
        assert!(target.is_dir());
        assert_eq!(std::fs::read(target.join("inner.txt")).unwrap(), b"keep");
    }

    #[test]
    fn write_atomic_on_root_is_an_error() {
        let (_d, s) = tmp();
        assert!(s.write_atomic(&RelPath::root(), b"x").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_keeps_the_permission_mode_of_the_file_it_replaces() {
        use std::os::unix::fs::PermissionsExt;
        let (d, s) = tmp();
        let p = RelPath::new("doc.md").unwrap();
        s.write_atomic(&p, b"one").unwrap();
        let f = d.path().join("doc.md");
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o600)).unwrap();
        s.write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&f).unwrap(), b"two");
        let mode = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "mode {mode:o}");
    }

    #[test]
    fn temp_name_uses_brand_suffix() {
        assert_eq!(temp_name_for("a.md"), format!("~$a.md{TEMP_SUFFIX}"));
        assert!(!is_temp_name(TEMP_SUFFIX));
        assert!(!is_temp_name("~$a.md"));
    }

    #[test]
    fn with_retry_attempts_are_total_tries() {
        let p = RelPath::new("x").unwrap();
        let calls = Cell::new(0);
        let r: Result<(), _> = with_retry(3, Duration::from_millis(1), || {
            calls.set(calls.get() + 1);
            Err(StorageError::Locked(p.clone(), "busy".into()))
        });
        assert!(r.is_err());
        assert_eq!(calls.get(), 3);

        calls.set(0);
        let r: Result<(), _> = with_retry(0, Duration::from_millis(1), || {
            calls.set(calls.get() + 1);
            Err(StorageError::Locked(p.clone(), "busy".into()))
        });
        assert!(r.is_err());
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn with_retry_does_not_sleep_after_final_failure() {
        let p = RelPath::new("x").unwrap();
        let start = std::time::Instant::now();
        let _: Result<(), _> = with_retry(2, Duration::from_millis(400), || {
            Err(StorageError::Io(p.clone(), "e".into()))
        });
        assert!(start.elapsed() < Duration::from_millis(700));
    }

    fn os_err(code: i32) -> io::Error {
        io::Error::from_raw_os_error(code)
    }

    #[test]
    fn only_windows_code_5_makes_a_rename_retry() {
        assert!(rename_is_retried(Some(5), Os::Windows));
        assert!(!rename_is_retried(Some(5), Os::Unix));
        assert!(!rename_is_retried(Some(32), Os::Windows));
        assert!(!rename_is_retried(Some(2), Os::Windows));
        assert!(!rename_is_retried(None, Os::Windows));
    }

    #[test]
    fn windows_code_5_on_rename_is_retried_until_it_succeeds() {
        let p = RelPath::new("x").unwrap();
        let calls = Cell::new(0);
        let r = rename_with_retry(
            &p,
            Os::Windows,
            SAVE_RETRIES + 1,
            Duration::from_millis(1),
            || {
                calls.set(calls.get() + 1);
                if calls.get() < 3 {
                    Err(os_err(5))
                } else {
                    Ok(())
                }
            },
        );
        assert!(r.is_ok());
        assert_eq!(calls.get(), 3);
    }

    #[test]
    fn windows_code_5_on_rename_gives_up_as_permission_denied_after_5_retries() {
        let p = RelPath::new("x").unwrap();
        let calls = Cell::new(0);
        let r = rename_with_retry(
            &p,
            Os::Windows,
            SAVE_RETRIES + 1,
            Duration::from_millis(1),
            || {
                calls.set(calls.get() + 1);
                Err(os_err(5))
            },
        );
        assert_eq!(r.unwrap_err().cause(), ErrorCause::PermissionDenied);
        assert_eq!(calls.get(), 6);
    }

    #[test]
    fn other_rename_errors_are_not_retried() {
        let p = RelPath::new("x").unwrap();
        for (code, os, cause) in [
            (32, Os::Windows, ErrorCause::Locked),
            (2, Os::Windows, ErrorCause::NotFound),
            (5, Os::Unix, ErrorCause::Io),
            (13, Os::Unix, ErrorCause::PermissionDenied),
        ] {
            let calls = Cell::new(0);
            let r = rename_with_retry(&p, os, SAVE_RETRIES + 1, Duration::from_millis(1), || {
                calls.set(calls.get() + 1);
                Err(os_err(code))
            });
            assert_eq!(r.unwrap_err().cause(), cause, "code {code} on {os:?}");
            assert_eq!(calls.get(), 1, "code {code} on {os:?} must not retry");
        }
    }

    #[test]
    fn disk_full_is_not_retried() {
        let p = RelPath::new("x").unwrap();
        let calls = Cell::new(0);
        let r: Result<(), _> = with_retry(5, Duration::from_millis(1), || {
            calls.set(calls.get() + 1);
            Err(StorageError::DiskFull(p.clone(), "full".into()))
        });
        assert_eq!(r.unwrap_err().cause(), ErrorCause::DiskFull);
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn relpath_rejects_unsafe_shapes() {
        for bad in [
            "a/./b",
            "a//b",
            "a/",
            "C:x",
            "a.md:stream",
            "\\\\server\\share",
            "",
            ".",
            "a/\0b",
        ] {
            assert!(RelPath::new(bad).is_err(), "should reject {bad:?}");
        }
        assert_eq!(RelPath::new("a/b").unwrap().as_str(), "a/b");
    }

    #[test]
    fn join_never_escapes() {
        assert_eq!(RelPath::root().join("../x").as_str(), "x");
        let a = RelPath::new("a").unwrap();
        assert_eq!(a.join("..").as_str(), "a");
        assert_eq!(a.join("b\\c").as_str(), "a/b/c");
        assert_eq!(a.join("./b//c/").as_str(), "a/b/c");
        assert_eq!(RelPath::root().join("x").as_str(), "x");
    }

    #[test]
    fn parent_and_file_name() {
        assert_eq!(RelPath::root().parent(), None);
        let a = RelPath::new("a").unwrap();
        assert_eq!(a.parent(), Some(RelPath::root()));
        let ab = RelPath::new("a/b.md").unwrap();
        assert_eq!(ab.parent(), Some(a));
        assert_eq!(ab.file_name(), "b.md");
    }

    #[test]
    fn relpath_serde_validates_and_round_trips() {
        assert!(serde_json::from_str::<RelPath>("\"../x\"").is_err());
        assert!(serde_json::from_str::<RelPath>("\"\"").is_err());
        let p = RelPath::new("a/b.md").unwrap();
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(json, "\"a/b.md\"");
        assert_eq!(serde_json::from_str::<RelPath>(&json).unwrap(), p);
    }

    #[cfg(windows)]
    #[test]
    fn colon_paths_are_refused_on_windows() {
        let (_d, s) = tmp();
        let p = RelPath::root().join("C:x");
        assert!(s.read(&p).is_err());
        assert!(s.write_atomic(&p, b"x").is_err());
        assert!(!s.exists(&p));
    }
}
