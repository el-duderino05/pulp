// SPDX-License-Identifier: MPL-2.0
use crate::device::DeviceConfig;
use crate::error::{AppError, ErrorCode};
use crate::project_cmds::{OpenedProject, close_with, first_document, open_project_with};
use crate::state::{AppState, run, update_device, with_library, with_project};
use crate::sync::sync_service;
use pulp_core::header::DocKind;
use pulp_core::id::Id;
use pulp_core::library::Library;
use pulp_core::library_trash::LibraryTrashEntry;
use pulp_core::manifest::Node;
use pulp_core::project::MANIFEST_FILE;
use pulp_core::version::READER_VERSION;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    pub root: String,
    pub author: String,
    pub sync: Option<String>,
    pub recreated: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Startup {
    #[serde(rename_all = "camelCase")]
    FirstLaunch {
        default_path: String,
    },
    Ready {
        library: LibraryInfo,
    },
    LibraryMissing {
        path: String,
    },
    Failed {
        error: AppError,
    },
}

fn info(library: &Library, recreated: Vec<String>) -> LibraryInfo {
    LibraryInfo {
        root: library.root.to_string_lossy().into_owned(),
        author: library.settings.author.name.clone(),
        sync: sync_service(&library.root).map(str::to_string),
        recreated,
    }
}

/// Decides what to show at launch. Never creates anything: a configured
/// folder that is gone is reported, not replaced.
pub fn startup_with(config: &DeviceConfig, default_path: &Path) -> (Startup, Option<Library>) {
    let Some(path) = &config.library_path else {
        let default_path = default_path.to_string_lossy().into_owned();
        return (Startup::FirstLaunch { default_path }, None);
    };
    if !path.exists() {
        let path = path.to_string_lossy().into_owned();
        return (Startup::LibraryMissing { path }, None);
    }
    match Library::open(path) {
        Ok((lib, recreated)) => (
            Startup::Ready {
                library: info(&lib, recreated),
            },
            Some(lib),
        ),
        Err(e) => (Startup::Failed { error: e.into() }, None),
    }
}

fn create_with(path: &Path, author: &str) -> Result<(Library, LibraryInfo), AppError> {
    // The interface offers to open it instead.
    if path.join("library.json").exists() {
        return Err(AppError {
            code: ErrorCode::LibraryExists,
            detail: format!("a Library already exists in {}", path.display()),
        });
    }
    let lib = Library::create(path, author)?;
    let i = info(&lib, Vec::new());
    Ok((lib, i))
}

fn open_with(path: &Path) -> Result<(Library, LibraryInfo), AppError> {
    let (lib, recreated) = Library::open(path)?;
    let i = info(&lib, recreated);
    Ok((lib, i))
}

/// Saves the path to device.json first; the Library is only kept in memory
/// once the setting is safely written.
fn remember(state: &AppState, lib: Library, path: PathBuf) -> Result<(), AppError> {
    update_device(state, |d| d.library_path = Some(path))?;
    *state.library.lock().expect("library lock") = Some(lib);
    Ok(())
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::other(e.to_string()))
}

#[tauri::command]
pub async fn startup(app: AppHandle, state: State<'_, AppState>) -> Result<Startup, AppError> {
    let default = app
        .path()
        .document_dir()
        .map_err(|e| AppError::other(e.to_string()))?
        .join("Pulp");
    let config = state.device.lock().expect("device lock").clone();
    let (result, lib) = blocking(move || startup_with(&config, &default)).await?;
    if lib.is_some() {
        *state.library.lock().expect("library lock") = lib;
    }
    Ok(result)
}

#[tauri::command]
pub async fn create_library(
    app: AppHandle,
    path: String,
    author: String,
) -> Result<LibraryInfo, AppError> {
    run(app, move |s| {
        let p = PathBuf::from(path);
        let (lib, i) = create_with(&p, &author)?;
        remember(s, lib, p)?;
        Ok(i)
    })
    .await
}

