// SPDX-License-Identifier: MPL-2.0
//! Reconcile rules run on every project open (spec section 4): files and the
//! manifest are brought back into agreement without ever losing a writer's text.

use std::collections::{HashMap, HashSet};

use chrono::{SecondsFormat, Utc};
use serde_json::Map;

use crate::header::{HeaderError, parse_markdown_item, render_markdown_item};
use crate::id::Id;
use crate::items::{companions_of, same_path};
use crate::manifest::{FolderNode, Manifest, Node};
use crate::project::{ProjectError, ReconcileReport, ScannedItem};
use crate::storage::{RelPath, Storage};
use crate::text::normalize_text;
use crate::trash::{TrashEntry, store_in_trash};
use crate::version::FORMAT_VERSION;

/// True when the file name looks like a sync tool's conflict copy.
pub fn is_sync_conflict_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains(".sync-conflict-") || lower.contains("conflicted copy")
}

/// An item to add to the tree: its file, its id, and whether it is reported
/// as found (a reassigned copy is added the same way but not reported so).
struct Addition {
    path: RelPath,
    id: Id,
    found: bool,
}

/// Brings the manifest and the files into agreement (spec section 4).
/// Returns the id-to-path index and the report. Files change only for
/// identical duplicates (moved to the trash) and different duplicates
/// (rewritten with a new id); the manifest is changed in memory only.
pub fn reconcile<S: Storage>(
    storage: &S,
    manifest: &mut Manifest,
    mut scanned: Vec<ScannedItem>,
) -> Result<(HashMap<Id, RelPath>, ReconcileReport), ProjectError> {
    let known: HashSet<Id> = manifest.item_ids().into_iter().collect();
    let folder_ids: HashSet<Id> = all_ids(&manifest.tree)
        .into_iter()
        .filter(|id| !known.contains(id))
        .collect();
    let mut report = ReconcileReport::default();
    let mut index = HashMap::new();
    let mut additions = Vec::new();

    let mut order: Vec<Id> = Vec::new();
    let mut groups: HashMap<Id, Vec<ScannedItem>> = HashMap::new();
    scanned.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    for item in scanned {
        if !groups.contains_key(&item.id) {
            order.push(item.id.clone());
        }
        groups.entry(item.id.clone()).or_default().push(item);
    }

    for id in order {
        let group = groups.remove(&id).unwrap_or_default();
        if folder_ids.contains(&id) {
            // A file carrying a folder's id is never placed in the tree and
            // never changed: it is reported for the writer to sort out.
            let mut all: Vec<RelPath> = group.into_iter().map(|g| g.path).collect();
            all.sort_by(|a, b| a.as_str().cmp(b.as_str()));
            report.conflicts.push((id, all));
            continue;
        }
        let keeper = resolve_group(storage, manifest, &id, group, &mut report, &mut additions);
        if !known.contains(&id) {
            additions.push(Addition {
                path: keeper.clone(),
                id: id.clone(),
                found: true,
            });
        }
        index.insert(id, keeper);
    }

    additions.sort_by(|a, b| a.path.as_str().cmp(b.path.as_str()));
    for add in additions {
        // Placing one file can fail without failing the open: the file is
        // left where it is and reported (R67).
        let dir = add.path.parent().unwrap_or_else(RelPath::root);
        let placed = folder_for(manifest, dir.as_str()).and_then(|folder| {
            manifest
                .insert_item(folder.as_ref(), add.id.clone(), None)
                .map_err(ProjectError::from)
        });
        if placed.is_err() {
            index.remove(&add.id);
            report.conflicts.push((add.id, vec![add.path]));
            continue;
        }
        index.insert(add.id.clone(), add.path);
        if add.found {
            report.found.push(add.id);
        }
    }

    report.missing = manifest
        .item_ids()
        .into_iter()
        .filter(|id| !index.contains_key(id))
        .collect();
    Ok((index, report))
}

