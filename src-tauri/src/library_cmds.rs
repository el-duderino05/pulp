// SPDX-License-Identifier: MPL-2.0
use crate::device::DeviceConfig;
use crate::error::AppError;
use crate::state::AppState;
use crate::sync::sync_service;
use pulp_core::library::Library;
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
    let mut device = state.device.lock().expect("device lock");
    let mut next = device.clone();
    next.library_path = Some(path);
    next.save(&state.config_path)?;
    *device = next;
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
    state: State<'_, AppState>,
    path: String,
    author: String,
) -> Result<LibraryInfo, AppError> {
    let p = PathBuf::from(path);
    let q = p.clone();
    let (lib, i) = blocking(move || create_with(&q, &author)).await??;
    remember(&state, lib, p)?;
    Ok(i)
}

#[tauri::command]
pub async fn open_library(
    state: State<'_, AppState>,
    path: String,
) -> Result<LibraryInfo, AppError> {
    let p = PathBuf::from(path);
    let q = p.clone();
    let (lib, i) = blocking(move || open_with(&q)).await??;
    remember(&state, lib, p)?;
    Ok(i)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

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
