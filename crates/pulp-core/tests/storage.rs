// SPDX-License-Identifier: MPL-2.0
use pulp_core::brand::TEMP_SUFFIX;
use pulp_core::storage::*;
use std::{cell::Cell, time::Duration};

fn tmp() -> (tempfile::TempDir, FsStorage) {
    let d = tempfile::tempdir().unwrap();
    let s = FsStorage::new(d.path());
    (d, s)
}

#[test]
fn relpath_normalises_and_rejects_escapes() {
    assert_eq!(RelPath::new("a\\b.md").unwrap().as_str(), "a/b.md");
    assert!(RelPath::new("../x").is_err());
    assert!(RelPath::new("a/../../x").is_err());
    assert!(RelPath::new("/abs").is_err());
    assert!(RelPath::new("C:\\x").is_err());
}

#[test]
fn write_atomic_creates_replaces_and_leaves_no_temp_file() {
    let (_d, s) = tmp();
    let p = RelPath::new("doc.md").unwrap();
    s.write_atomic(&p, b"one").unwrap();
    s.write_atomic(&p, b"two").unwrap();
    assert_eq!(s.read(&p).unwrap(), b"two");
    let names: Vec<_> = s
        .list(&RelPath::root())
        .unwrap()
        .into_iter()
        .map(|e| e.name)
        .collect();
    assert_eq!(names, vec!["doc.md".to_string()]);
}

#[test]
fn failed_save_leaves_original_untouched() {
    let (d, s) = tmp();
    let p = RelPath::new("doc.md").unwrap();
    s.write_atomic(&p, b"original").unwrap();
    // Block the temp file path with a directory so the save cannot proceed.
    std::fs::create_dir(d.path().join(temp_name_for("doc.md"))).unwrap();
    assert!(s.write_atomic(&p, b"new").is_err());
    assert_eq!(s.read(&p).unwrap(), b"original");
}

#[test]
fn temp_names_are_recognised() {
    assert_eq!(temp_name_for("a.md"), format!("~$a.md{TEMP_SUFFIX}"));
    assert!(is_temp_name(&format!("~$a.md{TEMP_SUFFIX}")));
    assert!(!is_temp_name("a.md"));
}

#[test]
fn os_errors_are_classified() {
    assert_eq!(classify_os_error(32, Os::Windows), ErrorCause::Locked);
    assert_eq!(classify_os_error(33, Os::Windows), ErrorCause::Locked);
    assert_eq!(classify_os_error(112, Os::Windows), ErrorCause::DiskFull);
    assert_eq!(
        classify_os_error(5, Os::Windows),
        ErrorCause::PermissionDenied
    );
    assert_eq!(classify_os_error(28, Os::Unix), ErrorCause::DiskFull);
    assert_eq!(
        classify_os_error(13, Os::Unix),
        ErrorCause::PermissionDenied
    );
}

#[test]
fn retry_repeats_locked_but_not_permission_errors() {
    let p = RelPath::new("x").unwrap();
    let calls = Cell::new(0);
    let r = with_retry(5, Duration::from_millis(1), || {
        calls.set(calls.get() + 1);
        if calls.get() < 3 {
            Err(StorageError::Locked(p.clone(), "busy".into()))
        } else {
            Ok(7)
        }
    });
    assert_eq!(r.unwrap(), 7);
    assert_eq!(calls.get(), 3);

    calls.set(0);
    let r: Result<(), _> = with_retry(5, Duration::from_millis(1), || {
        calls.set(calls.get() + 1);
        Err(StorageError::PermissionDenied(p.clone(), "no".into()))
    });
    assert!(r.is_err());
    assert_eq!(calls.get(), 1);
}
