// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, library::Library, project::*, storage::*};

fn write_item(root: &std::path::Path, rel: &str, title: &str) -> pulp_core::id::Id {
    let h = ItemHeader::new_document(title, DocKind::Prose);
    let id = h.id.clone();
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        p,
        render_markdown_item(&ItemFile {
            header: h,
            body: "x\n".into(),
        }),
    )
    .unwrap();
    id
}

#[test]
fn create_writes_manifest_and_folders() {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "The Storm").unwrap();
    assert!(d.path().join(MANIFEST_FILE).is_file());
    for dir in PROJECT_DIRS {
        assert!(d.path().join(dir).is_dir(), "{dir} missing");
    }
    assert_eq!(p.manifest().name, "The Storm");
}

#[test]
fn library_creates_projects_with_unique_slugs() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::create(d.path(), "G").unwrap();
    let a = lib.create_project("Capítulo Zero").unwrap();
    let b = lib.create_project("Capítulo Zero").unwrap();
    assert!(a.ends_with("Projects/capitulo-zero"));
    assert!(b.ends_with("Projects/capitulo-zero-2"));
    assert_eq!(lib.list_projects().unwrap().len(), 2);
}

#[test]
fn scan_finds_items_by_header_and_skips_hidden_and_temp() {
    let d = tempfile::tempdir().unwrap();
    Project::create(FsStorage::new(d.path()), "X").unwrap();
    let id = write_item(d.path(), "manuscript/arrival.md", "Arrival");
    write_item(d.path(), ".history/old.md", "Old");
    std::fs::write(
        d.path()
            .join("manuscript")
            .join(temp_name_for("arrival.md")),
        "junk",
    )
    .unwrap();
    let found = scan(&FsStorage::new(d.path())).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, id);
    assert_eq!(found[0].path.as_str(), "manuscript/arrival.md");
}

#[test]
fn plain_markdown_without_header_is_ignored_and_untouched() {
    let d = tempfile::tempdir().unwrap();
    Project::create(FsStorage::new(d.path()), "X").unwrap();
    let readme = d.path().join("notes/README.md");
    std::fs::write(&readme, "Just a readme\r\n").unwrap();
    let (p, _) = Project::open(FsStorage::new(d.path())).unwrap();
    assert!(p.manifest().item_ids().is_empty());
    assert_eq!(std::fs::read(&readme).unwrap(), b"Just a readme\r\n");
}

#[test]
fn open_recreates_missing_history_and_trash() {
    let d = tempfile::tempdir().unwrap();
    Project::create(FsStorage::new(d.path()), "X").unwrap();
    std::fs::remove_dir(d.path().join(".trash")).unwrap();
    let (_p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.recreated_dirs, vec![".trash".to_string()]);
}

#[test]
fn open_refuses_projects_needing_a_newer_pulp() {
    let d = tempfile::tempdir().unwrap();
    Project::create(FsStorage::new(d.path()), "X").unwrap();
    let mp = d.path().join(MANIFEST_FILE);
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&mp).unwrap()).unwrap();
    v["minReader"] = serde_json::json!(99);
    std::fs::write(&mp, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(Project::open(FsStorage::new(d.path())).is_err());
}