#[tauri::command]
pub async fn open_library(app: AppHandle, path: String) -> Result<LibraryInfo, AppError> {
    run(app, move |s| {
        let p = PathBuf::from(path);
        let (lib, i) = open_with(&p)?;
        remember(s, lib, p)?;
        Ok(i)
    })
    .await
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub too_new: bool,
    pub last_opened: Option<String>,
    pub external: bool,
    /// The folder or its manifest is gone (unplugged drive, renamed folder).
    pub missing: bool,
    /// The manifest is there but can't be read.
    pub unreadable: bool,
}

fn invalid(detail: impl Into<String>) -> AppError {
    AppError {
        code: ErrorCode::Invalid,
        detail: detail.into(),
    }
}

fn parse_id(s: &str) -> Result<Id, AppError> {
    Id::parse(s).map_err(|e| invalid(e.to_string()))
}

fn last_opened(state: &AppState, id: &str) -> Option<String> {
    state
        .device
        .lock()
        .expect("device lock")
        .last_opened
        .get(id)
        .cloned()
}

/// A row for the project in `path`, read from its manifest alone so that a
/// project needing a newer Pulp still shows up. A project that can't be found
/// or read still gets a row, flagged, so a writer can see it and (if external)
/// remove it.
fn row_for(state: &AppState, path: &Path, external: bool) -> ProjectRow {
    let shown = path.to_string_lossy().into_owned();
    let flagged = |name: String, missing: bool| ProjectRow {
        id: shown.clone(),
        name,
        path: shown.clone(),
        too_new: false,
        last_opened: None,
        external,
        missing,
        unreadable: !missing,
    };
    let fallback = || {
        path.file_name()
            .map_or_else(|| shown.clone(), |n| n.to_string_lossy().into_owned())
    };
    let manifest = path.join(MANIFEST_FILE);
    if !manifest.exists() {
        return flagged(fallback(), true);
    }
    let parsed = std::fs::read(&manifest)
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            Some((
                v.get("id")?.as_str()?.to_string(),
                v.get("name")?.as_str()?.to_string(),
                v.get("minReader").and_then(|n| n.as_u64()) > Some(READER_VERSION.into()),
            ))
        });
    let Some((id, name, too_new)) = parsed else {
        return flagged(fallback(), false);
    };
    ProjectRow {
        last_opened: last_opened(state, &id),
        id,
        name,
        path: shown,
        too_new,
        external,
        missing: false,
        unreadable: false,
    }
}

pub fn list_projects_with(state: &AppState) -> Result<Vec<ProjectRow>, AppError> {
    let mut rows: Vec<ProjectRow> = with_library(state, |lib| {
        let projects = lib.root.join("Projects");
        let summaries = lib.list_projects()?;
        let mut rows: Vec<ProjectRow> = summaries
            .iter()
            .map(|p| {
                let id = p.id.to_string();
                ProjectRow {
                    last_opened: last_opened(state, &id),
                    id,
                    name: p.name.clone(),
                    path: projects.join(&p.folder).to_string_lossy().into_owned(),
                    too_new: p.too_new,
                    external: false,
                    missing: false,
                    unreadable: false,
                }
            })
            .collect();
        // Project folders the core could not summarise (manifest unreadable).
        let known: Vec<&str> = summaries.iter().map(|p| p.folder.as_str()).collect();
        let mut folders: Vec<_> = std::fs::read_dir(&projects)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().join(MANIFEST_FILE).exists())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|f| !f.starts_with('.') && !known.contains(&f.as_str()))
            .collect();
        folders.sort();
        rows.extend(
            folders
                .iter()
                .map(|f| row_for(state, &projects.join(f), false)),
        );
        Ok(rows)
    })?;
    let external = state
        .device
        .lock()
        .expect("device lock")
        .external_projects
        .clone();
    rows.extend(external.iter().map(|p| row_for(state, p, true)));
    Ok(rows)
}

/// Makes a project with a first chapter and opens it on that chapter (D6).
pub fn create_project_with(state: &AppState, name: &str) -> Result<OpenedProject, AppError> {
    let path = with_library(state, |lib| Ok(lib.create_project(name)?))?;
    let mut opened = open_project_with(state, &path)?;
    with_project(state, &opened.id, |p| {
        if let Some(Node::Folder(f)) = p.manifest().tree.first() {
            let folder = f.id.clone();
            p.create_document(&folder, "Chapter 1", DocKind::Prose)?;
        }
        opened.tree = p.tree();
        opened.first_document = first_document(&opened.tree);
        Ok(())
    })?;
    Ok(opened)
}

