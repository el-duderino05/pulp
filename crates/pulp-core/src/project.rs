// SPDX-License-Identifier: MPL-2.0
//! Projects: a folder with a manifest plus Markdown documents found by header.

use std::collections::HashMap;
use std::time::SystemTime;

pub use crate::brand::MANIFEST_FILE;
use crate::header::{HeaderError, parse_markdown_item};
use crate::id::Id;
use crate::manifest::{Manifest, ManifestError};
use crate::storage::{RelPath, Storage, StorageError, is_temp_name};
use crate::text::normalize_text;
use crate::version::{VersionError, check_readable};

pub const PROJECT_DIRS: [&str; 6] = [
    "manuscript",
    "notes",
    "assets",
    "shelf",
    ".history",
    ".trash",
];
pub const HIDDEN_DIRS: [&str; 3] = [".history", ".trash", ".templates"];

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Header(#[from] HeaderError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Version(#[from] VersionError),
    #[error("no item with id {0}")]
    NotFound(Id),
    #[error("item {0} is not a folder")]
    NotAFolder(Id),
    #[error("this folder is already a project")]
    AlreadyAProject,
    #[error("{0} already exists, so nothing was changed")]
    Exists(RelPath),
}

#[derive(Debug, Default)]
pub struct ReconcileReport {
    pub found: Vec<Id>,
    pub missing: Vec<Id>,
    pub duplicates_trashed: Vec<RelPath>,
    pub copies_reassigned: Vec<(RelPath, Id)>,
    pub conflicts: Vec<(Id, Vec<RelPath>)>,
    pub recreated_dirs: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScannedItem {
    pub id: Id,
    pub path: RelPath,
    pub modified: SystemTime,
}

pub struct Project<S: Storage> {
    pub(crate) storage: S,
    pub(crate) manifest: Manifest,
    pub(crate) index: HashMap<Id, RelPath>,
}

impl<S: Storage> Project<S> {
    /// Makes a new project in the storage root. Refuses a folder that already
    /// has a manifest, changing nothing. The manifest is written last, so a
    /// half-made project has none.
    pub fn create(storage: S, name: &str) -> Result<Project<S>, ProjectError> {
        let manifest_path = RelPath::root().join(MANIFEST_FILE);
        if storage.exists(&manifest_path) {
            return Err(ProjectError::AlreadyAProject);
        }
        storage.create_dir_all(&RelPath::root())?;
        for dir in PROJECT_DIRS {
            storage.create_dir_all(&RelPath::root().join(dir))?;
        }
        let manifest = Manifest::new(&normalize_text(name));
        storage.write_atomic(&manifest_path, manifest.to_json().as_bytes())?;
        Ok(Project {
            storage,
            manifest,
            index: HashMap::new(),
        })
    }

    /// Opens a project. Only reads files, apart from recreating a missing
    /// `.history` or `.trash` folder.
    pub fn open(storage: S) -> Result<(Project<S>, ReconcileReport), ProjectError> {
        let bytes = storage.read(&RelPath::root().join(MANIFEST_FILE))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| ManifestError::Json("the manifest is not valid UTF-8".to_string()))?;
        let manifest = match Manifest::load(&text) {
            Ok(m) => m,
            Err(ManifestError::Version(v)) => return Err(ProjectError::Version(v)),
            Err(e) => return Err(ProjectError::Manifest(e)),
        };

        let mut report = ReconcileReport::default();
        for dir in PROJECT_DIRS.iter().filter(|d| HIDDEN_DIRS.contains(d)) {
            let p = RelPath::root().join(dir);
            if !storage.exists(&p) {
                storage.create_dir_all(&p)?;
                report.recreated_dirs.push((*dir).to_string());
            }
        }

        let mut index = HashMap::new();
        for item in scan(&storage)? {
            index.entry(item.id).or_insert(item.path);
        }
        Ok((
            Project {
                storage,
                manifest,
                index,
            },
            report,
        ))
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    pub fn path_of(&self, id: &Id) -> Option<&RelPath> {
        self.index.get(id)
    }

