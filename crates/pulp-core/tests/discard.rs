// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, id::Id, manifest::Node, project::*, storage::*};
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

fn setup() -> (tempfile::TempDir, Project<Recording>, Id) {
    let d = tempfile::tempdir().unwrap();
    let storage = Recording {
        inner: FsStorage::new(d.path()),
        root: d.path().to_path_buf(),
        recorded: RefCell::new(Vec::new()),
    };
    let p = Project::create(storage, "X").unwrap();
    let ms = match &p.manifest().tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    (d, p, ms)
}

fn trash_two(p: &mut Project<Recording>, ms: &Id) -> (Id, Id) {
    let a = p.create_document(ms, "A", DocKind::Prose).unwrap();
    let b = p.create_document(ms, "B", DocKind::Prose).unwrap();
    (p.trash_item(&a).unwrap(), p.trash_item(&b).unwrap())
}

#[test]
fn delete_from_trash_discards_only_that_entry() {
    let (_d, mut p, ms) = setup();
    let (ea, eb) = trash_two(&mut p, &ms);
    p.delete_from_trash(&ea).unwrap();
    assert_eq!(*p.storage().recorded.borrow(), vec![format!(".trash/{ea}")]);
    let left: Vec<_> = p
        .list_trash()
        .unwrap()
        .into_iter()
        .map(|e| e.entry_id)
        .collect();
    assert_eq!(left, vec![eb]);
}

#[test]
fn delete_unknown_entry_is_not_found() {
    let (_d, mut p, _ms) = setup();
    let r = p.delete_from_trash(&Id::new());
    assert!(matches!(r, Err(ProjectError::NotFound(_))));
    assert!(p.storage().recorded.borrow().is_empty());
}

#[test]
fn empty_trash_discards_all_and_counts() {
    let (_d, mut p, ms) = setup();
    trash_two(&mut p, &ms);
    assert_eq!(p.empty_trash().unwrap(), 2);
    assert!(p.list_trash().unwrap().is_empty());
}

#[test]
#[ignore = "moves a file to the real OS trash; run by hand"]
fn fs_discard_moves_to_os_trash() {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("gone.txt"), "x").unwrap();
    let s = FsStorage::new(d.path());
    s.discard(&RelPath::new("gone.txt").unwrap()).unwrap();
    assert!(!d.path().join("gone.txt").exists());
}
