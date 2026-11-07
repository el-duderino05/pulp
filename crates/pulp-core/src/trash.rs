// SPDX-License-Identifier: MPL-2.0
//! The project trash: `.trash/<entry-id>/` holds what was deleted plus an
//! `entry.json` saying where it came from. Nothing here overwrites or
//! discards: a failed step puts every moved file back.

use std::collections::HashSet;

use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::items::{companions_of, repath_folders};
use crate::manifest::{FolderNode, Node};
use crate::project::{Project, ProjectError, scan};
use crate::storage::{ErrorCause, RelPath, Storage};
use crate::text::normalize_text;
use crate::version::FORMAT_VERSION;

const TRASH_DIR: &str = ".trash";
const ENTRY_FILE: &str = "entry.json";

#[derive(Debug, Clone, PartialEq)]
pub struct TrashEntry {
    pub format: u32,
    /// The name of the entry's folder; it is not written to `entry.json`.
    pub entry_id: Id,
    /// The item's id, or the folder's id for a folder entry.
    pub item_id: Id,
    pub original_path: RelPath,
    pub original_folder: Option<Id>,
    pub index: usize,
    /// RFC 3339, UTC, whole seconds.
    pub deleted_at: String,
    /// Names as stored in the entry folder: the main file first, then its
    /// companions; for a folder entry, the folder's own name.
    pub files: Vec<String>,
    pub folder_node: Option<FolderNode>,
}

/// The JSON shape of `entry.json`; field order is the file's key order.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Wire {
    format: u32,
    item_id: Id,
    original_path: String,
    original_folder: Option<Id>,
    index: usize,
    deleted_at: String,
    files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    folder_node: Option<FolderNode>,
}

fn trash_root() -> RelPath {
    RelPath::root().join(TRASH_DIR)
}

fn entry_dir(entry: &Id) -> RelPath {
    trash_root().join(entry.as_str())
}

fn encode(entry: &TrashEntry) -> String {
    let wire = Wire {
        format: entry.format,
        item_id: entry.item_id.clone(),
        original_path: entry.original_path.as_str().to_string(),
        original_folder: entry.original_folder.clone(),
        index: entry.index,
        deleted_at: entry.deleted_at.clone(),
        files: entry.files.clone(),
        folder_node: entry.folder_node.clone(),
    };
    let mut s = serde_json::to_string_pretty(&wire).expect("an entry always serialises");
    s.push('\n');
    normalize_text(&s)
}

/// A plain file name that is safe to join under the entry folder.
fn is_stored_name(name: &str) -> bool {
    !name.is_empty()
        && name != ENTRY_FILE
        && RelPath::new(name).is_ok_and(|p| p.file_name() == name)
}

/// Reads and checks one entry folder. `None` for anything unusable.
fn load_entry<S: Storage>(storage: &S, entry_id: &Id) -> Option<TrashEntry> {
    let bytes = storage.read(&entry_dir(entry_id).join(ENTRY_FILE)).ok()?;
    let wire: Wire = serde_json::from_slice(&bytes).ok()?;
    if wire.format == 0 || wire.format > FORMAT_VERSION || wire.deleted_at.is_empty() {
        return None;
    }
    let original_path = RelPath::new(&wire.original_path).ok()?;
    if wire.files.is_empty() || !wire.files.iter().all(|f| is_stored_name(f)) {
        return None;
    }
    match &wire.folder_node {
        Some(node) => {
            if wire.files.len() != 1 || node.id != wire.item_id {
                return None;
            }
        }
        None => {
            // The main file, then only companions of it.
            let main = RelPath::root().join(&wire.files[0]);
            let allowed: Vec<String> = companions_of(&main)
                .iter()
                .map(|c| c.file_name().to_string())
                .collect();
            if !wire.files[1..].iter().all(|f| allowed.contains(f)) {
                return None;
            }
        }
    }
    Some(TrashEntry {
        format: wire.format,
        entry_id: entry_id.clone(),
        item_id: wire.item_id,
        original_path,
        original_folder: wire.original_folder,
        index: wire.index,
        deleted_at: wire.deleted_at,
        files: wire.files,
        folder_node: wire.folder_node,
    })
}