/// Decides what to do with every file that shares `id` and returns the path
/// that keeps the id. Anything that goes wrong turns the group into a
/// reported conflict, leaving the remaining files as they are.
fn resolve_group<S: Storage>(
    storage: &S,
    manifest: &Manifest,
    id: &Id,
    mut group: Vec<ScannedItem>,
    report: &mut ReconcileReport,
    additions: &mut Vec<Addition>,
) -> RelPath {
    group.sort_by(|a, b| {
        a.modified
            .cmp(&b.modified)
            .then_with(|| a.path.as_str().cmp(b.path.as_str()))
    });
    let oldest = group[0].path.clone();
    if group.len() == 1 {
        return oldest;
    }
    let mut all_paths: Vec<RelPath> = group.iter().map(|g| g.path.clone()).collect();
    all_paths.sort_by(|a, b| a.as_str().cmp(b.as_str()));

    if group
        .iter()
        .any(|g| is_sync_conflict_name(g.path.file_name()))
    {
        let keeper = group
            .iter()
            .find(|g| !is_sync_conflict_name(g.path.file_name()))
            .map_or(oldest, |g| g.path.clone());
        report.conflicts.push((id.clone(), all_paths));
        return keeper;
    }

    for newer in &group[1..] {
        if resolve_newer(
            storage,
            manifest,
            id,
            &oldest,
            &newer.path,
            report,
            additions,
        )
        .is_err()
        {
            report.conflicts.push((id.clone(), all_paths));
            break;
        }
    }
    oldest
}

/// Handles one file that is newer than the id's keeper: trash it when it is
/// byte-identical, otherwise give it a fresh id.
fn resolve_newer<S: Storage>(
    storage: &S,
    manifest: &Manifest,
    id: &Id,
    oldest: &RelPath,
    newer: &RelPath,
    report: &mut ReconcileReport,
    additions: &mut Vec<Addition>,
) -> Result<(), ProjectError> {
    let bytes = storage.read(newer)?;
    if bytes == storage.read(oldest)? {
        let mut files = vec![newer.clone()];
        files.extend(
            companions_of(newer)
                .into_iter()
                .filter(|c| storage.exists(c)),
        );
        let (original_folder, index) = manifest.parent_of(id).unwrap_or((None, 0));
        let entry = TrashEntry {
            format: FORMAT_VERSION,
            entry_id: Id::new(),
            item_id: id.clone(),
            original_path: newer.clone(),
            original_folder,
            index,
            deleted_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            files: Vec::new(),
            folder_node: None,
        };
        store_in_trash(storage, &files, entry)?;
        report.duplicates_trashed.push(newer.clone());
    } else {
        let text = String::from_utf8(bytes)
            .map_err(|_| HeaderError::Invalid("the file is not valid UTF-8".to_string()))?;
        let mut item = parse_markdown_item(&text)?;
        let new_id = Id::new();
        item.header.id = new_id.clone();
        item.header.title = format!("{} (copy)", item.header.title);
        storage.write_atomic(newer, render_markdown_item(&item).as_bytes())?;
        report
            .copies_reassigned
            .push((newer.clone(), new_id.clone()));
        additions.push(Addition {
            path: newer.clone(),
            id: new_id,
            found: false,
        });
    }
    Ok(())
}

/// Every id in the tree, folders and items.
fn all_ids(nodes: &[Node]) -> Vec<Id> {
    let mut out = Vec::new();
    for n in nodes {
        out.push(n.id().clone());
        if let Node::Folder(f) = n {
            out.extend(all_ids(&f.children));
        }
    }
    out
}

/// The id of the folder whose path is `path`, compared in NFC (R65).
fn folder_by_same_path(nodes: &[Node], path: &str) -> Option<Id> {
    nodes.iter().find_map(|n| match n {
        Node::Folder(f) if same_path(&f.path, path) => Some(f.id.clone()),
        Node::Folder(f) => folder_by_same_path(&f.children, path),
        Node::Item { .. } => None,
    })
}

