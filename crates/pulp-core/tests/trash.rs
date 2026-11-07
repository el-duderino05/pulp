// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, manifest::Node, project::*, storage::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, pulp_core::id::Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    (d, p, ms)
}

#[test]
fn trash_item_moves_file_and_companions_and_removes_from_tree() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[]").unwrap();
    let entry = p.trash_item(&id).unwrap();
    assert!(!d.path().join("manuscript/arrival.md").exists());
    assert!(!d.path().join("manuscript/arrival.comments.json").exists());
    let dir = d.path().join(".trash").join(entry.as_str());
    assert!(dir.join("arrival.md").is_file() && dir.join("entry.json").is_file());
    assert!(p.manifest().item_ids().is_empty());
    assert_eq!(p.list_trash().unwrap().len(), 1);
}

#[test]
fn restore_returns_item_to_its_folder_and_position() {
    let (d, mut p, ms) = setup();
    let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
    let b = p.create_document(&ms, "B", DocKind::Prose).unwrap();
    let entry = p.trash_item(&a).unwrap();
    p.restore_from_trash(&entry).unwrap();
    assert_eq!(p.manifest().item_ids(), vec![a.clone(), b]);
    assert!(d.path().join("manuscript/a.md").is_file());
    assert!(p.list_trash().unwrap().is_empty());
}

#[test]
fn restore_avoids_name_clashes() {
    let (_d, mut p, ms) = setup();
    let a = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    let entry = p.trash_item(&a).unwrap();
    p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    p.restore_from_trash(&entry).unwrap();
    assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/arrival-2.md");
}

#[test]
fn restore_goes_to_top_level_when_folder_is_gone() {
    let (_d, mut p, ms) = setup();
    let part = p.create_folder(Some(&ms), "Part One").unwrap();
    let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
    let entry = p.trash_item(&a).unwrap();
    p.trash_folder(&part).unwrap();
    p.restore_from_trash(&entry).unwrap();
    assert_eq!(p.manifest().parent_of(&a).unwrap().0, None);
    assert_eq!(p.path_of(&a).unwrap().as_str(), "a.md");
}

#[test]
fn trash_and_restore_whole_folder() {
    let (d, mut p, ms) = setup();
    let part = p.create_folder(Some(&ms), "Part One").unwrap();
    let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
    let entry = p.trash_folder(&part).unwrap();
    assert!(!d.path().join("manuscript/part-one").exists());
    assert!(p.manifest().item_ids().is_empty());
    p.restore_from_trash(&entry).unwrap();
    assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part-one/a.md");
}