/// Best effort: moves each file (or folder) back, last first, unless
/// something now sits at its old place. Once everything is back, the entry
/// folder goes too; otherwise it stays so nothing is lost.
fn put_back<S: Storage>(storage: &S, dir: &RelPath, moved: &[(RelPath, RelPath)]) {
    let mut all_back = true;
    for (orig, stored) in moved.iter().rev() {
        if storage.exists(orig) || storage.rename(stored, orig).is_err() {
            all_back = false;
        }
    }
    if all_back {
        let _ = storage.remove_file(&dir.join(ENTRY_FILE));
        let _ = storage.remove_dir(dir);
    }
}

fn stored_pairs(dir: &RelPath, files: &[RelPath]) -> Vec<(RelPath, RelPath)> {
    files
        .iter()
        .map(|f| (f.clone(), dir.join(f.file_name())))
        .collect()
}

/// Moves `files` into `.trash/<entry-id>/` (each under its own file name,
/// refusing to replace anything) and writes `entry.json` last. If any step
/// fails the files are put back and the entry folder is removed.
pub fn store_in_trash<S: Storage>(
    storage: &S,
    files: &[RelPath],
    mut entry: TrashEntry,
) -> Result<Id, ProjectError> {
    let dir = entry_dir(&entry.entry_id);
    if storage.exists(&dir) {
        return Err(ProjectError::Exists(dir));
    }
    if let Some(f) = files.iter().find(|f| !is_stored_name(f.file_name())) {
        return Err(ProjectError::Exists(dir.join(f.file_name())));
    }
    if !storage.exists(&trash_root()) {
        storage.create_dir_all(&trash_root())?;
    }
    storage.create_dir_all(&dir)?;
    let pairs = stored_pairs(&dir, files);
    entry.files = files.iter().map(|f| f.file_name().to_string()).collect();
    let mut done = 0;
    let result = (|| {
        for (from, to) in &pairs {
            if storage.exists(to) {
                return Err(ProjectError::Exists(to.clone()));
            }
            storage.rename(from, to)?;
            done += 1;
        }
        storage.write_atomic(&dir.join(ENTRY_FILE), encode(&entry).as_bytes())?;
        Ok(())
    })();
    match result {
        Ok(()) => Ok(entry.entry_id),
        Err(e) => {
            put_back(storage, &dir, &pairs[..done]);
            Err(e)
        }
    }
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn item_ids_in(node: &Node, out: &mut Vec<Id>) {
    match node {
        Node::Item { id, .. } => out.push(id.clone()),
        Node::Folder(f) => f.children.iter().for_each(|c| item_ids_in(c, out)),
    }
}

/// True when `path` is `folder` or lies under it, by whole segments.
fn is_under(path: &str, folder: &str) -> bool {
    path == folder
        || path
            .strip_prefix(folder)
            .is_some_and(|r| r.starts_with('/'))
}

/// The stored index counts only in the folder it came from (or the top
/// level); when that folder is gone the node goes last at the top level.
fn at_index(e: &TrashEntry, parent: Option<&Id>) -> Option<usize> {
    (parent.is_some() || e.original_folder.is_none()).then_some(e.index)
}

impl<S: Storage> Project<S> {
    /// Moves the document and its existing companions into the trash and
    /// takes it out of the tree. Returns the entry id.
    pub fn trash_item(&mut self, id: &Id) -> Result<Id, ProjectError> {
        if self.manifest.folder(id).is_some() {
            return Err(ProjectError::NotFound(id.clone()));
        }
        let (path, _) = self.read_verified(id)?;
        let (folder, index) = self
            .manifest
            .parent_of(id)
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        let mut files = vec![path.clone()];
        files.extend(
            companions_of(&path)
                .into_iter()
                .filter(|c| self.storage.exists(c)),
        );
        let entry = TrashEntry {
            format: FORMAT_VERSION,
            entry_id: Id::new(),
            item_id: id.clone(),
            original_path: path,
            original_folder: folder,
            index,
            deleted_at: now(),
            files: Vec::new(),
            folder_node: None,
        };
        let entry_id = store_in_trash(&self.storage, &files, entry)?;

        let snap = self.capture_state();
        self.manifest.remove_node(id);
        self.index.remove(id);
        if let Err(e) = self.save_manifest() {
            self.restore(snap);
            let dir = entry_dir(&entry_id);
            put_back(&self.storage, &dir, &stored_pairs(&dir, &files));
            return Err(e);
        }
        Ok(entry_id)
    }

    /// Moves the whole disk folder into the trash, with its node and
    /// everything under it. Returns the entry id.
    pub fn trash_folder(&mut self, id: &Id) -> Result<Id, ProjectError> {
        let node = match self.manifest.folder(id) {
            Some(f) => f.clone(),
            None if self.manifest.parent_of(id).is_some() => {
                return Err(ProjectError::NotAFolder(id.clone()));
            }
            None => return Err(ProjectError::NotFound(id.clone())),
        };
        let path = RelPath::new(&node.path)?;
        let (folder, index) = self
            .manifest
            .parent_of(id)
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        let files = vec![path.clone()];
        let entry = TrashEntry {
            format: FORMAT_VERSION,
            entry_id: Id::new(),
            item_id: id.clone(),
            original_path: path,
            original_folder: folder,
            index,
            deleted_at: now(),
            files: Vec::new(),
            folder_node: Some(node.clone()),
        };
        let entry_id = store_in_trash(&self.storage, &files, entry)?;

        let snap = self.capture_state();
        self.manifest.remove_node(id);
        self.index.retain(|_, p| !is_under(p.as_str(), &node.path));
        if let Err(e) = self.save_manifest() {
            self.restore(snap);
            let dir = entry_dir(&entry_id);
            put_back(&self.storage, &dir, &stored_pairs(&dir, &files));
            return Err(e);
        }
        Ok(entry_id)
    }

    /// Every readable entry, newest first, then by entry id. Entries that
    /// cannot be read or do not check out are left out.
    pub fn list_trash(&self) -> Result<Vec<TrashEntry>, ProjectError> {
        let names = match self.storage.list(&trash_root()) {
            Ok(v) => v,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out: Vec<TrashEntry> = names
            .iter()
            .filter(|e| e.is_dir)
            .filter_map(|e| Id::parse(&e.name).ok())
            .filter_map(|id| load_entry(&self.storage, &id))
            .collect();
        out.sort_by(|a, b| {
            b.deleted_at
                .cmp(&a.deleted_at)
                .then_with(|| a.entry_id.cmp(&b.entry_id))
        });
        Ok(out)
    }

    /// Puts a trashed item or folder back, in its folder and position when
    /// they still exist, else at the top level. Never overwrites.
    pub fn restore_from_trash(&mut self, entry: &Id) -> Result<(), ProjectError> {
        let e = load_entry(&self.storage, entry)
            .ok_or_else(|| ProjectError::NotFound(entry.clone()))?;
        if self.manifest.parent_of(&e.item_id).is_some() {
            return Err(crate::manifest::ManifestError::DuplicateId(e.item_id.clone()).into());
        }
        let parent = e
            .original_folder
            .clone()
            .filter(|f| self.manifest.folder(f).is_some());
        let dir = entry_dir(entry);
        if e.folder_node.is_some() {
            self.restore_folder(&e, parent, &dir)?;
        } else {
            self.restore_item(&e, parent, &dir)?;
        }
        // Done and saved: the entry is spent. Leftovers here cannot lose work.
        let _ = self.storage.remove_file(&dir.join(ENTRY_FILE));
        let _ = self.storage.remove_dir(&dir);
        Ok(())
    }

    fn restore_item(
        &mut self,
        e: &TrashEntry,
        parent: Option<Id>,
        dir: &RelPath,
    ) -> Result<(), ProjectError> {
        let stored = dir.join(&e.files[0]);
        let bytes = self.storage.read(&stored)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| crate::header::HeaderError::Invalid("not valid UTF-8".to_string()))?;
        let item = crate::header::parse_markdown_item(text)?;
        if item.header.id != e.item_id {
            return Err(ProjectError::NotFound(e.item_id.clone()));
        }

        let dest = self.disk_dir(parent.as_ref())?;
        let made_dir = !self.storage.exists(&dest);
        if made_dir {
            self.storage.create_dir_all(&dest)?;
        }
        let entries = match self.entries_of(&dest) {
            Ok(v) => v,
            Err(err) => {
                self.drop_dir(&dest, made_dir);
                return Err(err);
            }
        };
        let original = e.files[0].as_str();
        let (stem, ext) = original.rsplit_once('.').unwrap_or((original, ""));
        let name = self.pick_name(&dest, stem, ext, entries, None, true);
        let new_path = dest.join(&name);
        let moves = self.plan_moves(&stored, &new_path);
        if let Err(err) = self.move_files(&moves) {
            self.drop_dir(&dest, made_dir);
            return Err(err);
        }

        let snap = self.capture_state();
        let saved = self
            .manifest
            .insert_item(
                parent.as_ref(),
                e.item_id.clone(),
                at_index(e, parent.as_ref()),
            )
            .map_err(ProjectError::from)
            .and_then(|()| {
                self.index.insert(e.item_id.clone(), new_path.clone());
                self.save_manifest()
            });
        if let Err(err) = saved {
            self.restore(snap);
            self.undo_moves(&moves);
            self.drop_dir(&dest, made_dir);
            return Err(err);
        }
        Ok(())
    }

    fn restore_folder(
        &mut self,
        e: &TrashEntry,
        parent: Option<Id>,
        dir: &RelPath,
    ) -> Result<(), ProjectError> {
        let Some(node) = e.folder_node.clone() else {
            return Err(ProjectError::NotFound(e.entry_id.clone()));
        };
        // Check every id in the subtree before touching the disk.
        self.manifest.clone().insert_node(
            parent.as_ref(),
            Node::Folder(node.clone()),
            at_index(e, parent.as_ref()),
        )?;

        let stored = dir.join(&e.files[0]);
        let dest = self.disk_dir(parent.as_ref())?;
        let made_dir = !self.storage.exists(&dest);
        if made_dir {
            self.storage.create_dir_all(&dest)?;
        }
        let entries = match self.entries_of(&dest) {
            Ok(v) => v,
            Err(err) => {
                self.drop_dir(&dest, made_dir);
                return Err(err);
            }
        };
        let name = self.pick_name(&dest, &e.files[0], "", entries, None, false);
        let new_path = dest.join(&name);
        let moves = vec![(stored, new_path.clone())];
        if let Err(err) = self.move_files(&moves) {
            self.drop_dir(&dest, made_dir);
            return Err(err);
        }

        let old_path = node.path.clone();
        let mut wrapped = Node::Folder(node);
        repath_folders(
            std::slice::from_mut(&mut wrapped),
            &old_path,
            new_path.as_str(),
        );
        let mut ids = Vec::new();
        item_ids_in(&wrapped, &mut ids);
        let wanted: HashSet<Id> = ids.into_iter().collect();

        let snap = self.capture_state();
        let saved = self
            .manifest
            .insert_node(parent.as_ref(), wrapped, at_index(e, parent.as_ref()))
            .map_err(ProjectError::from)
            .and_then(|()| {
                for item in scan(&self.storage)? {
                    if wanted.contains(&item.id) && is_under(item.path.as_str(), new_path.as_str())
                    {
                        self.index.entry(item.id).or_insert(item.path);
                    }
                }
                self.save_manifest()
            });
        if let Err(err) = saved {
            self.restore(snap);
            self.undo_moves(&moves);
            self.drop_dir(&dest, made_dir);
            return Err(err);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::MANIFEST_FILE;
    use crate::header::*;
    use crate::manifest::{ManifestError, Node};
    use crate::project::*;
    use crate::storage::*;
    use std::cell::Cell;
    use std::path::Path;

    fn top_id<S: Storage>(p: &Project<S>) -> Id {
        match &p.manifest().tree[0] {
            Node::Folder(f) => f.id.clone(),
            _ => unreachable!(),
        }
    }

    fn setup() -> (tempfile::TempDir, Project<FsStorage>, Id) {
        let d = tempfile::tempdir().unwrap();
        let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        (d, p, ms)
    }

    fn read(d: &Path, rel: &str) -> Vec<u8> {
        std::fs::read(d.join(rel)).unwrap()
    }

    fn entry_json(d: &Path, entry: &Id) -> String {
        String::from_utf8(read(d, &format!(".trash/{entry}/entry.json"))).unwrap()
    }

    fn trash_count(d: &Path) -> usize {
        std::fs::read_dir(d.join(".trash")).unwrap().count()
    }

    /// Wraps `FsStorage` and fails chosen calls.
    struct Flaky {
        inner: FsStorage,
        renames: Cell<usize>,
        fail_rename_at: Cell<Option<usize>>,
        fail_manifest: Cell<bool>,
        fail_entry: Cell<bool>,
    }

    impl Flaky {
        fn new(root: &Path) -> Flaky {
            Flaky {
                inner: FsStorage::new(root),
                renames: Cell::new(0),
                fail_rename_at: Cell::new(None),
                fail_manifest: Cell::new(false),
                fail_entry: Cell::new(false),
            }
        }
    }

    impl Storage for Flaky {
        fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError> {
            self.inner.read(p)
        }
        fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError> {
            let hit = (self.fail_manifest.get() && p.as_str() == MANIFEST_FILE)
                || (self.fail_entry.get() && p.file_name() == "entry.json");
            if hit {
                return Err(StorageError::Io(p.clone(), "injected".into()));
            }
            self.inner.write_atomic(p, data)
        }
        fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError> {
            self.inner.list(dir)
        }
        fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError> {
            self.inner.create_dir_all(p)
        }
        fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), StorageError> {
            let n = self.renames.get() + 1;
            self.renames.set(n);
            if self.fail_rename_at.get() == Some(n) {
                return Err(StorageError::Io(from.clone(), "injected".into()));
            }
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
    }

    fn sample_entry(path: &str) -> TrashEntry {
        TrashEntry {
            format: 1,
            entry_id: Id::new(),
            item_id: Id::new(),
            original_path: RelPath::new(path).unwrap(),
            original_folder: None,
            index: 0,
            deleted_at: "2026-01-01T00:00:00Z".into(),
            files: Vec::new(),
            folder_node: None,
        }
    }

    #[test]
    fn entry_json_has_the_spec_keys_in_order_and_no_entry_id() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let entry = p.trash_item(&a).unwrap();
        let text = entry_json(d.path(), &entry);
        assert!(text.ends_with("}\n") && text.starts_with("{\n  \"format\": 1,"));
        let v: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            [
                "format",
                "itemId",
                "originalPath",
                "originalFolder",
                "index",
                "deletedAt",
                "files"
            ]
        );
        assert!(v["deletedAt"].as_str().unwrap().ends_with('Z'));
        assert_eq!(v["deletedAt"].as_str().unwrap().len(), 20);
        assert_eq!(v["originalPath"], "manuscript/a.md");
        assert_eq!(v["originalFolder"], ms.as_str());
        assert_eq!(v["files"], serde_json::json!(["a.md"]));
        assert!(!text.contains(entry.as_str()));
    }

    #[test]
    fn folder_entries_add_folder_node_last_and_hold_the_folder_id() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let entry = p.trash_folder(&part).unwrap();
        let v: serde_json::Value = serde_json::from_str(&entry_json(d.path(), &entry)).unwrap();
        let keys: Vec<_> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys.last(), Some(&"folderNode"));
        assert_eq!(keys.len(), 8);
        assert_eq!(v["itemId"], part.as_str());
        assert_eq!(v["files"], serde_json::json!(["part"]));
        assert_eq!(v["folderNode"]["path"], "manuscript/part");
        let listed = p.list_trash().unwrap();
        assert_eq!(listed[0].entry_id, entry);
        assert!(listed[0].folder_node.is_some());
    }

    #[test]
    fn a_failing_store_leaves_files_in_place_and_no_entry_folder() {
        for (rename_at, entry_write) in [(Some(2), false), (None, true)] {
            let d = tempfile::tempdir().unwrap();
            let p = Project::create(Flaky::new(d.path()), "X").unwrap();
            std::fs::write(d.path().join("manuscript/a.md"), "main").unwrap();
            std::fs::write(d.path().join("manuscript/a.comments.json"), "[]").unwrap();
            let files = [
                RelPath::new("manuscript/a.md").unwrap(),
                RelPath::new("manuscript/a.comments.json").unwrap(),
            ];
            let s = p.storage();
            s.fail_rename_at.set(rename_at);
            s.fail_entry.set(entry_write);
            let r = store_in_trash(s, &files, sample_entry("manuscript/a.md"));
            assert!(matches!(r, Err(ProjectError::Storage(_))), "{r:?}");
            assert_eq!(read(d.path(), "manuscript/a.md"), b"main");
            assert_eq!(read(d.path(), "manuscript/a.comments.json"), b"[]");
            assert_eq!(trash_count(d.path()), 0);
        }
    }

    #[test]
    fn store_refuses_to_overwrite_inside_the_entry_folder() {
        let d = tempfile::tempdir().unwrap();
        let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
        std::fs::write(d.path().join("manuscript/a.md"), "one").unwrap();
        std::fs::write(d.path().join("notes/a.md"), "two").unwrap();
        let files = [
            RelPath::new("manuscript/a.md").unwrap(),
            RelPath::new("notes/a.md").unwrap(),
        ];
        let r = store_in_trash(p.storage(), &files, sample_entry("manuscript/a.md"));
        assert!(matches!(r, Err(ProjectError::Exists(_))), "{r:?}");
        assert_eq!(read(d.path(), "manuscript/a.md"), b"one");
        assert_eq!(read(d.path(), "notes/a.md"), b"two");
        assert_eq!(trash_count(d.path()), 0);
    }

    #[test]
    fn restore_of_an_id_already_in_the_tree_is_a_duplicate_and_changes_nothing() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let text = read(d.path(), "manuscript/a.md");
        let entry = p.trash_item(&a).unwrap();
        // The same item id is back in the tree by another route.
        p.manifest.insert_item(Some(&ms), a.clone(), None).unwrap();
        let before = p.manifest().clone();
        let json = entry_json(d.path(), &entry);
        let r = p.restore_from_trash(&entry);
        assert!(
            matches!(r, Err(ProjectError::Manifest(ManifestError::DuplicateId(ref i))) if *i == a),
            "{r:?}"
        );
        assert_eq!(p.manifest(), &before);
        assert_eq!(entry_json(d.path(), &entry), json);
        assert_eq!(read(d.path(), &format!(".trash/{entry}/a.md")), text);
        assert!(!d.path().join("manuscript/a.md").exists());
    }

    #[test]
    fn restore_refuses_a_file_swapped_for_another_item() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let entry = p.trash_item(&a).unwrap();
        let other = render_markdown_item(&ItemFile {
            header: ItemHeader::new_document("Other", DocKind::Prose),
            body: "other work\n".into(),
        });
        let stored = d.path().join(format!(".trash/{entry}/a.md"));
        std::fs::write(&stored, &other).unwrap();
        let before = p.manifest().clone();
        let r = p.restore_from_trash(&entry);
        assert!(
            matches!(r, Err(ProjectError::NotFound(ref i)) if *i == a),
            "{r:?}"
        );
        assert_eq!(std::fs::read(&stored).unwrap(), other.as_bytes());
        assert!(!d.path().join("manuscript/a.md").exists());
        assert_eq!(p.manifest(), &before);
        assert_eq!(p.list_trash().unwrap().len(), 1);
    }

    #[test]
    fn restore_of_an_unknown_entry_is_not_found() {
        let (_d, mut p, _ms) = setup();
        assert!(matches!(
            p.restore_from_trash(&Id::new()),
            Err(ProjectError::NotFound(_))
        ));
    }

    #[test]
    fn restore_recreates_a_deleted_disk_folder() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
        let entry = p.trash_item(&a).unwrap();
        std::fs::remove_dir(d.path().join("manuscript/part")).unwrap();
        p.restore_from_trash(&entry).unwrap();
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part/a.md");
        assert!(d.path().join("manuscript/part/a.md").is_file());
        assert_eq!(p.manifest().parent_of(&a), Some((Some(part), 0)));
    }

    #[test]
    fn a_restore_clash_keeps_the_original_stem_and_companions_follow() {
        let (d, mut p, ms) = setup();
        let a = p
            .create_document(&ms, "Chapter 01", DocKind::Prose)
            .unwrap();
        std::fs::write(d.path().join("manuscript/chapter-01.comments.json"), "[a]").unwrap();
        std::fs::write(d.path().join("manuscript/chapter-01.md.meta.json"), "{a}").unwrap();
        let entry = p.trash_item(&a).unwrap();
        p.create_document(&ms, "Chapter 01", DocKind::Prose)
            .unwrap();
        p.restore_from_trash(&entry).unwrap();
        assert_eq!(
            p.path_of(&a).unwrap().as_str(),
            "manuscript/chapter-01-2.md"
        );
        assert_eq!(
            read(d.path(), "manuscript/chapter-01-2.comments.json"),
            b"[a]"
        );
        assert_eq!(
            read(d.path(), "manuscript/chapter-01-2.md.meta.json"),
            b"{a}"
        );
        assert!(d.path().join("manuscript/chapter-01.md").is_file());
        assert!(!d.path().join(format!(".trash/{entry}")).exists());
    }

    type Nested = (tempfile::TempDir, Project<FsStorage>, Id, Id, Id, Id, Id);

    /// `manuscript/part` holding `a`, `notes.txt` and a subfolder `inner`
    /// holding `b`.
    fn nested() -> Nested {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let inner = p.create_folder(Some(&part), "Inner").unwrap();
        let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
        let b = p.create_document(&inner, "B", DocKind::Prose).unwrap();
        std::fs::write(
            d.path().join("manuscript/part/notes.txt"),
            b"plain \xff bytes",
        )
        .unwrap();
        (d, p, ms, part, inner, a, b)
    }

    #[test]
    fn folder_trash_and_restore_bring_back_paths_index_and_foreign_files() {
        let (d, mut p, ms, part, inner, a, b) = nested();
        let entry = p.trash_folder(&part).unwrap();
        assert!(!d.path().join("manuscript/part").exists());
        assert!(p.path_of(&a).is_none() && p.path_of(&b).is_none());
        assert!(p.manifest().folder(&part).is_none());
        p.restore_from_trash(&entry).unwrap();
        assert_eq!(p.manifest().folder(&part).unwrap().path, "manuscript/part");
        assert_eq!(
            p.manifest().folder(&inner).unwrap().path,
            "manuscript/part/inner"
        );
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part/a.md");
        assert_eq!(
            p.path_of(&b).unwrap().as_str(),
            "manuscript/part/inner/b.md"
        );
        assert_eq!(
            read(d.path(), "manuscript/part/notes.txt"),
            b"plain \xff bytes"
        );
        assert_eq!(p.manifest().parent_of(&part), Some((Some(ms), 0)));
        assert!(p.list_trash().unwrap().is_empty());
        assert!(!d.path().join(format!(".trash/{entry}")).exists());
        let (reopened, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(reopened.manifest(), p.manifest());
    }

    #[test]
    fn a_folder_restore_whose_name_is_taken_gets_a_suffix_and_updates_nested_paths() {
        let (d, mut p, ms, part, inner, a, b) = nested();
        let entry = p.trash_folder(&part).unwrap();
        let newer = p.create_folder(Some(&ms), "Part").unwrap();
        let c = p.create_document(&newer, "C", DocKind::Prose).unwrap();
        p.restore_from_trash(&entry).unwrap();
        assert_eq!(
            p.manifest().folder(&part).unwrap().path,
            "manuscript/part-2"
        );
        assert_eq!(
            p.manifest().folder(&inner).unwrap().path,
            "manuscript/part-2/inner"
        );
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part-2/a.md");
        assert_eq!(
            p.path_of(&b).unwrap().as_str(),
            "manuscript/part-2/inner/b.md"
        );
        // The folder that took the old name is untouched.
        assert_eq!(p.manifest().folder(&newer).unwrap().path, "manuscript/part");
        assert_eq!(p.path_of(&c).unwrap().as_str(), "manuscript/part/c.md");
        assert!(d.path().join("manuscript/part-2/notes.txt").is_file());
    }

    #[test]
    fn list_trash_is_newest_first_and_skips_unreadable_entries() {
        let (d, p, _ms) = setup();
        let s = p.storage();
        for (name, when) in [
            ("old.md", "2026-01-01T00:00:00Z"),
            ("new.md", "2026-02-01T00:00:00Z"),
        ] {
            std::fs::write(d.path().join(name), "x").unwrap();
            let mut e = sample_entry(name);
            e.deleted_at = when.into();
            store_in_trash(s, &[RelPath::new(name).unwrap()], e).unwrap();
        }
        std::fs::create_dir(d.path().join(".trash/01ARZ3NDEKTSV4RRFFQ69G5FAV")).unwrap();
        std::fs::create_dir(d.path().join(".trash/not-an-id")).unwrap();
        let bad = Id::new();
        std::fs::create_dir(d.path().join(format!(".trash/{bad}"))).unwrap();
        std::fs::write(d.path().join(format!(".trash/{bad}/entry.json")), "{nope").unwrap();
        let listed = p.list_trash().unwrap();
        let names: Vec<_> = listed.iter().map(|e| e.original_path.as_str()).collect();
        assert_eq!(names, ["new.md", "old.md"]);
    }

    #[test]
    fn a_failed_manifest_save_undoes_trash_item() {
        let d = tempfile::tempdir().unwrap();
        let mut p = Project::create(Flaky::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        std::fs::write(d.path().join("manuscript/a.comments.json"), "[]").unwrap();
        let before = p.manifest().clone();
        let bytes = read(d.path(), "manuscript/a.md");
        p.storage().fail_manifest.set(true);
        assert!(p.trash_item(&a).is_err());
        assert_eq!(read(d.path(), "manuscript/a.md"), bytes);
        assert_eq!(read(d.path(), "manuscript/a.comments.json"), b"[]");
        assert_eq!(p.manifest(), &before);
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/a.md");
        assert!(p.list_trash().unwrap().is_empty());
        assert_eq!(trash_count(d.path()), 0);
    }

    #[test]
    fn a_failed_manifest_save_undoes_trash_folder() {
        let d = tempfile::tempdir().unwrap();
        let mut p = Project::create(Flaky::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
        let before = p.manifest().clone();
        p.storage().fail_manifest.set(true);
        assert!(p.trash_folder(&part).is_err());
        assert!(d.path().join("manuscript/part/a.md").is_file());
        assert_eq!(p.manifest(), &before);
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part/a.md");
        assert_eq!(trash_count(d.path()), 0);
    }

    #[test]
    fn a_failed_manifest_save_undoes_restore() {
        let d = tempfile::tempdir().unwrap();
        let mut p = Project::create(Flaky::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let b = p.create_document(&part, "B", DocKind::Prose).unwrap();
        let item_entry = p.trash_item(&a).unwrap();
        let folder_entry = p.trash_folder(&part).unwrap();
        let before = p.manifest().clone();
        let json = entry_json(d.path(), &item_entry);
        p.storage().fail_manifest.set(true);

        assert!(p.restore_from_trash(&item_entry).is_err());
        assert!(!d.path().join("manuscript/a.md").exists());
        assert!(d.path().join(format!(".trash/{item_entry}/a.md")).is_file());
        assert_eq!(entry_json(d.path(), &item_entry), json);
        assert_eq!(p.manifest(), &before);
        assert!(p.path_of(&a).is_none());

        assert!(p.restore_from_trash(&folder_entry).is_err());
        assert!(!d.path().join("manuscript/part").exists());
        assert!(
            d.path()
                .join(format!(".trash/{folder_entry}/part/b.md"))
                .is_file()
        );
        assert_eq!(p.manifest(), &before);
        assert!(p.path_of(&b).is_none());
        assert_eq!(p.list_trash().unwrap().len(), 2);
    }

    #[test]
    fn an_item_whose_folder_is_gone_is_appended_as_the_last_top_level_node() {
        let (_d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
        let entry = p.trash_item(&a).unwrap();
        p.trash_folder(&part).unwrap();
        p.restore_from_trash(&entry).unwrap();
        let last = p.manifest().tree.len() - 1;
        assert_eq!(p.manifest().parent_of(&a), Some((None, last)));
        assert!(last > 0 && p.manifest().tree[0].id() == &ms);
    }

    #[test]
    fn a_folder_whose_parent_is_gone_is_appended_as_the_last_top_level_node() {
        let (d, mut p, ms, part, inner, a, b) = nested();
        let entry = p.trash_folder(&part).unwrap();
        p.create_folder(None, "Other").unwrap();
        p.trash_folder(&ms).unwrap();
        p.restore_from_trash(&entry).unwrap();
        let last = p.manifest().tree.len() - 1;
        assert_eq!(p.manifest().parent_of(&part), Some((None, last)));
        assert!(last > 0);
        assert_eq!(p.manifest().folder(&part).unwrap().path, "part");
        assert_eq!(p.manifest().folder(&inner).unwrap().path, "part/inner");
        assert_eq!(p.path_of(&a).unwrap().as_str(), "part/a.md");
        assert_eq!(p.path_of(&b).unwrap().as_str(), "part/inner/b.md");
        assert!(d.path().join("part/notes.txt").is_file());
    }

    #[test]
    fn a_top_level_item_keeps_its_stored_index() {
        let (_d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.move_item(&a, None, 0).unwrap();
        assert_eq!(p.manifest().parent_of(&a), Some((None, 0)));
        let entry = p.trash_item(&a).unwrap();
        p.restore_from_trash(&entry).unwrap();
        assert_eq!(p.manifest().parent_of(&a), Some((None, 0)));
        assert_eq!(p.manifest().tree[1].id(), &ms);
    }
}
