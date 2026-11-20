// SPDX-License-Identifier: MPL-2.0
use crate::error::AppError;
use pulp_core::storage::{FsStorage, RelPath, Storage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Per-computer settings. Lives in the app config folder, never in the Library.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DeviceConfig {
    pub library_path: Option<PathBuf>,
    pub external_projects: Vec<PathBuf>,
    pub last_opened: BTreeMap<String, String>,
}

impl DeviceConfig {
    /// The default when the file is missing or unreadable.
    pub fn load(path: &Path) -> DeviceConfig {
        std::fs::read(path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), AppError> {
        let (dir, name) = match (path.parent(), path.file_name().and_then(|n| n.to_str())) {
            (Some(d), Some(n)) => (d, n),
            _ => {
                return Err(AppError::other(format!(
                    "bad config path {}",
                    path.display()
                )));
            }
        };
        let storage = FsStorage::new(dir);
        storage.create_dir_all(&RelPath::root())?;
        let mut json =
            serde_json::to_string_pretty(self).map_err(|e| AppError::other(e.to_string()))?;
        json.push('\n');
        storage.write_atomic(&RelPath::new(name)?, json.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_config_round_trips_and_defaults_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cfg").join("device.json");
        assert_eq!(DeviceConfig::load(&path), DeviceConfig::default());

        let mut c = DeviceConfig {
            library_path: Some(PathBuf::from("/x/Pulp")),
            external_projects: vec![PathBuf::from("/y")],
            ..Default::default()
        };
        c.last_opened.insert("lib".into(), "proj".into());
        c.save(&path).unwrap();
        assert_eq!(DeviceConfig::load(&path), c);

        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(DeviceConfig::load(&path), DeviceConfig::default());
    }
}
