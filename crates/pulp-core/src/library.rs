// SPDX-License-Identifier: MPL-2.0
//! The Library: the writer's home folder and its `library.json` settings.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::brand::MANIFEST_FILE;
use crate::filename::{slugify, unique_name};
use crate::id::Id;
use crate::manifest::Manifest;
use crate::project::{Project, ProjectError};
use crate::schema::{SchemaKind, validate};
use crate::storage::{FsStorage, RelPath, Storage, StorageError, is_temp_name};
use crate::text::normalize_text;
use crate::version::{FORMAT_VERSION, READER_VERSION, VersionError, check_readable};

/// The folders a Library always has, in the order they are (re)created.
pub const LIBRARY_DIRS: [&str; 5] = ["Projects", "Inbox", "Templates", "Backups", ".trash"];

const SETTINGS_FILE: &str = "library.json";

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("not a Library: {}", .0.display())]
    NotALibrary(PathBuf),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("the Library settings are not valid: {0}")]
    Invalid(String),
    #[error(transparent)]
    Version(#[from] VersionError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Author {
    pub id: Id,
    pub name: String,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySettings {
    pub format: u32,
    pub min_reader: u32,
    pub author: Author,
    pub settings: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl LibrarySettings {
    /// 2-space pretty JSON with a trailing newline.
    pub(crate) fn to_json(&self) -> String {
        let mut s = serde_json::to_string_pretty(self).expect("settings always serialise");
        s.push('\n');
        s
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    pub folder: String,
    pub name: String,
    pub id: Id,
}

#[derive(Debug)]
pub struct Library {
    pub root: PathBuf,
    pub settings: LibrarySettings,
}

fn rel(name: &str) -> RelPath {
    RelPath::root().join(name)
}

impl Library {
    /// Creates a Library in `root` (made if missing). Refuses a folder that
    /// already has `library.json`, changing nothing.
    pub fn create(root: &Path, author_name: &str) -> Result<Library, LibraryError> {
        let storage = FsStorage::new(root);
        let settings_path = rel(SETTINGS_FILE);
        if storage.exists(&settings_path) {
            return Err(LibraryError::Invalid(format!(
                "{SETTINGS_FILE} already exists in {}",
                root.display()
            )));
        }
        storage.create_dir_all(&RelPath::root())?;
        for dir in LIBRARY_DIRS {
            storage.create_dir_all(&rel(dir))?;
        }
        let settings = LibrarySettings {
            format: FORMAT_VERSION,
            min_reader: READER_VERSION,
            author: Author {
                id: Id::new(),
                name: normalize_text(author_name),
                extra: Map::new(),
            },
            settings: Map::new(),
            extra: Map::new(),
        };
        storage.write_atomic(&settings_path, settings.to_json().as_bytes())?;
        Ok(Library {
            root: root.to_path_buf(),
            settings,
        })
    }

    /// Opens a Library. The second value names the protected folders that
    /// were missing and have been recreated.
    pub fn open(root: &Path) -> Result<(Library, Vec<String>), LibraryError> {
        let storage = FsStorage::new(root);
        let settings_path = rel(SETTINGS_FILE);
        if !storage.exists(&RelPath::root()) || !storage.exists(&settings_path) {
            return Err(LibraryError::NotALibrary(root.to_path_buf()));
        }
        let bytes = storage.read(&settings_path)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| {
            LibraryError::Invalid(format!("{SETTINGS_FILE} is not valid JSON: {e}"))
        })?;
        // A too-new file is reported as such even if its body no longer fits
        // this build's schema.
        if let Some(n) = value.get("minReader").and_then(Value::as_u64) {
            check_readable(u32::try_from(n).unwrap_or(u32::MAX))?;
        }
        validate(SchemaKind::Library, &value).map_err(|errs| {
            LibraryError::Invalid(format!(
                "{SETTINGS_FILE} does not match its schema: {}",
                errs.join("; ")
            ))
        })?;
        let settings: LibrarySettings = serde_json::from_value(value)
            .map_err(|e| LibraryError::Invalid(format!("{SETTINGS_FILE}: {e}")))?;

        let mut recreated = Vec::new();
        for dir in LIBRARY_DIRS {
            let p = rel(dir);
            if !storage.exists(&p) {
                storage.create_dir_all(&p)?;
                recreated.push(dir.to_string());
            }
        }
        Ok((
            Library {
                root: root.to_path_buf(),
                settings,
            },
            recreated,
        ))
    }

    /// Makes a new project in `Projects/<unique slug of name>` and returns
    /// its absolute folder.
    pub fn create_project(&self, name: &str) -> Result<PathBuf, LibraryError> {
        let storage = FsStorage::new(&self.root);
        let projects = rel(LIBRARY_DIRS[0]);
        let existing: Vec<String> = storage
            .list(&projects)?
            .into_iter()
            .map(|e| e.name)
            .collect();
        let folder = unique_name(&slugify(name), "", &existing);
        let project_rel = projects.join(&folder);
        storage.create_dir_all(&project_rel)?;
        let abs = self.root.join(LIBRARY_DIRS[0]).join(&folder);
        match Project::create(FsStorage::new(&abs), name) {
            Ok(_) => Ok(abs),
            Err(ProjectError::Storage(e)) => Err(LibraryError::Storage(e)),
            Err(ProjectError::Version(e)) => Err(LibraryError::Version(e)),
            Err(e) => Err(LibraryError::Invalid(format!(
                "could not create the project: {e}"
            ))),
        }
    }

    /// Every project folder under `Projects/`, sorted by folder name. A
    /// project whose manifest is too new or invalid is still listed when its
    /// name and id can be read, so a writer never loses sight of it.
    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, LibraryError> {
        let storage = FsStorage::new(&self.root);
        let projects = rel(LIBRARY_DIRS[0]);
        let mut entries = storage.list(&projects)?;
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        let mut out = Vec::new();
        for e in entries {
            if !e.is_dir || e.name.starts_with('.') || is_temp_name(&e.name) {
                continue;
            }
            let manifest_path = projects.join(&e.name).join(MANIFEST_FILE);
            if !storage.exists(&manifest_path) {
                continue;
            }
            let Ok(bytes) = storage.read(&manifest_path) else {
                continue;
            };
            let Ok(text) = String::from_utf8(bytes) else {
                continue;
            };
            let summary = match Manifest::load(&text) {
                Ok(m) => Some((m.name, m.id)),
                Err(_) => raw_name_and_id(&text),
            };
            if let Some((name, id)) = summary {
                out.push(ProjectSummary {
                    folder: e.name,
                    name,
                    id,
                });
            }
        }
        Ok(out)
    }
}

/// Reads `name` and `id` straight from the raw JSON of a manifest that
/// `Manifest::load` refused.
fn raw_name_and_id(text: &str) -> Option<(String, Id)> {
    let v: Value = serde_json::from_str(text).ok()?;
    let name = v.get("name")?.as_str()?.to_string();
    let id = Id::parse(v.get("id")?.as_str()?).ok()?;
    Some((name, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brand::MANIFEST_FILE;
    use crate::manifest::Manifest;
    use serde_json::json;
    use std::path::Path;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    const AUTHOR_ID: &str = "01JC3Z8K7QW2M4N6P8R0T2V4X6";

    #[test]
    fn create_refuses_an_existing_library_and_leaves_it_untouched() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("library.json");
        write(&p, "keep me exactly");
        let r = Library::create(d.path(), "G");
        assert!(matches!(r, Err(LibraryError::Invalid(_))));
        assert_eq!(std::fs::read(&p).unwrap(), b"keep me exactly");
        assert!(!d.path().join("Projects").exists());
    }

    #[test]
    fn create_makes_a_missing_root() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("a").join("My Library");
        Library::create(&root, "G").unwrap();
        assert!(root.join("library.json").is_file());
        assert!(root.join("Projects").is_dir());
    }

    #[test]
    fn open_a_missing_root_is_not_a_library() {
        let d = tempfile::tempdir().unwrap();
        assert!(matches!(
            Library::open(&d.path().join("nope")),
            Err(LibraryError::NotALibrary(_))
        ));
    }

    #[test]
    fn too_new_is_reported_before_the_schema_is_checked() {
        let d = tempfile::tempdir().unwrap();
        write(
            &d.path().join("library.json"),
            r#"{"format": 9, "minReader": 99}"#,
        );
        assert!(matches!(
            Library::open(d.path()),
            Err(LibraryError::Version(_))
        ));
    }

    #[test]
    fn broken_json_and_missing_author_are_invalid() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("library.json");
        write(&p, "{ not json");
        assert!(matches!(
            Library::open(d.path()),
            Err(LibraryError::Invalid(_))
        ));
        write(&p, r#"{"format":1,"minReader":1,"settings":{}}"#);
        match Library::open(d.path()) {
            Err(LibraryError::Invalid(msg)) => assert!(!msg.is_empty()),
            other => panic!("expected Invalid, got {other:?}"),
        }
    }

    #[test]
    fn invalid_library_does_not_get_folders_created() {
        let d = tempfile::tempdir().unwrap();
        write(&d.path().join("library.json"), "{ not json");
        let _ = Library::open(d.path());
        assert!(!d.path().join("Projects").exists());
    }

    #[test]
    fn unknown_keys_survive_in_order() {
        let d = tempfile::tempdir().unwrap();
        let text = format!(
            r#"{{"zTop":1,"format":1,"minReader":1,
              "author":{{"zAuth":true,"id":"{AUTHOR_ID}","name":"G","aAuth":[1]}},
              "settings":{{}},"aTop":"x"}}"#
        );
        write(&d.path().join("library.json"), &text);
        let (mut lib, _) = Library::open(d.path()).unwrap();
        lib.settings.author.name = "Renamed".into();
        let out = lib.settings.to_json();
        assert!(out.ends_with("}\n"));
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let keys = |v: &serde_json::Value| -> Vec<String> {
            v.as_object().unwrap().keys().cloned().collect()
        };
        assert_eq!(
            keys(&v),
            ["format", "minReader", "author", "settings", "zTop", "aTop"]
        );
        assert_eq!(keys(&v["author"]), ["id", "name", "zAuth", "aAuth"]);
        assert_eq!(v["author"]["name"], json!("Renamed"));
        assert!(out.contains("\n  \"format\": 1"));
    }

    #[test]
    fn unknown_keys_survive_a_create_modify_open_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let mut lib = Library::create(d.path(), "G").unwrap();
        lib.settings.extra.insert("future".into(), json!({"a": 1}));
        lib.settings.author.extra.insert("pen".into(), json!("nib"));
        write(&d.path().join("library.json"), &lib.settings.to_json());
        let (lib2, recreated) = Library::open(d.path()).unwrap();
        assert!(recreated.is_empty());
        assert_eq!(lib2.settings.extra["future"], json!({"a": 1}));
        assert_eq!(lib2.settings.author.extra["pen"], json!("nib"));
        assert_eq!(lib2.settings.author.id, lib.settings.author.id);
    }

    #[test]
    fn author_name_is_stored_as_nfc() {
        let d = tempfile::tempdir().unwrap();
        let decomposed = "Gonc\u{0327}alo"; // c + combining cedilla
        let lib = Library::create(d.path(), decomposed).unwrap();
        assert_eq!(lib.settings.author.name, "Gon\u{00e7}alo");
        let (lib2, _) = Library::open(d.path()).unwrap();
        assert_eq!(lib2.settings.author.name, "Gon\u{00e7}alo");
    }

    #[test]
    fn several_missing_folders_come_back_in_library_dirs_order() {
        let d = tempfile::tempdir().unwrap();
        Library::create(d.path(), "G").unwrap();
        for n in [".trash", "Projects", "Templates"] {
            std::fs::remove_dir(d.path().join(n)).unwrap();
        }
        let (_lib, recreated) = Library::open(d.path()).unwrap();
        assert_eq!(recreated, vec!["Projects", "Templates", ".trash"]);
        for n in LIBRARY_DIRS {
            assert!(d.path().join(n).is_dir());
        }
    }

    #[test]
    fn list_projects_includes_readable_and_too_new_and_skips_the_rest() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::create(d.path(), "G").unwrap();
        let projects = d.path().join("Projects");

        let good = Manifest::new("Good Novel");
        write(
            &projects.join("b-good").join(MANIFEST_FILE),
            &good.to_json(),
        );

        let mut newer = Manifest::new("From The Future");
        newer.min_reader = 99;
        write(
            &projects.join("a-future").join(MANIFEST_FILE),
            &newer.to_json(),
        );

        write(&projects.join("c-broken").join(MANIFEST_FILE), "{ nope");
        std::fs::create_dir_all(projects.join("d-no-manifest")).unwrap();
        write(
            &projects.join(".hidden").join(MANIFEST_FILE),
            &Manifest::new("Hidden").to_json(),
        );
        write(&projects.join("notes.txt"), "plain file");

        let list = lib.list_projects().unwrap();
        let got: Vec<(&str, &str, &Id)> = list
            .iter()
            .map(|p| (p.folder.as_str(), p.name.as_str(), &p.id))
            .collect();
        assert_eq!(
            got,
            vec![
                ("a-future", "From The Future", &newer.id),
                ("b-good", "Good Novel", &good.id),
            ]
        );
    }

    #[test]
    fn list_projects_without_a_projects_folder_is_a_storage_error() {
        let d = tempfile::tempdir().unwrap();
        let lib = Library::create(d.path(), "G").unwrap();
        std::fs::remove_dir(d.path().join("Projects")).unwrap();
        assert!(matches!(lib.list_projects(), Err(LibraryError::Storage(_))));
    }
}
