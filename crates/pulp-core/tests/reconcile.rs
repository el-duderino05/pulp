// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, project::*, reconcile::is_sync_conflict_name, storage::*};
use std::path::Path;

fn write_item_with(root: &Path, rel: &str, header: ItemHeader, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(
        p,
        render_markdown_item(&ItemFile {
            header,
            body: body.into(),
        }),
    )
    .unwrap();
}

/// Sets the file's modified time one hour in the past, so it is clearly the
/// older of two files whatever the file system's time resolution.
fn make_older(root: &Path, rel: &str) {
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(root.join(rel))
        .unwrap();
    f.set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(3600))
        .unwrap();
}

fn new_project() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    Project::create(FsStorage::new(d.path()), "X").unwrap();
    d
}

#[test]
fn found_items_are_added_where_they_sit_on_disk() {
    let d = new_project();
    let h = ItemHeader::new_document("Lost", DocKind::Prose);
    let id = h.id.clone();
    write_item_with(d.path(), "manuscript/part-two/lost.md", h, "x\n");
    let (p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.found, vec![id.clone()]);
    let (folder, _) = p.manifest().parent_of(&id).unwrap();
    let folder = p.manifest().folder(&folder.unwrap()).unwrap();
    assert_eq!(folder.path, "manuscript/part-two");
    assert_eq!(folder.title, "part-two");
}

#[test]
fn missing_files_are_reported_but_kept_in_manifest() {
    let d = new_project();
    let id = {
        let (mut p, _) = Project::open(FsStorage::new(d.path())).unwrap();
        let ms = p
            .manifest()
            .folder_by_path("manuscript")
            .unwrap()
            .id
            .clone();
        p.create_document(&ms, "Arrival", DocKind::Prose).unwrap()
    };
    std::fs::remove_file(d.path().join("manuscript/arrival.md")).unwrap();
    let (p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.missing, vec![id.clone()]);
    assert!(p.manifest().item_ids().contains(&id));
}

#[test]
fn identical_duplicates_send_the_newer_file_to_trash() {
    let d = new_project();
    let h = ItemHeader::new_document("A", DocKind::Prose);
    write_item_with(d.path(), "manuscript/a.md", h.clone(), "same\n");
    write_item_with(d.path(), "manuscript/a-copy.md", h, "same\n");
    make_older(d.path(), "manuscript/a.md");
    let (_p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.duplicates_trashed.len(), 1);
    assert_eq!(
        report.duplicates_trashed[0].as_str(),
        "manuscript/a-copy.md"
    );
    assert!(d.path().join("manuscript/a.md").is_file());
    assert!(!d.path().join("manuscript/a-copy.md").exists());
}

#[test]
fn sync_conflict_copies_are_reported_not_changed() {
    let d = new_project();
    let h = ItemHeader::new_document("A", DocKind::Prose);
    let id = h.id.clone();
    write_item_with(d.path(), "manuscript/a.md", h.clone(), "mine\n");
    write_item_with(
        d.path(),
        "manuscript/a.sync-conflict-20261106-120000-ABCDEFG.md",
        h,
        "theirs\n",
    );
    let (_p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].0, id);
    assert!(is_sync_conflict_name(
        "Chapter (Gon\u{e7}alo's conflicted copy 2026-11-06).md"
    ));
}

#[test]
fn different_duplicates_give_the_newer_file_a_fresh_id() {
    let d = new_project();
    let h = ItemHeader::new_document("A", DocKind::Prose);
    let id = h.id.clone();
    write_item_with(d.path(), "manuscript/a.md", h.clone(), "original\n");
    write_item_with(d.path(), "manuscript/a-2.md", h, "edited copy\n");
    make_older(d.path(), "manuscript/a.md");
    let (p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.copies_reassigned.len(), 1);
    let (path, new_id) = &report.copies_reassigned[0];
    assert_eq!(path.as_str(), "manuscript/a-2.md");
    assert_ne!(new_id, &id);
    assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/a.md");
    assert_eq!(p.read_item(new_id).unwrap().header.title, "A (copy)");
}