    pub fn storage(&self) -> &S {
        &self.storage
    }
}

/// Finds every Pulp document under the project root by reading headers.
/// Skips hidden and dot folders, symlinked folders, temp files, and any
/// `.md` file that is not UTF-8, has no valid header, or needs a newer
/// reader. Unreadable files and subfolders are skipped; only a failure to
/// list the root fails the scan. Sorted by path.
pub fn scan<S: Storage>(storage: &S) -> Result<Vec<ScannedItem>, ProjectError> {
    let mut found = Vec::new();
    let mut pending = vec![RelPath::root()];
    while let Some(dir) = pending.pop() {
        let entries = match storage.list(&dir) {
            Ok(e) => e,
            Err(e) if dir == RelPath::root() => return Err(e.into()),
            Err(_) => continue,
        };
        for entry in entries {
            if is_temp_name(&entry.name) {
                continue;
            }
            let path = dir.join(&entry.name);
            if entry.is_dir {
                if entry.name.starts_with('.') || HIDDEN_DIRS.contains(&entry.name.as_str()) {
                    continue;
                }
                pending.push(path);
            } else if entry.name.to_lowercase().ends_with(".md")
                && let Some(id) = read_item_id(storage, &path)
            {
                found.push(ScannedItem {
                    id,
                    path,
                    modified: entry.modified,
                });
            }
        }
    }
    found.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    Ok(found)
}

fn read_item_id<S: Storage>(storage: &S, path: &RelPath) -> Option<Id> {
    let text = String::from_utf8(storage.read(path).ok()?).ok()?;
    let item = parse_markdown_item(&text).ok()?;
    check_readable(item.header.min_reader).ok()?;
    Some(item.header.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{DocKind, ItemFile, ItemHeader, render_markdown_item};
    use crate::library::Library;
    use crate::storage::{ErrorCause, FsStorage};
    use std::path::Path;

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn item_text(title: &str) -> (Id, String) {
        let h = ItemHeader::new_document(title, DocKind::Prose);
        let id = h.id.clone();
        let text = render_markdown_item(&ItemFile {
            header: h,
            body: "x\n".into(),
        });
        (id, text)
    }

    fn put_item(root: &Path, rel: &str, title: &str) -> Id {
        let (id, text) = item_text(title);
        put(root, rel, text.as_bytes());
        id
    }

    fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    let rel = p
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }

    #[test]
    fn create_refuses_an_existing_project_and_changes_nothing() {
        let d = tempfile::tempdir().unwrap();
        put(d.path(), MANIFEST_FILE, b"keep me exactly");
        let r = Project::create(FsStorage::new(d.path()), "X");
        assert!(matches!(r, Err(ProjectError::AlreadyAProject)));
        assert_eq!(
            std::fs::read(d.path().join(MANIFEST_FILE)).unwrap(),
            b"keep me exactly"
        );
        assert!(!d.path().join("manuscript").exists());
    }

    #[test]
    fn create_stores_the_name_in_nfc() {
        let d = tempfile::tempdir().unwrap();
        let p = Project::create(FsStorage::new(d.path()), "Ac\u{0327}a\u{0303}o").unwrap();
        assert_eq!(p.manifest().name, "A\u{e7}\u{e3}o");
        let text = std::fs::read_to_string(d.path().join(MANIFEST_FILE)).unwrap();
        assert_eq!(Manifest::load(&text).unwrap().name, "A\u{e7}\u{e3}o");
    }

    #[test]
    fn scan_finds_nested_and_uppercase_and_skips_everything_else() {
        let d = tempfile::tempdir().unwrap();
        Project::create(FsStorage::new(d.path()), "X").unwrap();
        let nested = put_item(d.path(), "manuscript/part-one/a.md", "A");
        let upper = put_item(d.path(), "notes/X.MD", "X");
        put_item(d.path(), ".obsidian/a.md", "Hidden");
        put_item(d.path(), "manuscript/.cache/b.md", "Hidden deep");
        put_item(d.path(), ".templates/t.md", "Template");
        put_item(d.path(), "notes/plain.txt", "Not markdown");
        let (_, text) = item_text("Future");
        assert!(text.contains("minReader: 1"));
        put(
            d.path(),
            "notes/future.md",
            text.replace("minReader: 1", "minReader: 99").as_bytes(),
        );
        put(d.path(), "notes/README.md", b"Just a readme\r\n");
        put(
            d.path(),
            "notes/broken.md",
            b"---\nid: [unclosed\n---\nbody\n",
        );
        put(d.path(), "notes/binary.md", &[0xff, 0xfe, 0x00, 0x80]);

        let found = scan(&FsStorage::new(d.path())).unwrap();
        let paths: Vec<&str> = found.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(paths, vec!["manuscript/part-one/a.md", "notes/X.MD"]);
        assert_eq!(found[0].id, nested);
        assert_eq!(found[1].id, upper);
    }

    #[test]
    fn open_changes_no_file_bytes() {
        let d = tempfile::tempdir().unwrap();
        Project::create(FsStorage::new(d.path()), "X").unwrap();
        put(
            d.path(),
            "notes/README.md",
            b"Just a readme\r\nline two\r\n",
        );
        let id = put_item(d.path(), "manuscript/a.md", "A");
        let before = snapshot(d.path());
        let (p, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/a.md");
        assert_eq!(snapshot(d.path()), before);
    }

    #[test]
    fn open_maps_too_new_to_version_and_missing_to_not_found() {
        let d = tempfile::tempdir().unwrap();
        Project::create(FsStorage::new(d.path()), "X").unwrap();
        let mp = d.path().join(MANIFEST_FILE);
        let mut v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&mp).unwrap()).unwrap();
        v["minReader"] = serde_json::json!(99);
        std::fs::write(&mp, serde_json::to_vec(&v).unwrap()).unwrap();
        assert!(matches!(
            Project::open(FsStorage::new(d.path())),
            Err(ProjectError::Version(_))
        ));

        let e = tempfile::tempdir().unwrap();
        match Project::open(FsStorage::new(e.path())) {
            Err(ProjectError::Storage(s)) => assert_eq!(s.cause(), ErrorCause::NotFound),
            other => panic!("expected Storage(NotFound), got {:?}", other.err()),
        }
    }

    #[test]
    fn duplicate_ids_resolve_to_the_first_path_in_sorted_order() {
        let d = tempfile::tempdir().unwrap();
        Project::create(FsStorage::new(d.path()), "X").unwrap();
        let (id, text) = item_text("A");
        put(d.path(), "notes/b.md", text.as_bytes());
        put(d.path(), "manuscript/z.md", text.as_bytes());
        let (p, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/z.md");
    }

    #[test]
    fn create_project_slugs_a_decomposed_name_and_stores_nfc() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::create(d.path(), "G").unwrap();
        let path = lib.create_project("Ac\u{0327}a\u{0303}o").unwrap();
        assert!(path.ends_with("Projects/acao"));
        let text = std::fs::read_to_string(path.join(MANIFEST_FILE)).unwrap();
        assert_eq!(Manifest::load(&text).unwrap().name, "A\u{e7}\u{e3}o");
    }
}
