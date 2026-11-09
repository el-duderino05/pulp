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
fn create_document_writes_file_header_and_manifest_entry() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/arrival.md");
    assert!(d.path().join("manuscript/arrival.md").is_file());
    assert_eq!(p.read_item(&id).unwrap().header.title, "Arrival");
    assert_eq!(p.manifest().item_ids(), vec![id]);
}

#[test]
fn case_and_accent_variants_get_distinct_files() {
    let (_d, mut p, ms) = setup();
    let a = p.create_document(&ms, "Ana", DocKind::Prose).unwrap();
    let b = p.create_document(&ms, "ana", DocKind::Prose).unwrap();
    let c = p.create_document(&ms, "Aná", DocKind::Prose).unwrap();
    let names: Vec<_> = [a, b, c]
        .iter()
        .map(|i| p.path_of(i).unwrap().as_str().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "manuscript/ana.md",
            "manuscript/ana-2.md",
            "manuscript/ana-3.md"
        ]
    );
}

#[test]
fn save_body_normalises_text_and_keeps_unknown_header_keys() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    let f = d.path().join("manuscript/arrival.md");
    let raw =
        std::fs::read_to_string(&f)
            .unwrap()
            .replacen("---\n", "---\nfutureKey: keep me\n", 1);
    std::fs::write(&f, raw).unwrap();
    p.save_body(&id, "Line 1\r\nAc\u{0327}a\u{0303}o\r\n")
        .unwrap();
    let out = std::fs::read_to_string(&f).unwrap();
    assert!(out.ends_with("---\nLine 1\nAção\n"));
    assert!(out.contains("futureKey: keep me"));
}

#[test]
fn rename_item_renames_file_and_carries_companions() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    std::fs::write(d.path().join("manuscript/arrival.comments.json"), "[]").unwrap();
    let new_path = p.rename_item(&id, "The Arrival").unwrap();
    assert_eq!(new_path.as_str(), "manuscript/the-arrival.md");
    assert!(!d.path().join("manuscript/arrival.md").exists());
    assert!(
        d.path()
            .join("manuscript/the-arrival.comments.json")
            .is_file()
    );
    assert_eq!(p.read_item(&id).unwrap().header.title, "The Arrival");
}

#[test]
fn folders_create_rename_and_items_move_between_them() {
    let (d, mut p, ms) = setup();
    let part = p.create_folder(Some(&ms), "Part One").unwrap();
    assert!(d.path().join("manuscript/part-one").is_dir());
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    p.move_item(&id, Some(&part), 0).unwrap();
    assert_eq!(
        p.path_of(&id).unwrap().as_str(),
        "manuscript/part-one/arrival.md"
    );
    p.rename_folder(&part, "Part I").unwrap();
    assert_eq!(
        p.path_of(&id).unwrap().as_str(),
        "manuscript/part-i/arrival.md"
    );
    assert!(d.path().join("manuscript/part-i/arrival.md").is_file());
}

#[test]
fn unknown_ids_are_reported() {
    let (_d, p, _ms) = setup();
    assert!(matches!(
        p.read_item(&pulp_core::id::Id::new()),
        Err(ProjectError::NotFound(_))
    ));
}