/// The folder node for a directory, creating a node for each missing level
/// under the nearest existing ancestor (the top level if there is none).
fn folder_for(manifest: &mut Manifest, dir: &str) -> Result<Option<Id>, ProjectError> {
    let mut parent: Option<Id> = None;
    let mut prefix = String::new();
    for segment in dir.split('/').filter(|s| !s.is_empty()) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(segment);
        let existing = folder_by_same_path(&manifest.tree, &prefix);
        let id = match existing {
            Some(id) => id,
            None => {
                let id = Id::new();
                manifest.insert_node(
                    parent.as_ref(),
                    Node::Folder(FolderNode {
                        id: id.clone(),
                        title: normalize_text(segment),
                        path: prefix.clone(),
                        synopsis: String::new(),
                        children: Vec::new(),
                        extra: Map::new(),
                    }),
                    None,
                )?;
                id
            }
        };
        parent = Some(id);
    }
    Ok(parent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::{DocKind, ItemFile, ItemHeader, parse_markdown_item, render_markdown_item};
    use crate::manifest::Node;
    use crate::project::Project;
    use crate::storage::{Entry, FsStorage, StorageError};
    use std::path::Path;
    use std::time::{Duration, SystemTime};

    fn new_project() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        Project::create(FsStorage::new(d.path()), "X").unwrap();
        d
    }

    fn header(title: &str) -> ItemHeader {
        ItemHeader::new_document(title, DocKind::Prose)
    }

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    fn put_item(root: &Path, rel: &str, header: &ItemHeader, body: &str) {
        let text = render_markdown_item(&ItemFile {
            header: header.clone(),
            body: body.into(),
        });
        put(root, rel, text.as_bytes());
    }

    /// Sets the modified time to a fixed point, `secs` after a base time.
    fn at(root: &Path, rel: &str, secs: u64) {
        let f = std::fs::OpenOptions::new()
            .write(true)
            .open(root.join(rel))
            .unwrap();
        f.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000 + secs))
            .unwrap();
    }

    fn read(root: &Path, rel: &str) -> Vec<u8> {
        std::fs::read(root.join(rel)).unwrap()
    }

    type Files = Vec<(String, Vec<u8>)>;

    fn snapshot(root: &Path) -> Files {
        fn walk(dir: &Path, root: &Path, out: &mut Files) {
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

    fn without_manifest(s: Files) -> Files {
        s.into_iter()
            .filter(|(n, _)| n != crate::brand::MANIFEST_FILE)
            .collect()
    }

    fn open(d: &tempfile::TempDir) -> (Project<FsStorage>, ReconcileReport) {
        Project::open(FsStorage::new(d.path())).unwrap()
    }

    fn paths(v: &[RelPath]) -> Vec<&str> {
        v.iter().map(|p| p.as_str()).collect()
    }

    #[test]
    fn three_identical_copies_keep_the_oldest_and_trash_two_with_companions() {
        let d = new_project();
        let h = header("A");
        for (rel, t) in [
            ("manuscript/a.md", 10),
            ("manuscript/a-2.md", 20),
            ("manuscript/a-3.md", 30),
        ] {
            put_item(d.path(), rel, &h, "same\n");
            at(d.path(), rel, t);
        }
        put(d.path(), "manuscript/a-2.comments.json", b"[2]");
        put(d.path(), "manuscript/a-3.comments.json", b"[3]");
        let (p, report) = open(&d);
        assert_eq!(
            paths(&report.duplicates_trashed),
            vec!["manuscript/a-2.md", "manuscript/a-3.md"]
        );
        assert!(d.path().join("manuscript/a.md").is_file());
        for gone in ["a-2.md", "a-3.md", "a-2.comments.json", "a-3.comments.json"] {
            assert!(!d.path().join("manuscript").join(gone).exists(), "{gone}");
        }
        assert_eq!(p.path_of(&h.id).unwrap().as_str(), "manuscript/a.md");
        let entries = p.list_trash().unwrap();
        assert_eq!(entries.len(), 2);
        for e in &entries {
            assert_eq!(e.item_id, h.id);
            assert_eq!(e.files.len(), 2);
            assert!(e.files[1].ends_with(".comments.json"));
        }
        let mut originals: Vec<&str> = entries.iter().map(|e| e.original_path.as_str()).collect();
        originals.sort();
        assert_eq!(originals, vec!["manuscript/a-2.md", "manuscript/a-3.md"]);
        assert_eq!(report.found, vec![h.id.clone()]);
    }

    #[test]
    fn two_different_copies_are_both_reassigned_and_keep_unknown_keys() {
        let d = new_project();
        let mut h = header("A");
        h.extra
            .insert("custom".into(), serde_norway::Value::String("keep".into()));
        put_item(d.path(), "manuscript/a.md", &h, "one\n");
        put_item(d.path(), "manuscript/b.md", &h, "two\n");
        put_item(d.path(), "manuscript/c.md", &h, "three\n");
        at(d.path(), "manuscript/a.md", 10);
        at(d.path(), "manuscript/b.md", 20);
        at(d.path(), "manuscript/c.md", 30);
        let oldest = read(d.path(), "manuscript/a.md");
        let (p, report) = open(&d);
        assert_eq!(read(d.path(), "manuscript/a.md"), oldest);
        assert_eq!(report.copies_reassigned.len(), 2);
        assert_eq!(report.copies_reassigned[0].0.as_str(), "manuscript/b.md");
        assert_eq!(report.copies_reassigned[1].0.as_str(), "manuscript/c.md");
        let (id_b, id_c) = (
            report.copies_reassigned[0].1.clone(),
            report.copies_reassigned[1].1.clone(),
        );
        assert_ne!(id_b, id_c);
        assert_ne!(id_b, h.id);
        assert_ne!(id_c, h.id);
        for (rel, id, body) in [
            ("manuscript/b.md", &id_b, "two\n"),
            ("manuscript/c.md", &id_c, "three\n"),
        ] {
            let text = String::from_utf8(read(d.path(), rel)).unwrap();
            let item = parse_markdown_item(&text).unwrap();
            assert_eq!(&item.header.id, id);
            assert_eq!(item.header.title, "A (copy)");
            assert_eq!(item.body, body);
            assert!(text.contains("custom: keep"));
            assert_eq!(p.path_of(id).unwrap().as_str(), rel);
            assert!(p.manifest().parent_of(id).is_some());
        }
        assert_eq!(report.found, vec![h.id.clone()]);
    }

    #[test]
    fn a_group_mixing_identical_and_different_copies() {
        let d = new_project();
        let h = header("A");
        put_item(d.path(), "manuscript/a.md", &h, "same\n");
        put_item(d.path(), "manuscript/b.md", &h, "same\n");
        put_item(d.path(), "manuscript/c.md", &h, "other\n");
        at(d.path(), "manuscript/a.md", 10);
        at(d.path(), "manuscript/b.md", 20);
        at(d.path(), "manuscript/c.md", 30);
        let (p, report) = open(&d);
        assert_eq!(paths(&report.duplicates_trashed), vec!["manuscript/b.md"]);
        assert_eq!(report.copies_reassigned.len(), 1);
        assert_eq!(report.copies_reassigned[0].0.as_str(), "manuscript/c.md");
        assert!(!d.path().join("manuscript/b.md").exists());
        assert_eq!(p.path_of(&h.id).unwrap().as_str(), "manuscript/a.md");
        assert!(report.conflicts.is_empty());
    }

    #[test]
    fn a_sync_conflict_group_changes_nothing_and_indexes_the_oldest_non_conflict_file() {
        let d = new_project();
        let h = header("A");
        let conflict = "manuscript/a.sync-conflict-20261106-120000-ABCDEFG.md";
        put_item(d.path(), conflict, &h, "theirs\n");
        put_item(d.path(), "manuscript/a.md", &h, "mine\n");
        put_item(d.path(), "manuscript/z.md", &h, "other\n");
        at(d.path(), conflict, 5);
        at(d.path(), "manuscript/a.md", 10);
        at(d.path(), "manuscript/z.md", 20);
        let before = without_manifest(snapshot(d.path()));
        let (p, report) = open(&d);
        assert_eq!(without_manifest(snapshot(d.path())), before);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].0, h.id);
        assert_eq!(
            paths(&report.conflicts[0].1),
            vec!["manuscript/a.md", conflict, "manuscript/z.md"]
        );
        assert_eq!(p.path_of(&h.id).unwrap().as_str(), "manuscript/a.md");
        assert!(report.duplicates_trashed.is_empty());
        assert!(report.copies_reassigned.is_empty());
    }

    #[test]
    fn a_group_of_only_conflict_names_indexes_the_oldest() {
        let d = new_project();
        let h = header("A");
        put_item(d.path(), "manuscript/x conflicted copy.md", &h, "1\n");
        put_item(d.path(), "manuscript/y.SYNC-CONFLICT-1.md", &h, "2\n");
        at(d.path(), "manuscript/x conflicted copy.md", 20);
        at(d.path(), "manuscript/y.SYNC-CONFLICT-1.md", 10);
        let (p, report) = open(&d);
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(
            p.path_of(&h.id).unwrap().as_str(),
            "manuscript/y.SYNC-CONFLICT-1.md"
        );
    }

    #[test]
    fn equal_modified_times_are_resolved_by_path_order() {
        let d = new_project();
        let h = header("A");
        put_item(d.path(), "manuscript/b.md", &h, "two\n");
        put_item(d.path(), "manuscript/a.md", &h, "one\n");
        at(d.path(), "manuscript/a.md", 10);
        at(d.path(), "manuscript/b.md", 10);
        let (p, report) = open(&d);
        assert_eq!(p.path_of(&h.id).unwrap().as_str(), "manuscript/a.md");
        assert_eq!(report.copies_reassigned[0].0.as_str(), "manuscript/b.md");
    }

    #[test]
    fn found_items_create_nested_folders_and_go_in_path_order() {
        let d = new_project();
        let (hx, hw, hr) = (header("X"), header("W"), header("R"));
        put_item(d.path(), "manuscript/a/b/x.md", &hx, "x\n");
        put_item(d.path(), "manuscript/a/b/w.md", &hw, "w\n");
        put_item(d.path(), "root.md", &hr, "r\n");
        let (p, report) = open(&d);
        assert_eq!(
            report.found,
            vec![hw.id.clone(), hx.id.clone(), hr.id.clone()]
        );
        let m = p.manifest();
        let ms = m.folder_by_path("manuscript").unwrap();
        let a = m.folder_by_path("manuscript/a").unwrap();
        let b = m.folder_by_path("manuscript/a/b").unwrap();
        assert_eq!((a.title.as_str(), b.title.as_str()), ("a", "b"));
        assert!(a.synopsis.is_empty() && a.children.len() == 1 && a.extra.is_empty());
        assert!(matches!(ms.children.last(), Some(Node::Folder(f)) if f.id == a.id));
        assert!(matches!(&a.children[0], Node::Folder(f) if f.id == b.id));
        let ids: Vec<&Id> = b.children.iter().map(|n| n.id()).collect();
        assert_eq!(ids, vec![&hw.id, &hx.id]);
        assert!(matches!(m.tree.last(), Some(Node::Item { id, .. }) if *id == hr.id));
        assert_eq!(m.parent_of(&hr.id).unwrap().0, None);
    }

    #[test]
    fn found_folder_without_any_tree_ancestor_goes_to_the_top_level() {
        let d = new_project();
        let h = header("X");
        put_item(d.path(), "elsewhere/deep/x.md", &h, "x\n");
        let (p, _) = open(&d);
        let m = p.manifest();
        let e = m.folder_by_path("elsewhere").unwrap();
        assert!(m.tree.iter().any(|n| n.id() == &e.id));
        assert_eq!(
            m.parent_of(&h.id).unwrap().0,
            Some(m.folder_by_path("elsewhere/deep").unwrap().id.clone())
        );
    }

    #[test]
    fn plain_markdown_is_never_touched_even_when_reconcile_changes_things() {
        let d = new_project();
        put(d.path(), "manuscript/README.md", b"Plain\r\ntext\r\n");
        put(d.path(), "notes/no-header.md", b"# Title\n\nbody\n");
        let h = header("A");
        put_item(d.path(), "manuscript/a.md", &h, "same\n");
        put_item(d.path(), "manuscript/a-2.md", &h, "same\n");
        put_item(d.path(), "manuscript/found.md", &header("F"), "f\n");
        at(d.path(), "manuscript/a.md", 1);
        at(d.path(), "manuscript/a-2.md", 2);
        let r = read(d.path(), "manuscript/README.md");
        let n = read(d.path(), "notes/no-header.md");
        let (_, report) = open(&d);
        assert_eq!(report.duplicates_trashed.len(), 1);
        assert_eq!(report.found.len(), 2);
        assert_eq!(read(d.path(), "manuscript/README.md"), r);
        assert_eq!(read(d.path(), "notes/no-header.md"), n);
    }

    /// Storage whose renames always fail, so trashing a duplicate fails.
    struct NoRename(FsStorage);

    impl Storage for NoRename {
        fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError> {
            self.0.read(p)
        }
        fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError> {
            self.0.write_atomic(p, data)
        }
        fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError> {
            self.0.list(dir)
        }
        fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError> {
            self.0.create_dir_all(p)
        }
        fn rename(&self, from: &RelPath, _to: &RelPath) -> Result<(), StorageError> {
            Err(StorageError::Io(from.clone(), "injected".into()))
        }
        fn remove_file(&self, p: &RelPath) -> Result<(), StorageError> {
            self.0.remove_file(p)
        }
        fn remove_dir(&self, p: &RelPath) -> Result<(), StorageError> {
            self.0.remove_dir(p)
        }
        fn exists(&self, p: &RelPath) -> bool {
            self.0.exists(p)
        }
    }

    #[test]
    fn a_failed_trash_reports_a_conflict_and_the_open_succeeds() {
        let d = new_project();
        let h = header("A");
        put_item(d.path(), "manuscript/a.md", &h, "same\n");
        put_item(d.path(), "manuscript/a-copy.md", &h, "same\n");
        at(d.path(), "manuscript/a.md", 1);
        at(d.path(), "manuscript/a-copy.md", 2);
        let before = without_manifest(snapshot(d.path()));
        let (p, report) = Project::open(NoRename(FsStorage::new(d.path()))).unwrap();
        assert_eq!(without_manifest(snapshot(d.path())), before);
        assert!(report.duplicates_trashed.is_empty());
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].0, h.id);
        assert_eq!(
            paths(&report.conflicts[0].1),
            vec!["manuscript/a-copy.md", "manuscript/a.md"]
        );
        assert_eq!(p.path_of(&h.id).unwrap().as_str(), "manuscript/a.md");
    }

    #[test]
    fn opening_a_consistent_project_twice_writes_nothing_the_second_time() {
        let d = new_project();
        put_item(d.path(), "manuscript/part/x.md", &header("X"), "x\n");
        let (_, first) = open(&d);
        assert_eq!(first.found.len(), 1);
        let manifest = d.path().join(crate::brand::MANIFEST_FILE);
        let bytes = std::fs::read(&manifest).unwrap();
        let mtime = std::fs::metadata(&manifest).unwrap().modified().unwrap();
        let files = snapshot(d.path());
        let (_, second) = open(&d);
        assert!(second.found.is_empty() && second.missing.is_empty());
        assert_eq!(std::fs::read(&manifest).unwrap(), bytes);
        assert_eq!(
            std::fs::metadata(&manifest).unwrap().modified().unwrap(),
            mtime
        );
        assert_eq!(snapshot(d.path()), files);
    }

    #[test]
    fn missing_ids_come_out_in_tree_order() {
        let d = new_project();
        let (x, y, z) = {
            let (mut p, _) = open(&d);
            let ms = p
                .manifest()
                .folder_by_path("manuscript")
                .unwrap()
                .id
                .clone();
            let x = p.create_document(&ms, "X", DocKind::Prose).unwrap();
            let y = p.create_document(&ms, "Y", DocKind::Prose).unwrap();
            let z = p.create_document(&ms, "Z", DocKind::Prose).unwrap();
            p.manifest.remove_node(&z);
            p.manifest
                .insert_item(Some(&ms), z.clone(), Some(0))
                .unwrap();
            p.save_manifest().unwrap();
            (x, y, z)
        };
        for f in ["x.md", "y.md", "z.md"] {
            std::fs::remove_file(d.path().join("manuscript").join(f)).unwrap();
        }
        let (p, report) = open(&d);
        assert_eq!(report.missing, vec![z.clone(), x.clone(), y.clone()]);
        assert_eq!(p.manifest().item_ids(), vec![z, x, y]);
    }

    #[test]
    fn a_found_item_in_an_nfd_named_directory_joins_the_existing_folder() {
        use unicode_normalization::UnicodeNormalization;
        let d = new_project();
        let f = {
            let (mut p, _) = open(&d);
            let ms = p
                .manifest()
                .folder_by_path("manuscript")
                .unwrap()
                .id
                .clone();
            p.create_folder(Some(&ms), "\u{39b}\u{3cc}\u{3b3}\u{3bf}\u{3c2}")
                .unwrap()
        };
        let nfc = {
            let (p, _) = open(&d);
            p.manifest().folder(&f).unwrap().path.clone()
        };
        let nfd: String = nfc.nfd().collect();
        assert_ne!(nfc, nfd);
        std::fs::remove_dir(d.path().join(&nfc)).unwrap();
        let h = header("X");
        put_item(d.path(), &format!("{nfd}/x.md"), &h, "x\n");
        let (p, report) = open(&d);
        assert_eq!(report.found, vec![h.id.clone()]);
        assert_eq!(p.manifest().parent_of(&h.id).unwrap().0, Some(f.clone()));
        let ms = p.manifest().folder_by_path("manuscript").unwrap();
        let folders = ms
            .children
            .iter()
            .filter(|n| matches!(n, Node::Folder(_)))
            .count();
        assert_eq!(folders, 1, "no twin folder node");
    }

    #[test]
    fn a_file_whose_id_is_a_folder_id_is_a_conflict_and_the_open_succeeds() {
        let d = new_project();
        let ms = {
            let (p, _) = open(&d);
            p.manifest()
                .folder_by_path("manuscript")
                .unwrap()
                .id
                .clone()
        };
        let mut h = header("Odd");
        h.id = ms.clone();
        put_item(d.path(), "notes/odd.md", &h, "precious\n");
        let bytes = read(d.path(), "notes/odd.md");
        let manifest = read(d.path(), crate::brand::MANIFEST_FILE);
        let (p, report) = Project::open(FsStorage::new(d.path())).expect("the open succeeds");
        assert_eq!(report.conflicts.len(), 1);
        assert_eq!(report.conflicts[0].0, ms);
        assert_eq!(paths(&report.conflicts[0].1), vec!["notes/odd.md"]);
        assert!(report.found.is_empty());
        assert!(p.path_of(&ms).is_none());
        assert_eq!(read(d.path(), "notes/odd.md"), bytes);
        assert_eq!(read(d.path(), crate::brand::MANIFEST_FILE), manifest);
    }

    #[test]
    fn sync_conflict_names() {
        assert!(is_sync_conflict_name(
            "chapter.sync-conflict-20261106-120000-ABCDEFG.md"
        ));
        assert!(is_sync_conflict_name(
            "Chapter (Gon\u{e7}alo's Conflicted Copy 2026-11-06).md"
        ));
        assert!(is_sync_conflict_name("a.SYNC-CONFLICT-1.md"));
        assert!(!is_sync_conflict_name("chapter-one.md"));
        assert!(!is_sync_conflict_name("sync-conflict.md"));
    }
}
