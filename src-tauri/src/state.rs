// SPDX-License-Identifier: MPL-2.0
use crate::device::DeviceConfig;
use pulp_core::library::Library;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config_path: PathBuf,
    pub device: Mutex<DeviceConfig>,
    pub library: Mutex<Option<Library>>,
}
