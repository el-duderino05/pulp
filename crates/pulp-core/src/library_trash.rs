// SPDX-License-Identifier: MPL-2.0
//! The Library trash: whole projects moved out of `Projects/` into
//! `.trash/<entry>/`, using the project trash's `entry.json` (decision D4):
//! `itemId` is the project id, `originalPath` is `Projects/<folder>`,
//! `originalFolder` is null, `index` is 0 and `files` is the folder name.

use std::path::PathBuf;

use serde::Serialize;

use crate::brand::MANIFEST_FILE;
use crate::filename::unique_name;
use crate::id::Id;
use crate::library::{LIBRARY_DIRS, Library, LibraryError, name_and_id, rel};
use crate::project::ProjectError;
use crate::storage::{ErrorCause, FsStorage, RelPath, Storage, StorageError};
use crate::trash::{TrashEntry, entry_dir, load_entry, now, store_in_trash, trash_root};
use crate::version::FORMAT_VERSION;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrashEntry {
    pub entry_id: Id,
    pub project_id: Id,
    pub name: String,
    pub deleted_at: String,
}

fn from_project_error(e: ProjectError) -> LibraryError {
    match e {
        ProjectError::Storage(e) => LibraryError::Storage(e),
        ProjectError::Version(e) => LibraryError::Version(e),
        e => LibraryError::Invalid(format!("the trash could not be used: {e}")),
    }
}

fn not_found(entry: &Id) -> LibraryError {
    LibraryError::Storage(StorageError::NotFound(
        entry_dir(entry),
        "no such trash entry".into(),
    ))
}

/// The one project folder an entry holds, if it is a Library trash entry.
fn project_entry<S: Storage>(storage: &S, entry: &Id) -> Option<TrashEntry> {
    let e = load_entry(storage, entry)?;
    let ok = e.folder_node.is_none()
        && e.files.len() == 1
        && e.original_path.as_str() == format!("{}/{}", LIBRARY_DIRS[0], e.files[0]);
    ok.then_some(e)
}

impl Library {
    /// Moves `Projects/<folder>` into the trash and returns the entry id.
    pub fn trash_project(&self, folder: &str) -> Result<Id, LibraryError> {
        let storage = FsStorage::new(&self.root);
        let invalid = || LibraryError::Invalid(format!("not a project folder: {folder}"));
        let path = rel(LIBRARY_DIRS[0]).join(folder);
        if RelPath::new(folder).map_or(true, |p| p.file_name() != folder) {
            return Err(invalid());
        }
        let manifest =
            String::from_utf8(storage.read(&path.join(MANIFEST_FILE))?).map_err(|_| invalid())?;
        let (_, project_id) = name_and_id(&manifest).ok_or_else(invalid)?;
        let entry = TrashEntry {
            format: FORMAT_VERSION,
            entry_id: Id::new(),
            item_id: project_id,
            original_path: path.clone(),
            original_folder: None,
            index: 0,
            deleted_at: now(),
            files: Vec::new(), // filled in by store_in_trash
            folder_node: None,
        };
        store_in_trash(&storage, &[path], entry).map_err(from_project_error)
    }

    /// Every readable project entry, newest first, then by entry id. The name
    /// comes from the trashed manifest, else the folder name.
    pub fn list_trash(&self) -> Result<Vec<LibraryTrashEntry>, LibraryError> {
        let storage = FsStorage::new(&self.root);
        let names = match storage.list(&trash_root()) {
            Ok(v) => v,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out: Vec<LibraryTrashEntry> = names
            .iter()
            .filter(|e| e.is_dir)
            .filter_map(|e| Id::parse(&e.name).ok())
            .filter_map(|id| project_entry(&storage, &id))
            .map(|e| {
                let manifest = entry_dir(&e.entry_id).join(&e.files[0]).join(MANIFEST_FILE);
                let read = storage
                    .read(&manifest)
                    .ok()
                    .and_then(|b| String::from_utf8(b).ok())
                    .and_then(|t| name_and_id(&t));
                LibraryTrashEntry {
                    entry_id: e.entry_id,
                    project_id: e.item_id,
                    name: read.map_or_else(|| e.files[0].clone(), |(n, _)| n),
                    deleted_at: e.deleted_at,
                }
            })
            .collect();
        out.sort_by(|a, b| {
            b.deleted_at
                .cmp(&a.deleted_at)
                .then_with(|| a.entry_id.cmp(&b.entry_id))
        });
        Ok(out)
    }

    /// Puts a trashed project back in `Projects/`, under a unique folder name
    /// when the old one is taken, and returns its absolute folder.
    pub fn restore_project(&self, entry: &Id) -> Result<PathBuf, LibraryError> {
        let storage = FsStorage::new(&self.root);
        let e = project_entry(&storage, entry).ok_or_else(|| not_found(entry))?;
        let projects = rel(LIBRARY_DIRS[0]);
        let existing: Vec<String> = storage
            .list(&projects)?
            .into_iter()
            .map(|x| x.name)
            .collect();
        let folder = unique_name(&e.files[0], "", &existing);
        let dir = entry_dir(entry);
        storage.rename(&dir.join(&e.files[0]), &projects.join(&folder))?;
        // The project is back; leftovers of the entry are only bookkeeping.
        let _ = storage.remove_file(&dir.join("entry.json"));
        let _ = storage.remove_dir(&dir);
        Ok(self.root.join(LIBRARY_DIRS[0]).join(folder))
    }

    /// Sends one trash entry to the computer's trash for good.
    pub fn delete_from_trash(&self, entry: &Id) -> Result<(), LibraryError> {
        self.delete_from_trash_with(&FsStorage::new(&self.root), entry)
    }

    /// Sends every trash entry to the computer's trash; returns how many.
    /// Stops at the first failure.
    pub fn empty_trash(&self) -> Result<usize, LibraryError> {
        self.empty_trash_with(&FsStorage::new(&self.root))
    }

    /// `delete_from_trash` through a chosen storage (tests inject one).
    #[doc(hidden)]
    pub fn delete_from_trash_with<S: Storage>(
        &self,
        storage: &S,
        entry: &Id,
    ) -> Result<(), LibraryError> {
        let dir = entry_dir(entry);
        if !storage.exists(&dir) {
            return Err(not_found(entry));
        }
        Ok(storage.discard(&dir)?)
    }

    /// `empty_trash` through a chosen storage (tests inject one).
    #[doc(hidden)]
    pub fn empty_trash_with<S: Storage>(&self, storage: &S) -> Result<usize, LibraryError> {
        let names = match storage.list(&trash_root()) {
            Ok(v) => v,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        let mut count = 0;
        for e in names.iter().filter(|e| e.is_dir) {
            if let Ok(id) = Id::parse(&e.name) {
                storage.discard(&entry_dir(&id))?;
                count += 1;
            }
        }
        Ok(count)
    }
}