pub fn add_external_with(state: &AppState, path: &Path) -> Result<ProjectRow, AppError> {
    let row = row_for(state, path, true);
    if row.missing || row.unreadable {
        return Err(AppError {
            code: ErrorCode::NotAProject,
            detail: format!("{} is not a Pulp project", path.display()),
        });
    }
    update_device(state, |d| {
        if !d.external_projects.iter().any(|p| p == path) {
            d.external_projects.push(path.to_path_buf());
        }
    })?;
    Ok(row)
}

pub fn forget_external_with(state: &AppState, path: &Path) -> Result<(), AppError> {
    update_device(state, |d| d.external_projects.retain(|p| p != path))
}

/// Only projects directly under `Projects/` can be moved to the trash.
pub fn trash_project_with(state: &AppState, path: &Path) -> Result<(), AppError> {
    let id = with_library(state, |lib| {
        let folder = match (path.parent(), path.file_name().and_then(|n| n.to_str())) {
            (Some(parent), Some(f)) if parent == lib.root.join("Projects") => f,
            _ => return Err(invalid("only projects in the Library can be deleted")),
        };
        let id = lib
            .list_projects()?
            .into_iter()
            .find(|p| p.folder == folder)
            .map(|p| p.id.to_string());
        lib.trash_project(folder)?;
        Ok(id)
    })?;
    if let Some(id) = id {
        close_with(state, &id);
    }
    Ok(())
}

pub fn restore_project_with(state: &AppState, entry: &str) -> Result<ProjectRow, AppError> {
    let entry = parse_id(entry)?;
    let path = with_library(state, |lib| Ok(lib.restore_project(&entry)?))?;
    Ok(row_for(state, &path, false))
}

#[tauri::command]
pub async fn list_projects(app: AppHandle) -> Result<Vec<ProjectRow>, AppError> {
    run(app, list_projects_with).await
}

#[tauri::command]
pub async fn create_project(app: AppHandle, name: String) -> Result<OpenedProject, AppError> {
    run(app, move |s| create_project_with(s, &name)).await
}

#[tauri::command]
pub async fn add_external_project(app: AppHandle, path: String) -> Result<ProjectRow, AppError> {
    run(app, move |s| add_external_with(s, Path::new(&path))).await
}

#[tauri::command]
pub async fn forget_external_project(app: AppHandle, path: String) -> Result<(), AppError> {
    run(app, move |s| forget_external_with(s, Path::new(&path))).await
}

#[tauri::command]
pub async fn trash_project(app: AppHandle, path: String) -> Result<(), AppError> {
    run(app, move |s| trash_project_with(s, Path::new(&path))).await
}

#[tauri::command]
pub async fn list_library_trash(app: AppHandle) -> Result<Vec<LibraryTrashEntry>, AppError> {
    run(app, |s| with_library(s, |lib| Ok(lib.list_trash()?))).await
}

#[tauri::command]
pub async fn restore_project(app: AppHandle, entry: String) -> Result<ProjectRow, AppError> {
    run(app, move |s| restore_project_with(s, &entry)).await
}

#[tauri::command]
pub async fn delete_library_trash_entry(app: AppHandle, entry: String) -> Result<(), AppError> {
    run(app, move |s| {
        let entry = parse_id(&entry)?;
        with_library(s, |lib| Ok(lib.delete_from_trash(&entry)?))
    })
    .await
}

