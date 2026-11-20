// SPDX-License-Identifier: MPL-2.0
use std::path::Path;

/// Names the cloud-sync service a folder lives under, if any. Components are
/// split on both separators so Windows paths work on any host.
pub fn sync_service(path: &Path) -> Option<&'static str> {
    let s = path.to_string_lossy();
    for part in s.split(['/', '\\']) {
        if part.starts_with("OneDrive") {
            return Some("OneDrive");
        }
        if part.starts_with("Dropbox") {
            return Some("Dropbox");
        }
        if matches!(part, "iCloud Drive" | "iCloudDrive" | "Mobile Documents") {
            return Some("iCloud Drive");
        }
    }
    path.ancestors()
        .any(|a| a.join(".stfolder").exists())
        .then_some("Syncthing")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sync_service_detects_known_folders() {
        let cases = [
            (r"C:\Users\a\OneDrive\Documents\Pulp", Some("OneDrive")),
            (r"C:\Users\a\OneDrive - Acme\Pulp", Some("OneDrive")),
            ("/home/a/Dropbox/Pulp", Some("Dropbox")),
            (r"C:\Users\a\Dropbox (Personal)\Pulp", Some("Dropbox")),
            (r"C:\Users\a\iCloudDrive\Pulp", Some("iCloud Drive")),
            (
                "/Users/a/Library/Mobile Documents/com~apple~CloudDocs/Pulp",
                Some("iCloud Drive"),
            ),
            ("/Users/a/iCloud Drive/Pulp", Some("iCloud Drive")),
            ("/home/a/Documents/Pulp", None),
            (r"C:\Users\a\Documents\Pulp", None),
        ];
        for (p, want) in cases {
            assert_eq!(sync_service(&PathBuf::from(p)), want, "{p}");
        }
    }

    #[test]
    fn sync_service_detects_syncthing_marker_in_an_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("Pulp");
        std::fs::create_dir_all(&lib).unwrap();
        assert_eq!(sync_service(&lib), None);
        std::fs::create_dir(dir.path().join(".stfolder")).unwrap();
        assert_eq!(sync_service(&lib), Some("Syncthing"));
    }
}
