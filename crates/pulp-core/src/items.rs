// SPDX-License-Identifier: MPL-2.0
//! Item and folder operations.

use std::collections::HashMap;

use unicode_normalization::UnicodeNormalization;

use crate::brand::MANIFEST_FILE;
use crate::filename::{slugify, unique_name};
use crate::header::{
    DocKind, HeaderError, ItemFile, ItemHeader, parse_markdown_item, render_markdown_item,
};
use crate::id::Id;
use crate::manifest::{FolderNode, Manifest, Node};
use crate::project::{Project, ProjectError};
use crate::storage::{ErrorCause, RelPath, Storage};
use crate::text::normalize_text;
use crate::version::check_readable;

type Moves = Vec<(RelPath, RelPath)>;
type Snapshot = (Manifest, HashMap<Id, RelPath>);

/// The files that can sit next to a document and travel with it:
/// `<stem>.comments.json` and `<file>.meta.json`. Callers keep only those
/// that exist.
pub fn companions_of(path: &RelPath) -> Vec<RelPath> {
    let dir = path.parent().unwrap_or_else(RelPath::root);
    let file = path.file_name();
    let stem = file.rsplit_once('.').map_or(file, |(s, _)| s);
    vec![
        dir.join(&format!("{stem}.comments.json")),
        dir.join(&format!("{file}.meta.json")),
    ]
}

/// Case-insensitive, NFC form used to compare names.
fn fold(name: &str) -> String {
    name.nfc().collect::<String>().to_uppercase().to_lowercase()
}

/// `p` with the whole-segment prefix `old` swapped for `new`, if it has it.
fn reprefix(p: &str, old: &str, new: &str) -> Option<String> {
    if p == old {
        return Some(new.to_string());
    }
    let rest = p.strip_prefix(old)?.strip_prefix('/')?;
    Some(format!("{new}/{rest}"))
}

fn repath_folders(nodes: &mut [Node], old: &str, new: &str) {
    for n in nodes {
        if let Node::Folder(f) = n {
            if let Some(p) = reprefix(&f.path, old, new) {
                f.path = p;
            }
            repath_folders(&mut f.children, old, new);
        }
    }
}

impl<S: Storage> Project<S> {
    /// Reads the item and checks that the indexed file still holds it.
    pub fn read_item(&self, id: &Id) -> Result<ItemFile, ProjectError> {
        Ok(self.read_verified(id)?.1)
    }

    /// Creates an empty document in `folder`. The file is named after the
    /// title and never replaces anything already on disk.
    pub fn create_document(
        &mut self,
        folder: &Id,
        title: &str,
        kind: DocKind,
    ) -> Result<Id, ProjectError> {
        let dir = self.disk_dir(Some(folder))?;
        let title = normalize_text(title);
        let made_dir = !self.storage.exists(&dir);
        if made_dir {
            self.storage.create_dir_all(&dir)?;
        }
        let entries = match self.entries_of(&dir) {
            Ok(e) => e,
            Err(e) => {
                self.drop_dir(&dir, made_dir);
                return Err(e);
            }
        };
        let name = self.pick_name(&dir, &slugify(&title), "md", entries, None, true);
        let path = dir.join(&name);
        if self.storage.exists(&path) {
            self.drop_dir(&dir, made_dir);
            return Err(ProjectError::Exists(path));
        }
        let header = ItemHeader::new_document(&title, kind);
        let id = header.id.clone();
        let text = render_markdown_item(&ItemFile {
            header,
            body: String::new(),
        });
        if let Err(e) = self.storage.write_atomic(&path, text.as_bytes()) {
            self.drop_dir(&dir, made_dir);
            return Err(e.into());
        }
        let snap = self.snapshot();
        let saved = self
            .manifest
            .insert_item(Some(folder), id.clone(), None)
            .map_err(ProjectError::from)
            .and_then(|()| {
                self.index.insert(id.clone(), path.clone());
                self.save_manifest()
            });
        if let Err(e) = saved {
            self.restore(snap);
            let _ = self.storage.remove_file(&path);
            self.drop_dir(&dir, made_dir);
            return Err(e);
        }
        Ok(id)
    }