#[tauri::command]
pub async fn empty_library_trash(app: AppHandle) -> Result<usize, AppError> {
    run(app, |s| with_library(s, |lib| Ok(lib.empty_trash()?))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_first_launch_when_unset() {
        let (s, lib) = startup_with(&DeviceConfig::default(), Path::new("/docs/Pulp"));
        assert_eq!(
            s,
            Startup::FirstLaunch {
                default_path: Path::new("/docs/Pulp").to_string_lossy().into_owned()
            }
        );
        assert!(lib.is_none());
    }

    #[test]
    fn startup_reports_missing_library_without_creating_it() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("Pulp");
        let config = DeviceConfig {
            library_path: Some(gone.clone()),
            ..Default::default()
        };
        let (s, lib) = startup_with(&config, Path::new("/docs/Pulp"));
        assert!(matches!(s, Startup::LibraryMissing { .. }));
        assert!(lib.is_none());
        assert!(!gone.exists());
    }

    #[test]
    fn startup_opens_an_existing_library_and_rejects_a_plain_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Pulp");
        create_with(&root, "Ana").unwrap();
        let mut config = DeviceConfig {
            library_path: Some(root),
            ..Default::default()
        };
        let (s, lib) = startup_with(&config, Path::new("x"));
        assert!(matches!(&s, Startup::Ready { library } if library.author == "Ana"));
        assert!(lib.is_some());

        config.library_path = Some(dir.path().to_path_buf());
        let (s, _) = startup_with(&config, Path::new("x"));
        assert!(matches!(s, Startup::Failed { error } if error.code == ErrorCode::NotALibrary));
    }
}

#[cfg(test)]
mod project_list_tests {
    use super::*;
    use crate::state::test_state;

    #[test]
    fn external_projects_need_a_manifest_and_only_library_projects_are_trashed() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();

        let plain = dir.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(
            add_external_with(&state, &plain).unwrap_err().code,
            ErrorCode::NotAProject
        );
        // A project outside the Library is added as external, then listed.
        let ext = dir.path().join("ext");
        std::fs::create_dir_all(&ext).unwrap();
        pulp_core::project::Project::create(pulp_core::storage::FsStorage::new(&ext), "Out")
            .unwrap();
        assert!(add_external_with(&state, &ext).unwrap().external);
        let rows = list_projects_with(&state).unwrap();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.id == o.id && r.last_opened.is_some()));
        assert_eq!(
            trash_project_with(&state, &ext).unwrap_err().code,
            ErrorCode::Invalid
        );
        forget_external_with(&state, &ext).unwrap();
        assert_eq!(list_projects_with(&state).unwrap().len(), 1);

        trash_project_with(&state, Path::new(&o.path)).unwrap();
        assert!(list_projects_with(&state).unwrap().is_empty());
        assert!(state.projects.lock().unwrap().is_empty());
    }

    #[test]
    fn unreachable_and_unreadable_projects_still_get_rows() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let o = create_project_with(&state, "Novel").unwrap();

        // An external project whose folder has gone is listed, flagged, and can be forgotten.
        let ext = dir.path().join("usb").join("Elsewhere");
        std::fs::create_dir_all(&ext).unwrap();
        pulp_core::project::Project::create(pulp_core::storage::FsStorage::new(&ext), "Out")
            .unwrap();
        add_external_with(&state, &ext).unwrap();
        std::fs::remove_dir_all(&ext).unwrap();

        // A Library project whose manifest is garbage is listed, flagged.
        let bad = dir.path().join("Pulp").join("Projects").join("broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join(MANIFEST_FILE), b"not json").unwrap();

        let rows = list_projects_with(&state).unwrap();
        assert_eq!(rows.len(), 3);
        assert!(
            rows.iter()
                .any(|r| r.id == o.id && !r.missing && !r.unreadable)
        );
        let gone = rows.iter().find(|r| r.external).unwrap();
        assert!(gone.missing && !gone.unreadable);
        assert_eq!(gone.name, "Elsewhere");
        let broken = rows.iter().find(|r| r.name == "broken").unwrap();
        assert!(broken.unreadable && !broken.missing && !broken.external);

        forget_external_with(&state, &ext).unwrap();
        assert_eq!(list_projects_with(&state).unwrap().len(), 2);
    }

    #[test]
    fn creating_over_an_existing_library_has_its_own_code() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("Pulp");
        create_with(&root, "Ana").unwrap();
        let err = create_with(&root, "Ana").unwrap_err();
        assert_eq!(err.code, ErrorCode::LibraryExists);
    }
}
