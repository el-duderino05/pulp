// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::DocKind, manifest::Node, project::*, storage::*, tree::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, pulp_core::id::Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    (d, p, ms)
}

fn n(tree: &[TreeNode]) -> Vec<(&str, NodeKind)> {
    tree.iter().map(|t| (t.title.as_str(), t.kind)).collect()
}

#[test]
fn new_project_tree_is_manuscript_only() {
    let (_d, p, _) = setup();
    let t = p.tree();
    assert_eq!(n(&t), vec![("Manuscript", NodeKind::Folder)]);
    assert!(t[0].children.is_empty());
}

#[test]
fn tree_keeps_order_and_nesting() {
    let (_d, mut p, ms) = setup();
    p.create_document(&ms, "Chapter 1", DocKind::Prose).unwrap();
    p.create_document(&ms, "Chapter 2", DocKind::Prose).unwrap();
    let part = p.create_folder(Some(&ms), "Part Two").unwrap();
    p.create_document(&part, "Scene", DocKind::Prose).unwrap();
    let t = p.tree();
    let kids = &t[0].children;
    assert_eq!(
        n(kids),
        vec![
            ("Chapter 1", NodeKind::Document),
            ("Chapter 2", NodeKind::Document),
            ("Part Two", NodeKind::Folder)
        ]
    );
    assert_eq!(n(&kids[2].children), vec![("Scene", NodeKind::Document)]);
}

#[test]
fn tree_keeps_unicode_titles() {
    let (_d, mut p, ms) = setup();
    p.create_document(&ms, "Capítulo 1: A Chegada? 🌊", DocKind::Prose)
        .unwrap();
    assert_eq!(p.tree()[0].children[0].title, "Capítulo 1: A Chegada? 🌊");
}

#[test]
fn missing_file_is_missing_kind() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Gone", DocKind::Prose).unwrap();
    std::fs::remove_file(d.path().join(p.path_of(&id).unwrap().as_str())).unwrap();
    let (p, _) = Project::open(FsStorage::new(d.path())).unwrap();
    let node = &p.tree()[0].children[0];
    assert_eq!((node.kind, node.title.as_str()), (NodeKind::Missing, ""));
}

#[test]
fn unreadable_header_uses_file_stem() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Broken", DocKind::Prose).unwrap();
    std::fs::write(d.path().join(p.path_of(&id).unwrap().as_str()), "no header").unwrap();
    let node = &p.tree()[0].children[0];
    assert_eq!(
        (node.kind, node.title.as_str()),
        (NodeKind::Document, "broken")
    );
}
