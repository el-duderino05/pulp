// SPDX-License-Identifier: MPL-2.0
use pulp_core::library::*;

#[test]
fn create_makes_protected_folders_and_settings() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::create(d.path(), "Gonçalo").unwrap();
    for name in LIBRARY_DIRS {
        assert!(d.path().join(name).is_dir(), "{name} missing");
    }
    assert!(d.path().join("library.json").is_file());
    assert_eq!(lib.settings.author.name, "Gonçalo");
}

#[test]
fn opening_a_folder_without_library_json_fails() {
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(
        Library::open(d.path()),
        Err(LibraryError::NotALibrary(_))
    ));
}

#[test]
fn open_recreates_missing_protected_folders_and_reports_them() {
    let d = tempfile::tempdir().unwrap();
    Library::create(d.path(), "G").unwrap();
    std::fs::remove_dir(d.path().join("Inbox")).unwrap();
    let (_lib, recreated) = Library::open(d.path()).unwrap();
    assert_eq!(recreated, vec!["Inbox".to_string()]);
    assert!(d.path().join("Inbox").is_dir());
}

#[test]
fn library_json_keeps_unknown_keys_and_author_id() {
    let d = tempfile::tempdir().unwrap();
    let id = Library::create(d.path(), "G").unwrap().settings.author.id;
    let p = d.path().join("library.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    v["futureKey"] = serde_json::json!(1);
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let (lib, _) = Library::open(d.path()).unwrap();
    assert_eq!(lib.settings.author.id, id);
    assert!(lib.settings.extra.contains_key("futureKey"));
}
