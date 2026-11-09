// SPDX-License-Identifier: MPL-2.0
use chrono::{Duration, TimeZone, Utc};
use pulp_core::{header::*, history::*, id::Id, manifest::Node, project::*, storage::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] {
        Node::Folder(f) => f.id.clone(),
        _ => unreachable!(),
    };
    (d, p, ms)
}

#[test]
fn snapshot_and_restore_an_item() {
    let (_d, mut p, ms) = setup();
    let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
    p.save_body(&id, "first\n").unwrap();
    let snap = p
        .snapshot(
            HistoryTarget::Item(id.clone()),
            SnapshotReason::Named("before".into()),
        )
        .unwrap();
    p.save_body(&id, "second\n").unwrap();
    p.restore_snapshot(&HistoryTarget::Item(id.clone()), &snap)
        .unwrap();
    assert_eq!(p.read_item(&id).unwrap().body, "first\n");
    // restoring took a safety snapshot of "second" first
    assert_eq!(p.list_snapshots(&HistoryTarget::Item(id)).unwrap().len(), 2);
}

#[test]
fn thinning_keeps_the_documented_set() {
    let now = Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap();
    let m = |h: Duration, protected| SnapshotMeta {
        id: Id::new(),
        taken_at: now - h,
        protected,
    };
    let at = |y, mo, d, h, protected| SnapshotMeta {
        id: Id::new(),
        taken_at: Utc.with_ymd_and_hms(y, mo, d, h, 0, 0).unwrap(),
        protected,
    };
    let a = m(Duration::hours(1), false);
    let b = m(Duration::hours(23), false);
    let c = at(2026, 11, 20, 8, false); // same day as d, older
    let d = at(2026, 11, 20, 18, false);
    let e = at(2026, 9, 1, 10, false); // same ISO week as f, older
    let f = at(2026, 9, 2, 10, false);
    let g = at(2026, 9, 1, 9, true); // named: always kept
    let all = vec![
        a.clone(),
        b.clone(),
        c.clone(),
        d.clone(),
        e.clone(),
        f.clone(),
        g.clone(),
    ];
    let keep = snapshots_to_keep(&all, now);
    let expected: std::collections::HashSet<_> =
        [a.id, b.id, d.id, f.id, g.id].into_iter().collect();
    assert_eq!(keep, expected);
    assert!(!keep.contains(&c.id) && !keep.contains(&e.id));
}

#[test]
fn operation_group_undo_restores_items_manifest_and_file_names() {
    let (d, mut p, ms) = setup();
    let a = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    let b = p.create_document(&ms, "Storm", DocKind::Prose).unwrap();
    p.save_body(&a, "a1\n").unwrap();
    p.save_body(&b, "b1\n").unwrap();
    let manifest_before = p.manifest().to_json();
    let group = p
        .begin_operation("Rename and edit", &[a.clone(), b.clone()])
        .unwrap();
    p.rename_item(&a, "The Arrival").unwrap();
    p.save_body(&b, "b2\n").unwrap();
    p.undo_operation(&group).unwrap();
    assert_eq!(p.read_item(&a).unwrap().header.title, "Arrival");
    assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/arrival.md");
    assert!(!d.path().join("manuscript/the-arrival.md").exists());
    assert_eq!(p.read_item(&b).unwrap().body, "b1\n");
    assert_eq!(p.manifest().to_json(), manifest_before);
}

#[test]
fn thin_history_never_removes_operation_snapshots() {
    let (_d, mut p, ms) = setup();
    let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
    p.begin_operation("op", std::slice::from_ref(&a)).unwrap();
    let far_future = Utc::now() + Duration::days(400);
    p.thin_history(far_future).unwrap();
    assert_eq!(p.list_snapshots(&HistoryTarget::Item(a)).unwrap().len(), 1);
}
