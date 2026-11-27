// SPDX-License-Identifier: MPL-2.0
use pulp_core::{id::Id, library::*, manifest::Manifest, storage::*};
use std::cell::RefCell;
use std::path::PathBuf;

/// `FsStorage` whose `discard` records the path and deletes it for good, so
/// the tests do not depend on a desktop trash.
struct Recording {
    inner: FsStorage,
    root: PathBuf,
    recorded: RefCell<Vec<String>>,
}

impl Storage for Recording {
    fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError> {
        self.inner.read(p)
    }
    fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError> {
        self.inner.write_atomic(p, data)
    }
    fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError> {
        self.inner.list(dir)
    }
    fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError> {
        self.inner.create_dir_all(p)
    }
    fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), StorageError> {
        self.inner.rename(from, to)
    }
    fn remove_file(&self, p: &RelPath) -> Result<(), StorageError> {
        self.inner.remove_file(p)
    }
    fn remove_dir(&self, p: &RelPath) -> Result<(), StorageError> {
        self.inner.remove_dir(p)
    }
    fn exists(&self, p: &RelPath) -> bool {
        self.inner.exists(p)
    }
    fn discard(&self, p: &RelPath) -> Result<(), StorageError> {
        self.recorded.borrow_mut().push(p.as_str().to_string());
        let path = self.root.join(p.as_str());
        let r = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        r.map_err(|e| StorageError::Io(p.clone(), e.to_string()))
    }
}

fn setup() -> (tempfile::TempDir, Library) {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::create(d.path(), "G").unwrap();
    (d, lib)
}

fn recording(d: &tempfile::TempDir) -> Recording {
    Recording {
        inner: FsStorage::new(d.path()),
        root: d.path().to_path_buf(),
        recorded: RefCell::new(Vec::new()),
    }
}

#[test]
fn trash_and_restore_project_round_trip() {
    let (d, lib) = setup();
    let abs = lib.create_project("My Novel").unwrap();
    let manifest = abs.join("pulp-project.json");
    let before = std::fs::read(&manifest).unwrap();
    let folder = abs.file_name().unwrap().to_str().unwrap().to_string();

    let entry = lib.trash_project(&folder).unwrap();
    assert!(lib.list_projects().unwrap().is_empty());
    assert!(!abs.exists());
    let trash = lib.list_trash().unwrap();
    assert_eq!(trash.len(), 1);
    assert_eq!(trash[0].entry_id, entry);
    assert_eq!(trash[0].name, "My Novel");
    assert!(!trash[0].deleted_at.is_empty());

    let back = lib.restore_project(&entry).unwrap();
    assert_eq!(back, d.path().join("Projects").join(&folder));
    assert_eq!(
        std::fs::read(back.join("pulp-project.json")).unwrap(),
        before
    );
    assert_eq!(lib.list_projects().unwrap().len(), 1);
    assert!(lib.list_trash().unwrap().is_empty());
    assert!(!d.path().join(".trash").join(entry.as_str()).exists());
}

#[test]
fn restore_on_clash_gets_unique_folder() {
    let (_d, lib) = setup();
    let abs = lib.create_project("Draft").unwrap();
    let folder = abs.file_name().unwrap().to_str().unwrap().to_string();
    let entry = lib.trash_project(&folder).unwrap();
    let again = lib.create_project("Draft").unwrap();
    assert_eq!(again.file_name().unwrap().to_str().unwrap(), folder);

    let back = lib.restore_project(&entry).unwrap();
    assert_eq!(
        back.file_name().unwrap().to_str().unwrap(),
        format!("{folder}-2")
    );
    assert!(back.join("pulp-project.json").is_file());
    assert!(again.join("pulp-project.json").is_file());
}

#[test]
fn delete_and_empty_use_discard() {
    let (d, lib) = setup();
    let mut ids = Vec::new();
    for n in ["One", "Two", "Three"] {
        let abs = lib.create_project(n).unwrap();
        ids.push(
            lib.trash_project(abs.file_name().unwrap().to_str().unwrap())
                .unwrap(),
        );
    }
    let s = recording(&d);

    lib.delete_from_trash_with(&s, &ids[0]).unwrap();
    assert_eq!(*s.recorded.borrow(), vec![format!(".trash/{}", ids[0])]);
    assert!(!d.path().join(".trash").join(ids[0].as_str()).exists());
    assert_eq!(lib.list_trash().unwrap().len(), 2);

    assert_eq!(lib.empty_trash_with(&s).unwrap(), 2);
    assert!(lib.list_trash().unwrap().is_empty());
    assert_eq!(s.recorded.borrow().len(), 3);
}

#[test]
fn too_new_project_is_flagged() {
    let (d, lib) = setup();
    lib.create_project("Fine").unwrap();
    let mut newer = Manifest::new("Future");
    newer.min_reader = 99;
    let dir = d.path().join("Projects").join("future");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("pulp-project.json"), newer.to_json()).unwrap();

    let list = lib.list_projects().unwrap();
    assert_eq!(list.len(), 2);
    for p in &list {
        assert_eq!(p.too_new, p.folder == "future", "{}", p.folder);
    }
    let json = serde_json::to_value(&list[0]).unwrap();
    assert!(json.get("tooNew").is_some());
}

#[test]
fn unknown_entry_is_reported() {
    let (d, lib) = setup();
    let abs = lib.create_project("Keep").unwrap();
    let entry = lib
        .trash_project(abs.file_name().unwrap().to_str().unwrap())
        .unwrap();
    let ghost = Id::new();
    let s = recording(&d);

    assert!(lib.restore_project(&ghost).is_err());
    assert!(lib.delete_from_trash_with(&s, &ghost).is_err());
    assert!(s.recorded.borrow().is_empty());
    assert!(lib.list_projects().unwrap().is_empty());
    let left: Vec<_> = lib
        .list_trash()
        .unwrap()
        .into_iter()
        .map(|e| e.entry_id)
        .collect();
    assert_eq!(left, vec![entry]);
}

#[test]
fn trash_project_rejects_a_bad_folder_and_changes_nothing() {
    let (_d, lib) = setup();
    lib.create_project("Safe").unwrap();
    assert!(lib.trash_project("../Safe").is_err());
    assert!(lib.trash_project("missing").is_err());
    assert_eq!(lib.list_projects().unwrap().len(), 1);
    assert!(lib.list_trash().unwrap().is_empty());
}