    /// Creates a folder under `parent` (`None` is the top level, at the
    /// project root). Its disk folder never reuses an existing name.
    pub fn create_folder(&mut self, parent: Option<&Id>, title: &str) -> Result<Id, ProjectError> {
        let parent_dir = self.disk_dir(parent)?;
        let title = normalize_text(title);
        let entries = self.entries_of(&parent_dir)?;
        let name = self.pick_name(&parent_dir, &slugify(&title), "", entries, None, false);
        let path = parent_dir.join(&name);
        if self.storage.exists(&path) {
            return Err(ProjectError::Exists(path));
        }
        self.storage.create_dir_all(&path)?;
        let id = Id::new();
        let node = Node::Folder(FolderNode {
            id: id.clone(),
            title,
            path: path.as_str().to_string(),
            synopsis: String::new(),
            children: Vec::new(),
            extra: serde_json::Map::new(),
        });
        let snap = self.snapshot();
        let siblings = match parent {
            None => Some(&mut self.manifest.tree),
            Some(pid) => self.manifest.folder_mut(pid).map(|f| &mut f.children),
        };
        let saved = match siblings {
            Some(list) => {
                list.push(node);
                self.save_manifest()
            }
            None => Err(ProjectError::NotFound(
                parent.cloned().unwrap_or_else(Id::new),
            )),
        };
        if let Err(e) = saved {
            self.restore(snap);
            let _ = self.storage.remove_dir(&path);
            return Err(e);
        }
        Ok(id)
    }

    /// Replaces the body of the item with `body` (normalised), keeping its
    /// header, including keys this version does not know. Never creates a
    /// file, and never writes to a file that does not hold this item.
    pub fn save_body(&mut self, id: &Id, body: &str) -> Result<(), ProjectError> {
        let (path, mut item) = self.read_verified(id)?;
        item.body = normalize_text(body);
        let text = render_markdown_item(&item);
        self.storage.write_atomic(&path, text.as_bytes())?;
        Ok(())
    }

    /// Sets the title and renames the file (and the companions next to it) to
    /// match. Returns the item's path afterwards.
    pub fn rename_item(&mut self, id: &Id, title: &str) -> Result<RelPath, ProjectError> {
        let (old, mut item) = self.read_verified(id)?;
        let title = normalize_text(title);
        let dir = old.parent().unwrap_or_else(RelPath::root);
        let own = old.file_name().to_string();
        let mut taken = self.entries_of(&dir)?;
        taken.retain(|n| fold(n) != fold(&own));
        let name = self.pick_name(&dir, &slugify(&title), "md", taken, Some(&own), true);
        let same = fold(&name) == fold(&own);
        let new = if same { old.clone() } else { dir.join(&name) };
        let moves = if same {
            Vec::new()
        } else {
            self.plan_moves(&old, &new)
        };
        self.move_files(&moves)?;
        item.header.title = title;
        let text = render_markdown_item(&item);
        if let Err(e) = self.storage.write_atomic(&new, text.as_bytes()) {
            self.undo_moves(&moves);
            return Err(e.into());
        }
        self.index.insert(id.clone(), new.clone());
        Ok(new)
    }

