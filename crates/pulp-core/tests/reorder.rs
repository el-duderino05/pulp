// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::DocKind, id::Id, manifest::Node, project::*, storage::*, tree::*};
use std::path::Path;

fn setup() -> (tempfile::TempDir, Project<FsStorage>, Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    (d, p, ms)
}

fn titles(p: &Project<FsStorage>, folder: usize) -> Vec<String> {
    p.tree()[folder]
        .children
        .iter()
        .map(|t| t.title.clone())
        .collect()
}

fn files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(base: &Path, d: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[test]
fn reorder_moves_a_document_up() {
    let (d, mut p, ms) = setup();
    p.create_document(&ms, "A", DocKind::Prose).unwrap();
    p.create_document(&ms, "B", DocKind::Prose).unwrap();
    let c = p.create_document(&ms, "C", DocKind::Prose).unwrap();
    p.reorder(&c, 0).unwrap();
    assert_eq!(titles(&p, 0), ["C", "A", "B"]);
    let (q, _) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(titles(&q, 0), ["C", "A", "B"]);
}

#[test]
fn reorder_moves_a_folder() {
    let (_d, mut p, ms) = setup();
    let f = p.create_folder(None, "Notes").unwrap();
    p.reorder(&f, 0).unwrap();
    let t = p.tree();
    assert_eq!(t[0].title, "Notes");
    assert_eq!(t[1].id, ms);
    assert_eq!(t[1].kind, NodeKind::Folder);
}

#[test]
fn reorder_clamps_index() {
    let (_d, mut p, ms) = setup();
    let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
    p.create_document(&ms, "B", DocKind::Prose).unwrap();
    p.reorder(&a, 99).unwrap();
    assert_eq!(titles(&p, 0), ["B", "A"]);
}

#[test]
fn reorder_unknown_id_is_not_found() {
    let (d, mut p, _) = setup();
    let before = std::fs::read(d.path().join(MANIFEST_FILE)).unwrap();
    let r = p.reorder(&Id::new(), 0);
    assert!(matches!(r, Err(ProjectError::NotFound(_))));
    assert_eq!(std::fs::read(d.path().join(MANIFEST_FILE)).unwrap(), before);
}

#[test]
fn reorder_never_touches_disk_paths() {
    let (d, mut p, ms) = setup();
    p.create_document(&ms, "A", DocKind::Prose).unwrap();
    let b = p.create_document(&ms, "B", DocKind::Prose).unwrap();
    let before: Vec<_> = files(d.path())
        .into_iter()
        .filter(|f| f != MANIFEST_FILE)
        .collect();
    p.reorder(&b, 0).unwrap();
    let after: Vec<_> = files(d.path())
        .into_iter()
        .filter(|f| f != MANIFEST_FILE)
        .collect();
    assert_eq!(before, after);
}
