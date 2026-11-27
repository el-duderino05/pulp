// SPDX-License-Identifier: MPL-2.0
use crate::device::DeviceConfig;
use crate::error::{AppError, ErrorCode};
use pulp_core::library::Library;
use pulp_core::project::{Project, ProjectError};
use pulp_core::storage::FsStorage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};

pub type OpenProject = Arc<Mutex<Project<FsStorage>>>;

pub struct AppState {
    pub config_path: PathBuf,
    pub device: Mutex<DeviceConfig>,
    pub library: Mutex<Option<Library>>,
    /// Open projects by project id. Each has its own lock, so work on one
    /// project never waits for another.
    pub projects: Mutex<HashMap<String, OpenProject>>,
    /// Held while a project is being opened.
    pub opening: Mutex<()>,
}

/// Runs `f` on the one open project `id`, holding only that project's lock.
pub fn with_project<T>(
    state: &AppState,
    id: &str,
    f: impl FnOnce(&mut Project<FsStorage>) -> Result<T, ProjectError>,
) -> Result<T, AppError> {
    let project = state
        .projects
        .lock()
        .expect("projects lock")
        .get(id)
        .cloned()
        .ok_or_else(|| AppError {
            code: ErrorCode::NotFound,
            detail: format!("project {id} is not open"),
        })?;
    let mut guard = project.lock().expect("project lock");
    Ok(f(&mut guard)?)
}

/// Runs `f` with the open Library.
pub fn with_library<T>(
    state: &AppState,
    f: impl FnOnce(&Library) -> Result<T, AppError>,
) -> Result<T, AppError> {
    let guard = state.library.lock().expect("library lock");
    match guard.as_ref() {
        Some(lib) => f(lib),
        None => Err(AppError::other("no Library is open")),
    }
}

/// Saves a changed copy of device.json first; the in-memory settings change
/// only once the file is safely written.
pub fn update_device(
    state: &AppState,
    change: impl FnOnce(&mut DeviceConfig),
) -> Result<(), AppError> {
    let mut device = state.device.lock().expect("device lock");
    let mut next = device.clone();
    change(&mut next);
    next.save(&state.config_path)?;
    *device = next;
    Ok(())
}

/// Runs blocking work off the async runtime, with the app state.
pub async fn run<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&AppState) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    tauri::async_runtime::spawn_blocking(move || f(&app.state::<AppState>()))
        .await
        .map_err(|e| AppError::other(e.to_string()))?
}

#[cfg(test)]
pub(crate) fn test_state(dir: &std::path::Path) -> AppState {
    AppState {
        config_path: dir.join("cfg").join("device.json"),
        device: Mutex::new(DeviceConfig::default()),
        library: Mutex::new(Some(Library::create(&dir.join("Pulp"), "Ana").unwrap())),
        projects: Mutex::new(HashMap::new()),
        opening: Mutex::new(()),
    }
}