    /// Sets the folder title and renames its disk folder to match, then
    /// updates the paths of everything inside it.
    pub fn rename_folder(&mut self, id: &Id, title: &str) -> Result<(), ProjectError> {
        let old_str = match self.manifest.folder(id) {
            Some(f) => f.path.clone(),
            None if self.manifest.parent_of(id).is_some() => {
                return Err(ProjectError::NotAFolder(id.clone()));
            }
            None => return Err(ProjectError::NotFound(id.clone())),
        };
        let old = RelPath::new(&old_str)?;
        let parent = old.parent().unwrap_or_else(RelPath::root);
        let own = old.file_name().to_string();
        let title = normalize_text(title);
        let mut taken = self.entries_of(&parent)?;
        taken.retain(|n| fold(n) != fold(&own));
        let name = self.pick_name(&parent, &slugify(&title), "", taken, Some(&own), false);
        let same = fold(&name) == fold(&own);
        let new = if same {
            old.clone()
        } else {
            parent.join(&name)
        };
        let moves: Moves = if same {
            Vec::new()
        } else {
            vec![(old.clone(), new.clone())]
        };
        self.move_files(&moves)?;

        let snap = self.snapshot();
        if let Some(f) = self.manifest.folder_mut(id) {
            f.title = title;
        }
        if !same {
            repath_folders(&mut self.manifest.tree, &old_str, new.as_str());
            for path in self.index.values_mut() {
                if let Some(p) = reprefix(path.as_str(), &old_str, new.as_str())
                    && let Ok(p) = RelPath::new(&p)
                {
                    *path = p;
                }
            }
        }
        if let Err(e) = self.save_manifest() {
            self.restore(snap);
            self.undo_moves(&moves);
            return Err(e);
        }
        Ok(())
    }

    /// Puts the item at `index` (clamped, counted after it is taken out) in
    /// `to_folder` (`None` is the top level). When the disk folder changes,
    /// the file and its companions move too.
    pub fn move_item(
        &mut self,
        id: &Id,
        to_folder: Option<&Id>,
        index: usize,
    ) -> Result<(), ProjectError> {
        if self.manifest.parent_of(id).is_none() || self.manifest.folder(id).is_some() {
            return Err(ProjectError::NotFound(id.clone()));
        }
        let dest = self.disk_dir(to_folder)?;
        let (cur, item) = self.read_verified(id)?;
        let cur_dir = cur.parent().unwrap_or_else(RelPath::root);

        let mut new_path = cur.clone();
        let mut moves: Moves = Vec::new();
        let mut made_dir = false;
        if dest != cur_dir {
            made_dir = !self.storage.exists(&dest);
            if made_dir {
                self.storage.create_dir_all(&dest)?;
            }
            let entries = match self.entries_of(&dest) {
                Ok(e) => e,
                Err(e) => {
                    self.drop_dir(&dest, made_dir);
                    return Err(e);
                }
            };
            let slug = slugify(&item.header.title);
            let name = self.pick_name(&dest, &slug, "md", entries, None, true);
            new_path = dest.join(&name);
            moves = self.plan_moves(&cur, &new_path);
            if let Err(e) = self.move_files(&moves) {
                self.drop_dir(&dest, made_dir);
                return Err(e);
            }
        }

        let snap = self.snapshot();
        self.manifest.remove_node(id);
        let saved = self
            .manifest
            .insert_item(to_folder, id.clone(), Some(index))
            .map_err(ProjectError::from)
            .and_then(|()| {
                self.index.insert(id.clone(), new_path.clone());
                self.save_manifest()
            });
        if let Err(e) = saved {
            self.restore(snap);
            self.undo_moves(&moves);
            self.drop_dir(&dest, made_dir);
            return Err(e);
        }
        Ok(())
    }

    /// Reads the file at the indexed path. It must parse, be readable by this
    /// version, and carry `id`; otherwise nothing may be written to it.
    fn read_verified(&self, id: &Id) -> Result<(RelPath, ItemFile), ProjectError> {
        let path = self
            .index
            .get(id)
            .cloned()
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        let bytes = self.storage.read(&path)?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| HeaderError::Invalid("the file is not valid UTF-8".to_string()))?;
        let item = parse_markdown_item(text)?;
        if item.header.id != *id {
            return Err(ProjectError::NotFound(id.clone()));
        }
        check_readable(item.header.min_reader)?;
        Ok((path, item))
    }

    /// The disk folder of a manifest folder (`None` is the project root).
    fn disk_dir(&self, folder: Option<&Id>) -> Result<RelPath, ProjectError> {
        let Some(id) = folder else {
            return Ok(RelPath::root());
        };
        match self.manifest.folder(id) {
            Some(f) => Ok(RelPath::new(&f.path)?),
            None if self.manifest.parent_of(id).is_some() => {
                Err(ProjectError::NotAFolder(id.clone()))
            }
            None => Err(ProjectError::NotFound(id.clone())),
        }
    }

    /// Names in a disk folder; a missing folder has none.
    fn entries_of(&self, dir: &RelPath) -> Result<Vec<String>, ProjectError> {
        match self.storage.list(dir) {
            Ok(v) => Ok(v.into_iter().map(|e| e.name).collect()),
            Err(e) if e.cause() == ErrorCause::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    /// The first unique name whose own path (and, for documents, every
    /// companion path) is free on disk. `own` is the current name when
    /// renaming: choosing it again means no move is needed.
    fn pick_name(
        &self,
        dir: &RelPath,
        slug: &str,
        ext: &str,
        mut taken: Vec<String>,
        own: Option<&str>,
        with_companions: bool,
    ) -> String {
        loop {
            let name = unique_name(slug, ext, &taken);
            if own.is_some_and(|o| fold(o) == fold(&name)) {
                return name;
            }
            let target = dir.join(&name);
            let free = !self.storage.exists(&target)
                && (!with_companions
                    || companions_of(&target)
                        .iter()
                        .all(|c| !self.storage.exists(c)));
            if free {
                return name;
            }
            taken.push(name);
        }
    }

    /// The document first, then each companion that exists.
    fn plan_moves(&self, old: &RelPath, new: &RelPath) -> Moves {
        let mut moves = vec![(old.clone(), new.clone())];
        for (from, to) in companions_of(old).into_iter().zip(companions_of(new)) {
            if self.storage.exists(&from) {
                moves.push((from, to));
            }
        }
        moves
    }

    /// Renames in order, refusing to replace anything. If one fails, the ones
    /// already done are put back.
    fn move_files(&self, moves: &[(RelPath, RelPath)]) -> Result<(), ProjectError> {
        for (i, (from, to)) in moves.iter().enumerate() {
            if self.storage.exists(to) {
                self.undo_moves(&moves[..i]);
                return Err(ProjectError::Exists(to.clone()));
            }
            if let Err(e) = self.storage.rename(from, to) {
                self.undo_moves(&moves[..i]);
                return Err(e.into());
            }
        }
        Ok(())
    }

    /// Best effort: moves each file back, last first, unless something now
    /// sits at the old place.
    fn undo_moves(&self, done: &[(RelPath, RelPath)]) {
        for (from, to) in done.iter().rev() {
            if !self.storage.exists(from) {
                let _ = self.storage.rename(to, from);
            }
        }
    }

    /// Removes a folder this call just made (only if it is still empty).
    fn drop_dir(&self, dir: &RelPath, made: bool) {
        if made {
            let _ = self.storage.remove_dir(dir);
        }
    }

    fn save_manifest(&self) -> Result<(), ProjectError> {
        let path = RelPath::root().join(MANIFEST_FILE);
        self.storage
            .write_atomic(&path, self.manifest.to_json().as_bytes())?;
        Ok(())
    }

    fn snapshot(&self) -> Snapshot {
        (self.manifest.clone(), self.index.clone())
    }

    fn restore(&mut self, (manifest, index): Snapshot) {
        self.manifest = manifest;
        self.index = index;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::*;
    use crate::id::Id;
    use crate::manifest::Node;
    use crate::project::*;
    use crate::storage::*;
    use std::cell::Cell;
    use std::path::Path;

    fn setup() -> (tempfile::TempDir, Project<FsStorage>, Id) {
        let d = tempfile::tempdir().unwrap();
        let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        (d, p, ms)
    }

    fn top_id<S: Storage>(p: &Project<S>) -> Id {
        match &p.manifest().tree[0] {
            Node::Folder(f) => f.id.clone(),
            _ => unreachable!(),
        }
    }

    fn read(d: &Path, rel: &str) -> Vec<u8> {
        std::fs::read(d.join(rel)).unwrap()
    }

    fn children<S: Storage>(p: &Project<S>, folder: &Id) -> Vec<Id> {
        p.manifest()
            .folder(folder)
            .unwrap()
            .children
            .iter()
            .map(|n| n.id().clone())
            .collect()
    }

    /// Wraps `FsStorage` and fails chosen calls.
    struct Flaky {
        inner: FsStorage,
        renames: Cell<usize>,
        fail_rename_at: Cell<Option<usize>>,
        fail_manifest: Cell<bool>,
    }

    impl Flaky {
        fn new(root: &Path) -> Flaky {
            Flaky {
                inner: FsStorage::new(root),
                renames: Cell::new(0),
                fail_rename_at: Cell::new(None),
                fail_manifest: Cell::new(false),
            }
        }
    }

    impl Storage for Flaky {
        fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError> {
            self.inner.read(p)
        }
        fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError> {
            if self.fail_manifest.get() && p.as_str() == MANIFEST_FILE {
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

    fn flaky_setup() -> (tempfile::TempDir, Project<Flaky>, Id) {
        let d = tempfile::tempdir().unwrap();
        let p = Project::create(Flaky::new(d.path()), "X").unwrap();
        let ms = top_id(&p);
        (d, p, ms)
    }

    #[test]
    fn renaming_to_an_existing_title_gets_a_suffix_and_leaves_the_other_file_alone() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "Alpha", DocKind::Prose).unwrap();
        let b = p.create_document(&ms, "Beta", DocKind::Prose).unwrap();
        p.save_body(&a, "alpha text\n").unwrap();
        let before = read(d.path(), "manuscript/alpha.md");
        let new_path = p.rename_item(&b, "Alpha").unwrap();
        assert_eq!(new_path.as_str(), "manuscript/alpha-2.md");
        assert_eq!(read(d.path(), "manuscript/alpha.md"), before);
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/alpha.md");
        assert!(!d.path().join("manuscript/beta.md").exists());
    }

    #[test]
    fn a_stray_companion_at_the_target_makes_rename_pick_another_name() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[1]").unwrap();
        std::fs::write(
            d.path().join("manuscript/the-arrival.comments.json"),
            "stray",
        )
        .unwrap();
        let new_path = p.rename_item(&id, "The Arrival").unwrap();
        assert_eq!(new_path.as_str(), "manuscript/the-arrival-2.md");
        assert_eq!(
            read(d.path(), "manuscript/the-arrival-2.comments.json"),
            b"[1]"
        );
        assert_eq!(
            read(d.path(), "manuscript/the-arrival.comments.json"),
            b"stray"
        );
        assert!(!d.path().join("manuscript/arrival.comments.json").exists());
    }

    #[test]
    fn a_title_with_the_same_slug_only_changes_the_header() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        let new_path = p.rename_item(&id, "ARRIVAL!").unwrap();
        assert_eq!(new_path.as_str(), "manuscript/arrival.md");
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/arrival.md");
        assert_eq!(p.read_item(&id).unwrap().header.title, "ARRIVAL!");
        let names: Vec<_> = std::fs::read_dir(d.path().join("manuscript"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["arrival.md".to_string()]);
    }

    #[test]
    fn rename_keeps_unknown_header_keys() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        let f = d.path().join("manuscript/arrival.md");
        let raw =
            std::fs::read_to_string(&f)
                .unwrap()
                .replacen("---\n", "---\nfutureKey: keep me\n", 1);
        std::fs::write(&f, raw).unwrap();
        p.rename_item(&id, "Departure").unwrap();
        let out = std::fs::read_to_string(d.path().join("manuscript/departure.md")).unwrap();
        assert!(out.contains("futureKey: keep me"));
        assert!(out.contains("title: Departure"));
    }

    #[test]
    fn save_body_refuses_a_file_that_now_holds_a_different_item() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        let other = render_markdown_item(&ItemFile {
            header: ItemHeader::new_document("Other", DocKind::Prose),
            body: "other work\n".into(),
        });
        std::fs::write(d.path().join("manuscript/arrival.md"), &other).unwrap();
        assert!(matches!(
            p.save_body(&id, "mine"),
            Err(ProjectError::NotFound(ref i)) if *i == id
        ));
        assert!(matches!(p.read_item(&id), Err(ProjectError::NotFound(_))));
        assert!(matches!(
            p.rename_item(&id, "Z"),
            Err(ProjectError::NotFound(_))
        ));
        assert_eq!(read(d.path(), "manuscript/arrival.md"), other.as_bytes());
    }

    #[test]
    fn save_body_after_the_file_was_deleted_errors_and_does_not_recreate_it() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        std::fs::remove_file(d.path().join("manuscript/arrival.md")).unwrap();
        assert!(p.save_body(&id, "text").is_err());
        assert!(!d.path().join("manuscript/arrival.md").exists());
    }

    #[test]
    fn a_broken_header_on_disk_is_reported_and_the_file_is_left_alone() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        let broken = b"---\nid: [unclosed\n---\nprecious words\n";
        std::fs::write(d.path().join("manuscript/arrival.md"), broken).unwrap();
        assert!(matches!(
            p.save_body(&id, "x"),
            Err(ProjectError::Header(_))
        ));
        assert!(matches!(
            p.rename_item(&id, "Y"),
            Err(ProjectError::Header(_))
        ));
        assert_eq!(read(d.path(), "manuscript/arrival.md"), broken);
    }

    #[test]
    fn create_folder_never_reuses_an_existing_disk_folder() {
        let (d, mut p, _ms) = setup();
        std::fs::write(d.path().join("notes/keep.txt"), "mine").unwrap();
        let id = p.create_folder(None, "Notes").unwrap();
        assert!(d.path().join("notes-2").is_dir());
        assert_eq!(p.manifest().folder(&id).unwrap().path, "notes-2");
        assert_eq!(read(d.path(), "notes/keep.txt"), b"mine");
        assert_eq!(p.manifest().tree.len(), 2);
        let (reopened, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(reopened.manifest().folder(&id).unwrap().title, "Notes");
    }

    #[test]
    fn create_document_recreates_a_deleted_disk_folder() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        std::fs::remove_dir(d.path().join("manuscript/part")).unwrap();
        let id = p.create_document(&part, "Arrival", DocKind::Prose).unwrap();
        assert_eq!(
            p.path_of(&id).unwrap().as_str(),
            "manuscript/part/arrival.md"
        );
        assert!(d.path().join("manuscript/part/arrival.md").is_file());
    }

    #[test]
    fn create_document_needs_a_folder() {
        let (_d, mut p, ms) = setup();
        let doc = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        assert!(matches!(
            p.create_document(&doc, "B", DocKind::Prose),
            Err(ProjectError::NotAFolder(_))
        ));
        assert!(matches!(
            p.create_document(&Id::new(), "B", DocKind::Prose),
            Err(ProjectError::NotFound(_))
        ));
    }

    #[test]
    fn move_item_reorders_within_a_folder() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let b = p.create_document(&ms, "B", DocKind::Prose).unwrap();
        let c = p.create_document(&ms, "C", DocKind::Prose).unwrap();
        p.move_item(&a, Some(&ms), 2).unwrap();
        assert_eq!(children(&p, &ms), vec![b, c.clone(), a.clone()]);
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/a.md");
        p.move_item(&c, Some(&ms), 99).unwrap();
        assert_eq!(children(&p, &ms).last(), Some(&c));
        assert!(d.path().join("manuscript/a.md").is_file());
        let (reopened, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(children(&reopened, &ms).last(), Some(&c));
    }

    #[test]
    fn move_item_to_the_top_level_lands_in_the_project_root() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        p.move_item(&id, None, 0).unwrap();
        assert_eq!(p.path_of(&id).unwrap().as_str(), "arrival.md");
        assert!(d.path().join("arrival.md").is_file());
        assert!(!d.path().join("manuscript/arrival.md").exists());
        assert_eq!(p.manifest().parent_of(&id), Some((None, 0)));
    }

    #[test]
    fn move_item_carries_companions_and_avoids_name_clashes() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        let other = p.create_document(&part, "Arrival", DocKind::Prose).unwrap();
        std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[a]").unwrap();
        std::fs::write(d.path().join("manuscript/arrival.md.meta.json"), "{a}").unwrap();
        p.move_item(&a, Some(&part), 0).unwrap();
        assert_eq!(
            p.path_of(&a).unwrap().as_str(),
            "manuscript/part/arrival-2.md"
        );
        assert_eq!(
            read(d.path(), "manuscript/part/arrival-2.comments.json"),
            b"[a]"
        );
        assert_eq!(
            read(d.path(), "manuscript/part/arrival-2.md.meta.json"),
            b"{a}"
        );
        assert!(!d.path().join("manuscript/arrival.comments.json").exists());
        assert!(!d.path().join("manuscript/arrival.md.meta.json").exists());
        assert_eq!(
            p.path_of(&other).unwrap().as_str(),
            "manuscript/part/arrival.md"
        );
    }

    #[test]
    fn move_item_reports_bad_ids() {
        let (_d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let b = p.create_document(&ms, "B", DocKind::Prose).unwrap();
        assert!(matches!(
            p.move_item(&a, Some(&b), 0),
            Err(ProjectError::NotAFolder(_))
        ));
        assert!(matches!(
            p.move_item(&Id::new(), Some(&ms), 0),
            Err(ProjectError::NotFound(_))
        ));
        assert!(matches!(
            p.move_item(&a, Some(&Id::new()), 0),
            Err(ProjectError::NotFound(_))
        ));
        assert!(matches!(
            p.move_item(&ms, None, 0),
            Err(ProjectError::NotFound(_))
        ));
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/a.md");
    }

    #[test]
    fn rename_folder_matches_whole_path_segments_and_updates_nested_folders() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let two = p.create_folder(Some(&ms), "Part Two").unwrap();
        let inner = p.create_folder(Some(&part), "Inner").unwrap();
        let in_part = p.create_document(&part, "One", DocKind::Prose).unwrap();
        let in_inner = p.create_document(&inner, "Deep", DocKind::Prose).unwrap();
        let in_two = p.create_document(&two, "Two", DocKind::Prose).unwrap();
        p.rename_folder(&part, "Chapter").unwrap();
        assert_eq!(
            p.manifest().folder(&part).unwrap().path,
            "manuscript/chapter"
        );
        assert_eq!(p.manifest().folder(&part).unwrap().title, "Chapter");
        assert_eq!(
            p.manifest().folder(&inner).unwrap().path,
            "manuscript/chapter/inner"
        );
        assert_eq!(
            p.manifest().folder(&two).unwrap().path,
            "manuscript/part-two"
        );
        assert_eq!(
            p.path_of(&in_part).unwrap().as_str(),
            "manuscript/chapter/one.md"
        );
        assert_eq!(
            p.path_of(&in_inner).unwrap().as_str(),
            "manuscript/chapter/inner/deep.md"
        );
        assert_eq!(
            p.path_of(&in_two).unwrap().as_str(),
            "manuscript/part-two/two.md"
        );
        assert!(d.path().join("manuscript/chapter/inner/deep.md").is_file());
        assert!(d.path().join("manuscript/part-two/two.md").is_file());
        let (reopened, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(
            reopened.manifest().folder(&inner).unwrap().path,
            "manuscript/chapter/inner"
        );
    }

    #[test]
    fn rename_folder_never_takes_an_existing_name() {
        let (d, mut p, ms) = setup();
        let a = p.create_folder(Some(&ms), "Alpha").unwrap();
        p.create_folder(Some(&ms), "Beta").unwrap();
        std::fs::write(d.path().join("manuscript/beta/keep.txt"), "mine").unwrap();
        p.rename_folder(&a, "Beta").unwrap();
        assert_eq!(p.manifest().folder(&a).unwrap().path, "manuscript/beta-2");
        assert_eq!(read(d.path(), "manuscript/beta/keep.txt"), b"mine");
    }

    #[test]
    fn rename_folder_with_the_same_slug_keeps_the_path() {
        let (_d, mut p, ms) = setup();
        let a = p.create_folder(Some(&ms), "Alpha").unwrap();
        p.rename_folder(&a, "ALPHA").unwrap();
        let f = p.manifest().folder(&a).unwrap();
        assert_eq!(f.path, "manuscript/alpha");
        assert_eq!(f.title, "ALPHA");
    }

    #[test]
    fn a_failing_companion_rename_puts_the_main_file_back() {
        let (d, mut p, ms) = flaky_setup();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[]").unwrap();
        let before = read(d.path(), "manuscript/arrival.md");
        p.storage().fail_rename_at.set(Some(2));
        let r = p.rename_item(&id, "The Arrival");
        assert!(matches!(r, Err(ProjectError::Storage(_))), "{r:?}");
        assert_eq!(read(d.path(), "manuscript/arrival.md"), before);
        assert_eq!(read(d.path(), "manuscript/arrival.comments.json"), b"[]");
        assert!(!d.path().join("manuscript/the-arrival.md").exists());
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/arrival.md");
    }

    #[test]
    fn a_failed_manifest_save_undoes_create_folder_and_create_document() {
        let (d, mut p, ms) = flaky_setup();
        let before = p.manifest().clone();
        p.storage().fail_manifest.set(true);
        assert!(p.create_folder(Some(&ms), "Part").is_err());
        assert!(!d.path().join("manuscript/part").exists());
        assert_eq!(p.manifest(), &before);

        assert!(p.create_document(&ms, "Arrival", DocKind::Prose).is_err());
        assert!(!d.path().join("manuscript/arrival.md").exists());
        assert_eq!(p.manifest(), &before);
    }

    #[test]
    fn a_failed_manifest_save_undoes_move_and_folder_rename() {
        let (d, mut p, ms) = flaky_setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[]").unwrap();
        let before = p.manifest().clone();
        let bytes = read(d.path(), "manuscript/arrival.md");
        p.storage().fail_manifest.set(true);

        assert!(p.move_item(&id, Some(&part), 0).is_err());
        assert_eq!(read(d.path(), "manuscript/arrival.md"), bytes);
        assert!(d.path().join("manuscript/arrival.comments.json").is_file());
        assert!(!d.path().join("manuscript/part/arrival.md").exists());
        assert_eq!(p.manifest(), &before);
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/arrival.md");

        assert!(p.rename_folder(&part, "Chapter").is_err());
        assert!(d.path().join("manuscript/part").is_dir());
        assert!(!d.path().join("manuscript/chapter").exists());
        assert_eq!(p.manifest(), &before);
    }

    #[test]
    fn companions_of_lists_the_candidate_paths() {
        let p = RelPath::new("manuscript/arrival.md").unwrap();
        let c: Vec<_> = companions_of(&p)
            .iter()
            .map(|c| c.as_str().to_string())
            .collect();
        assert_eq!(
            c,
            vec![
                "manuscript/arrival.comments.json",
                "manuscript/arrival.md.meta.json"
            ]
        );
    }
}
