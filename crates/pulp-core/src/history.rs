// SPDX-License-Identifier: MPL-2.0
//! Earlier versions of documents and of the manifest, kept in `.history/`,
//! thinned on a schedule, plus operation groups that undo several changes in
//! one step.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Datelike, Duration, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};

use crate::brand::MANIFEST_FILE;
use crate::header::parse_markdown_item;
use crate::id::Id;
use crate::items::{Snapshot as State, companions_of, parse_verified};
use crate::manifest::{Manifest, ManifestError};
use crate::project::{Project, ProjectError};
use crate::storage::{ErrorCause, RelPath, Storage};
use crate::text::normalize_text;

/// What a snapshot is of.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryTarget {
    Item(Id),
    Manifest,
}

/// Why a snapshot was taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SnapshotReason {
    Timer,
    Close,
    Named(String),
    Operation(Id),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: Id,
    pub target: HistoryTarget,
    pub taken_at: DateTime<Utc>,
    pub reason: SnapshotReason,
}

/// What thinning needs to know about a snapshot.
#[derive(Clone, Debug)]
pub struct SnapshotMeta {
    pub id: Id,
    pub taken_at: DateTime<Utc>,
    pub protected: bool,
}

const HISTORY_DIR: &str = ".history";
const GROUPS_DIR: &str = "groups";
const MANIFEST_DIR: &str = "manifest";

/// The JSON shape of a snapshot's `.json`; field order is the file's key order.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetaWire {
    item: String,
    taken_at: String,
    reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    group: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

/// The JSON shape of a group file.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroupWire {
    label: String,
    created_at: String,
    /// `[target, snapshot-id]` pairs; the target is an item id or `"manifest"`.
    snapshots: Vec<(String, Id)>,
}

/// A snapshot whose metadata was read.
struct Stored {
    info: SnapshotInfo,
    /// The item's path when the snapshot was taken (items only).
    path: Option<RelPath>,
    protected: bool,
}

/// What undo needs for one item, all checked before anything changes.
struct ItemStep {
    id: Id,
    current_path: RelPath,
    current_bytes: Vec<u8>,
    snap_bytes: Vec<u8>,
    recorded: RelPath,
}

/// The manifest part of an undo, checked before anything changes.
struct ManifestStep {
    current_bytes: Vec<u8>,
    snap_bytes: Vec<u8>,
    snap: Manifest,
}

/// What an undo has done so far, so a failure can put it all back.
#[derive(Default)]
struct Applied {
    manifest: bool,
    written: usize,
    moves: Vec<Vec<(RelPath, RelPath)>>,
    made_dirs: Vec<RelPath>,
}

fn history_root() -> RelPath {
    RelPath::root().join(HISTORY_DIR)
}

fn target_name(target: &HistoryTarget) -> String {
    match target {
        HistoryTarget::Item(id) => id.as_str().to_string(),
        HistoryTarget::Manifest => MANIFEST_DIR.to_string(),
    }
}

fn target_dir(target: &HistoryTarget) -> RelPath {
    history_root().join(&target_name(target))
}

/// The target a history folder name stands for. Names that are neither
/// `manifest` nor a canonical item id (such as `groups`) are not targets.
fn parse_target(name: &str) -> Option<HistoryTarget> {
    if name == MANIFEST_DIR {
        return Some(HistoryTarget::Manifest);
    }
    let id = Id::parse(name).ok()?;
    (id.as_str() == name).then_some(HistoryTarget::Item(id))
}

fn snap_path(target: &HistoryTarget, id: &Id) -> RelPath {
    target_dir(target).join(&format!("{id}.snap"))
}

fn meta_path(target: &HistoryTarget, id: &Id) -> RelPath {
    target_dir(target).join(&format!("{id}.json"))
}

fn group_path(group: &Id) -> RelPath {
    history_root()
        .join(GROUPS_DIR)
        .join(&format!("{group}.json"))
}

fn manifest_path() -> RelPath {
    RelPath::root().join(MANIFEST_FILE)
}

/// RFC 3339, UTC, whole seconds, with `Z`.
fn timestamp(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn now_secs() -> DateTime<Utc> {
    DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap_or_else(Utc::now)
}

/// 2-space pretty JSON with a trailing newline, in NFC.
fn encode<T: Serialize>(value: &T) -> String {
    let mut s = serde_json::to_string_pretty(value).expect("history JSON always serialises");
    s.push('\n');
    normalize_text(&s)
}

fn load_manifest(bytes: &[u8]) -> Result<Manifest, ProjectError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ManifestError::Json("the manifest is not valid UTF-8".to_string()))?;
    match Manifest::load(text) {
        Ok(m) => Ok(m),
        Err(ManifestError::Version(v)) => Err(ProjectError::Version(v)),
        Err(e) => Err(ProjectError::Manifest(e)),
    }
}

/// Which snapshots thinning keeps (spec section 3.6):
/// - protected snapshots (named, or in an operation group) always;
/// - unprotected ones under 24 hours old;
/// - from 24 hours to 30 days old, the newest per UTC calendar day;
/// - older, the newest per ISO week.
///
/// Protected snapshots never count when choosing the newest of a bucket.
/// Equal times are broken by id, the larger winning.
pub fn snapshots_to_keep(snaps: &[SnapshotMeta], now: DateTime<Utc>) -> HashSet<Id> {
    #[derive(PartialEq, Eq, Hash)]
    enum Bucket {
        Day(NaiveDate),
        Week(i32, u32),
    }
    let mut keep = HashSet::new();
    let mut best: HashMap<Bucket, &SnapshotMeta> = HashMap::new();
    for s in snaps {
        if s.protected {
            keep.insert(s.id.clone());
            continue;
        }
        let age = now - s.taken_at;
        if age < Duration::hours(24) {
            keep.insert(s.id.clone());
            continue;
        }
        let bucket = if age <= Duration::days(30) {
            Bucket::Day(s.taken_at.date_naive())
        } else {
            let week = s.taken_at.iso_week();
            Bucket::Week(week.year(), week.week())
        };
        match best.get(&bucket) {
            Some(b) if (b.taken_at, &b.id) >= (s.taken_at, &s.id) => {}
            _ => {
                best.insert(bucket, s);
            }
        }
    }
    keep.extend(best.values().map(|s| s.id.clone()));
    keep
}

impl<S: Storage> Project<S> {
    /// Stores the current state of `target` and returns the snapshot's id.
    pub fn snapshot(
        &mut self,
        target: HistoryTarget,
        reason: SnapshotReason,
    ) -> Result<Id, ProjectError> {
        self.take_snapshot(&target, &reason)
    }

    /// The snapshots of `target`, newest first (ties: larger id first).
    /// Entries whose metadata or bytes are missing or unreadable are skipped.
    pub fn list_snapshots(
        &self,
        target: &HistoryTarget,
    ) -> Result<Vec<SnapshotInfo>, ProjectError> {
        let mut out = Vec::new();
        for id in self.snapshot_ids(target)? {
            if let Some(stored) = self.read_stored(target, &id)
                && self.storage.exists(&snap_path(target, &id))
            {
                out.push(stored.info);
            }
        }
        out.sort_by(|a, b| (b.taken_at, &b.id).cmp(&(a.taken_at, &a.id)));
        Ok(out)
    }

    /// Puts a snapshot back after taking a `Timer` snapshot of the current
    /// state. An item is written to its current path, never renamed. The
    /// manifest bytes must load, else nothing is written.
    pub fn restore_snapshot(
        &mut self,
        target: &HistoryTarget,
        snapshot: &Id,
    ) -> Result<(), ProjectError> {
        let bytes = self.read_snapshot_bytes(target, snapshot)?;
        match target {
            HistoryTarget::Item(id) => {
                if let Some((raw, path)) = self.raw_item_bytes(id)? {
                    self.write_snapshot(
                        target,
                        &SnapshotReason::Timer,
                        &raw,
                        Some(&path),
                        now_secs(),
                    )?;
                }
                parse_verified(id, &bytes).map_err(|_| ProjectError::NotFound(id.clone()))?;
                let path = self
                    .index
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
                self.storage.write_atomic(&path, &bytes)?;
            }
            HistoryTarget::Manifest => {
                self.take_snapshot(target, &SnapshotReason::Timer)?;
                let manifest = load_manifest(&bytes)?;
                self.storage.write_atomic(&manifest_path(), &bytes)?;
                self.manifest = manifest;
            }
        }
        Ok(())
    }

    /// Snapshots each item and the manifest under one new group, so
    /// `undo_operation` can put them back together. Every item must be in
    /// the index. The group file is written last; if anything fails, the
    /// snapshots made so far are removed again.
    pub fn begin_operation(&mut self, label: &str, items: &[Id]) -> Result<Id, ProjectError> {
        let mut ids: Vec<Id> = Vec::new();
        for id in items {
            if !self.index.contains_key(id) {
                return Err(ProjectError::NotFound(id.clone()));
            }
            if !ids.contains(id) {
                ids.push(id.clone());
            }
        }
        let group = Id::new();
        let mut done: Vec<(HistoryTarget, Id)> = Vec::new();
        match self.write_group(&group, label, &ids, &mut done) {
            Ok(()) => Ok(group),
            Err(e) => {
                for (target, snap) in &done {
                    let _ = self.storage.remove_file(&snap_path(target, snap));
                    let _ = self.storage.remove_file(&meta_path(target, snap));
                }
                Err(e)
            }
        }
    }

    /// Puts every item and the manifest of the group back as they were.
    ///
    /// Everything is checked first; if anything is wrong nothing changes.
    /// Then a `Timer` snapshot of each target is taken, so the undo can
    /// itself be reverted. Each item is written to its current path and, if
    /// that differs from the path recorded in its snapshot, moved back there
    /// (never over another file: a taken name gets a suffix).
    pub fn undo_operation(&mut self, group: &Id) -> Result<(), ProjectError> {
        let (steps, manifest) = self.preflight_undo(group)?;

        let at = now_secs();
        for s in &steps {
            self.write_snapshot(
                &HistoryTarget::Item(s.id.clone()),
                &SnapshotReason::Timer,
                &s.current_bytes,
                Some(&s.current_path),
                at,
            )?;
        }
        if let Some(m) = &manifest {
            self.write_snapshot(
                &HistoryTarget::Manifest,
                &SnapshotReason::Timer,
                &m.current_bytes,
                None,
                at,
            )?;
        }

        let state = self.capture_state();
        let mut applied = Applied::default();
        if let Err(e) = self.apply_undo(&steps, manifest.as_ref(), &mut applied) {
            self.rollback_undo(&steps, manifest.as_ref(), state, &applied);
            return Err(e);
        }
        Ok(())
    }

    /// Deletes the snapshots that `snapshots_to_keep` drops, per item and for
    /// the manifest, and returns how many were removed. A snapshot whose
    /// metadata cannot be read is kept, and so is anything that is not a
    /// snapshot. Group files are never touched.
    pub fn thin_history(&mut self, now: DateTime<Utc>) -> Result<usize, ProjectError> {
        let entries = match self.storage.list(&history_root()) {
            Ok(e) => e,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(0),
            Err(e) => return Err(e.into()),
        };
        let mut removed = 0;
        for entry in entries {
            if !entry.is_dir {
                continue;
            }
            if let Some(target) = parse_target(&entry.name) {
                removed += self.thin_target(&target, now)?;
            }
        }
        Ok(removed)
    }

    fn thin_target(
        &self,
        target: &HistoryTarget,
        now: DateTime<Utc>,
    ) -> Result<usize, ProjectError> {
        let metas: Vec<SnapshotMeta> = self
            .snapshot_ids(target)?
            .iter()
            .filter_map(|id| self.read_stored(target, id))
            .map(|s| SnapshotMeta {
                id: s.info.id,
                taken_at: s.info.taken_at,
                protected: s.protected,
            })
            .collect();
        let keep = snapshots_to_keep(&metas, now);
        let mut removed = 0;
        for m in metas.iter().filter(|m| !keep.contains(&m.id)) {
            self.remove_if_present(&snap_path(target, &m.id))?;
            self.remove_if_present(&meta_path(target, &m.id))?;
            removed += 1;
        }
        Ok(removed)
    }

    fn remove_if_present(&self, p: &RelPath) -> Result<(), ProjectError> {
        match self.storage.remove_file(p) {
            Err(e) if e.cause() != ErrorCause::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// The current bytes of the target, checked: an item's file must hold
    /// that item. Also gives the item's path.
    fn current_bytes(
        &self,
        target: &HistoryTarget,
    ) -> Result<(Vec<u8>, Option<RelPath>), ProjectError> {
        match target {
            HistoryTarget::Item(id) => {
                let path = self
                    .index
                    .get(id)
                    .cloned()
                    .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
                let bytes = self.storage.read(&path)?;
                parse_verified(id, &bytes)?;
                Ok((bytes, Some(path)))
            }
            HistoryTarget::Manifest => Ok((self.storage.read(&manifest_path())?, None)),
        }
    }

    /// The raw bytes of an item's file for a restore safety snapshot, with no
    /// header check: a damaged file is exactly what a restore may be
    /// recovering from. `None` if the file is missing. A file that holds a
    /// different item is never touched (`NotFound`).
    fn raw_item_bytes(&self, id: &Id) -> Result<Option<(Vec<u8>, RelPath)>, ProjectError> {
        let path = self
            .index
            .get(id)
            .cloned()
            .ok_or_else(|| ProjectError::NotFound(id.clone()))?;
        let bytes = match self.storage.read(&path) {
            Ok(b) => b,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let other = std::str::from_utf8(&bytes)
            .ok()
            .and_then(|t| parse_markdown_item(t).ok())
            .is_some_and(|f| f.header.id != *id);
        if other {
            return Err(ProjectError::NotFound(id.clone()));
        }
        Ok(Some((bytes, path)))
    }

    fn take_snapshot(
        &self,
        target: &HistoryTarget,
        reason: &SnapshotReason,
    ) -> Result<Id, ProjectError> {
        let (bytes, path) = self.current_bytes(target)?;
        self.write_snapshot(target, reason, &bytes, path.as_ref(), now_secs())
    }

    /// Writes `<id>.snap` (exact bytes), then `<id>.json`. A snapshot with
    /// no `.json` is not listed, so a failure part-way is cleaned up.
    fn write_snapshot(
        &self,
        target: &HistoryTarget,
        reason: &SnapshotReason,
        bytes: &[u8],
        path: Option<&RelPath>,
        at: DateTime<Utc>,
    ) -> Result<Id, ProjectError> {
        let dir = target_dir(target);
        if !self.storage.exists(&dir) {
            self.storage.create_dir_all(&dir)?;
        }
        let id = Id::new();
        let snap = snap_path(target, &id);
        let meta = meta_path(target, &id);
        for p in [&snap, &meta] {
            if self.storage.exists(p) {
                return Err(ProjectError::Exists(p.clone()));
            }
        }
        let (word, name, group) = match reason {
            SnapshotReason::Timer => ("timer", None, None),
            SnapshotReason::Close => ("close", None, None),
            SnapshotReason::Named(n) => ("named", Some(n.clone()), None),
            SnapshotReason::Operation(g) => ("operation", None, Some(g.clone())),
        };
        let wire = MetaWire {
            item: target_name(target),
            taken_at: timestamp(at),
            reason: word.to_string(),
            name,
            group,
            path: path.map(|p| p.as_str().to_string()),
        };
        self.storage.write_atomic(&snap, bytes)?;
        if let Err(e) = self.storage.write_atomic(&meta, encode(&wire).as_bytes()) {
            let _ = self.storage.remove_file(&snap);
            return Err(e.into());
        }
        Ok(id)
    }

    fn read_snapshot_bytes(
        &self,
        target: &HistoryTarget,
        snap: &Id,
    ) -> Result<Vec<u8>, ProjectError> {
        self.storage
            .read(&snap_path(target, snap))
            .map_err(|e| match e.cause() {
                ErrorCause::NotFound => ProjectError::History(format!("no snapshot {snap}")),
                _ => e.into(),
            })
    }

    /// The ids that have a `.json` in the target's history folder.
    fn snapshot_ids(&self, target: &HistoryTarget) -> Result<Vec<Id>, ProjectError> {
        let entries = match self.storage.list(&target_dir(target)) {
            Ok(e) => e,
            Err(e) if e.cause() == ErrorCause::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        Ok(entries
            .iter()
            .filter(|e| !e.is_dir)
            .filter_map(|e| e.name.strip_suffix(".json"))
            .filter_map(|stem| Id::parse(stem).ok().filter(|id| id.as_str() == stem))
            .collect())
    }

    /// The snapshot's metadata, or `None` if it is missing or cannot be
    /// understood (including when it names a different target).
    fn read_stored(&self, target: &HistoryTarget, id: &Id) -> Option<Stored> {
        let bytes = self.storage.read(&meta_path(target, id)).ok()?;
        let wire: MetaWire = serde_json::from_slice(&bytes).ok()?;
        if wire.item != target_name(target) {
            return None;
        }
        let taken_at = DateTime::parse_from_rfc3339(&wire.taken_at)
            .ok()?
            .with_timezone(&Utc);
        let reason = match wire.reason.as_str() {
            "timer" => SnapshotReason::Timer,
            "close" => SnapshotReason::Close,
            "named" => SnapshotReason::Named(wire.name?),
            "operation" => SnapshotReason::Operation(wire.group.clone()?),
            _ => return None,
        };
        let protected = wire.group.is_some() || matches!(reason, SnapshotReason::Named(_));
        let path = match wire.path {
            Some(p) => Some(RelPath::new(&p).ok()?),
            None => None,
        };
        Some(Stored {
            info: SnapshotInfo {
                id: id.clone(),
                target: target.clone(),
                taken_at,
                reason,
            },
            path,
            protected,
        })
    }

    /// Snapshots the items, then the manifest, then writes the group file.
    /// `done` collects the snapshots written, for clean-up.
    fn write_group(
        &self,
        group: &Id,
        label: &str,
        items: &[Id],
        done: &mut Vec<(HistoryTarget, Id)>,
    ) -> Result<(), ProjectError> {
        let reason = SnapshotReason::Operation(group.clone());
        let mut listed = Vec::new();
        let targets = items
            .iter()
            .map(|id| HistoryTarget::Item(id.clone()))
            .chain(std::iter::once(HistoryTarget::Manifest));
        for target in targets {
            let snap = self.take_snapshot(&target, &reason)?;
            listed.push((target_name(&target), snap.clone()));
            done.push((target, snap));
        }
        let wire = GroupWire {
            label: normalize_text(label),
            created_at: timestamp(now_secs()),
            snapshots: listed,
        };
        let dir = history_root().join(GROUPS_DIR);
        if !self.storage.exists(&dir) {
            self.storage.create_dir_all(&dir)?;
        }
        let path = group_path(group);
        if self.storage.exists(&path) {
            return Err(ProjectError::Exists(path));
        }
        self.storage.write_atomic(&path, encode(&wire).as_bytes())?;
        Ok(())
    }

    /// Reads the group and checks everything undo needs, changing nothing.
    fn preflight_undo(
        &self,
        group: &Id,
    ) -> Result<(Vec<ItemStep>, Option<ManifestStep>), ProjectError> {
        let bytes = self
            .storage
            .read(&group_path(group))
            .map_err(|e| match e.cause() {
                ErrorCause::NotFound => {
                    ProjectError::History(format!("no operation group {group}"))
                }
                _ => e.into(),
            })?;
        let wire: GroupWire = serde_json::from_slice(&bytes)
            .map_err(|e| ProjectError::History(format!("group {group} is unreadable: {e}")))?;

        let mut steps = Vec::new();
        let mut manifest = None;
        for (name, snap) in &wire.snapshots {
            let target = parse_target(name).ok_or_else(|| {
                ProjectError::History(format!("group {group} names an unknown target {name:?}"))
            })?;
            let snap_bytes = self.read_snapshot_bytes(&target, snap)?;
            let (current_bytes, current_path) = self.current_bytes(&target)?;
            match &target {
                HistoryTarget::Manifest => {
                    let loaded = load_manifest(&snap_bytes)?;
                    manifest = Some(ManifestStep {
                        current_bytes,
                        snap_bytes,
                        snap: loaded,
                    });
                }
                HistoryTarget::Item(id) => {
                    parse_verified(id, &snap_bytes)
                        .map_err(|_| ProjectError::NotFound(id.clone()))?;
                    let recorded = self
                        .read_stored(&target, snap)
                        .and_then(|s| s.path)
                        .ok_or_else(|| {
                            ProjectError::History(format!("snapshot {snap} has no readable path"))
                        })?;
                    let current_path =
                        current_path.ok_or_else(|| ProjectError::NotFound(id.clone()))?;
                    steps.push(ItemStep {
                        id: id.clone(),
                        current_path,
                        current_bytes,
                        snap_bytes,
                        recorded,
                    });
                }
            }
        }
        Ok((steps, manifest))
    }

    /// Writes the manifest, then each item's bytes, then moves the items
    /// whose path differs from the recorded one.
    fn apply_undo(
        &mut self,
        steps: &[ItemStep],
        manifest: Option<&ManifestStep>,
        applied: &mut Applied,
    ) -> Result<(), ProjectError> {
        if let Some(m) = manifest {
            self.storage.write_atomic(&manifest_path(), &m.snap_bytes)?;
            applied.manifest = true;
            self.manifest = m.snap.clone();
        }
        for (i, s) in steps.iter().enumerate() {
            self.storage.write_atomic(&s.current_path, &s.snap_bytes)?;
            applied.written = i + 1;
        }
        for s in steps.iter().filter(|s| s.current_path != s.recorded) {
            let dir = s.recorded.parent().unwrap_or_else(RelPath::root);
            if !self.storage.exists(&dir) {
                self.storage.create_dir_all(&dir)?;
                applied.made_dirs.push(dir.clone());
            }
            let target = self.free_path_for(&s.recorded, &dir)?;
            let moves = self.plan_moves(&s.current_path, &target);
            self.move_files(&moves)?;
            applied.moves.push(moves);
            self.index.insert(s.id.clone(), target);
        }
        Ok(())
    }

    /// `wanted` if it and its companion names are free; otherwise the first
    /// suffixed name in the same folder for which they are.
    fn free_path_for(&self, wanted: &RelPath, dir: &RelPath) -> Result<RelPath, ProjectError> {
        let free = !self.storage.exists(wanted)
            && companions_of(wanted)
                .iter()
                .all(|c| !self.storage.exists(c));
        if free {
            return Ok(wanted.clone());
        }
        let file = wanted.file_name();
        let (stem, ext) = file.rsplit_once('.').unwrap_or((file, ""));
        let entries = self.entries_of(dir)?;
        Ok(dir.join(&self.pick_name(dir, stem, ext, entries, None, true)))
    }

    /// Best effort: puts files, folders, bytes, manifest and index back as
    /// they were before `apply_undo`.
    fn rollback_undo(
        &mut self,
        steps: &[ItemStep],
        manifest: Option<&ManifestStep>,
        state: State,
        applied: &Applied,
    ) {
        for moves in applied.moves.iter().rev() {
            self.undo_moves(moves);
        }
        for dir in applied.made_dirs.iter().rev() {
            let _ = self.storage.remove_dir(dir);
        }
        for s in steps.iter().take(applied.written) {
            let _ = self.storage.write_atomic(&s.current_path, &s.current_bytes);
        }
        if applied.manifest
            && let Some(m) = manifest
        {
            let _ = self
                .storage
                .write_atomic(&manifest_path(), &m.current_bytes);
        }
        self.restore(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::DocKind;
    use crate::manifest::Node;
    use crate::project::MANIFEST_FILE;
    use crate::storage::FsStorage;
    use chrono::{Duration, TimeZone};
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

    fn read(d: &Path, rel: &str) -> Vec<u8> {
        std::fs::read(d.join(rel)).unwrap()
    }

    fn read_str(d: &Path, rel: &str) -> String {
        String::from_utf8(read(d, rel)).unwrap()
    }

    fn write(d: &Path, rel: &str, bytes: &[u8]) {
        let p = d.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    /// Every file under `root`, with its bytes, sorted by path.
    fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for e in std::fs::read_dir(dir).unwrap() {
                let p = e.unwrap().path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    let rel = p
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }

    fn keys(text: &str) -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        v.as_object().unwrap().keys().cloned().collect()
    }

    fn json(text: &str) -> serde_json::Value {
        serde_json::from_str(text).unwrap()
    }

    /// Rewrites the `takenAt` of a stored snapshot.
    fn set_taken_at(d: &Path, dir: &str, snap: &Id, when: &str) {
        let rel = format!(".history/{dir}/{snap}.json");
        let mut v = json(&read_str(d, &rel));
        v["takenAt"] = serde_json::json!(when);
        let mut s = serde_json::to_string_pretty(&v).unwrap();
        s.push('\n');
        write(d, &rel, s.as_bytes());
    }

    fn meta(id: &Id, taken_at: DateTime<Utc>, protected: bool) -> SnapshotMeta {
        SnapshotMeta {
            id: id.clone(),
            taken_at,
            protected,
        }
    }

    fn utc(y: i32, mo: u32, d: u32, h: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, 0, 0).unwrap()
    }

    // ---- stored shapes --------------------------------------------------

    #[test]
    fn a_named_item_snapshot_has_the_documented_json_and_exact_bytes() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.save_body(&id, "text\n").unwrap();
        let file = read(d.path(), "manuscript/a.md");
        let snap = p
            .snapshot(
                HistoryTarget::Item(id.clone()),
                SnapshotReason::Named("before".into()),
            )
            .unwrap();
        let base = format!(".history/{id}/{snap}");
        assert_eq!(read(d.path(), &format!("{base}.snap")), file);
        let text = read_str(d.path(), &format!("{base}.json"));
        assert_eq!(keys(&text), ["item", "takenAt", "reason", "name", "path"]);
        assert!(text.starts_with("{\n  \"item\": \""), "2-space pretty");
        assert!(text.ends_with("}\n"));
        assert!(!text.contains("protected"));
        let v = json(&text);
        assert_eq!(v["item"], id.as_str());
        assert_eq!(v["reason"], "named");
        assert_eq!(v["name"], "before");
        assert_eq!(v["path"], "manuscript/a.md");
        let at = v["takenAt"].as_str().unwrap();
        assert_eq!(at.len(), 20, "whole seconds: {at}");
        assert!(at.ends_with('Z'));
        assert!(DateTime::parse_from_rfc3339(at).is_ok());
    }

    #[test]
    fn timer_and_close_snapshots_have_no_name_and_no_group() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        for (reason, word) in [
            (SnapshotReason::Timer, "timer"),
            (SnapshotReason::Close, "close"),
        ] {
            let snap = p.snapshot(HistoryTarget::Item(id.clone()), reason).unwrap();
            let text = read_str(d.path(), &format!(".history/{id}/{snap}.json"));
            assert_eq!(keys(&text), ["item", "takenAt", "reason", "path"]);
            assert_eq!(json(&text)["reason"], word);
        }
    }

    #[test]
    fn operation_snapshots_and_the_group_file_have_the_documented_json() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let file = read(d.path(), "manuscript/a.md");
        let manifest_file = read(d.path(), MANIFEST_FILE);
        let group = p
            .begin_operation("Tidy up", std::slice::from_ref(&a))
            .unwrap();

        let item_snap = p.list_snapshots(&HistoryTarget::Item(a.clone())).unwrap();
        assert_eq!(item_snap.len(), 1);
        assert_eq!(
            item_snap[0].reason,
            SnapshotReason::Operation(group.clone())
        );
        let isid = item_snap[0].id.clone();
        let text = read_str(d.path(), &format!(".history/{a}/{isid}.json"));
        assert_eq!(keys(&text), ["item", "takenAt", "reason", "group", "path"]);
        assert!(!text.contains("protected"));
        let v = json(&text);
        assert_eq!(v["reason"], "operation");
        assert_eq!(v["group"], group.as_str());
        assert_eq!(v["path"], "manuscript/a.md");
        assert_eq!(read(d.path(), &format!(".history/{a}/{isid}.snap")), file);

        let m_snap = p.list_snapshots(&HistoryTarget::Manifest).unwrap();
        assert_eq!(m_snap.len(), 1);
        let msid = m_snap[0].id.clone();
        let text = read_str(d.path(), &format!(".history/manifest/{msid}.json"));
        assert_eq!(keys(&text), ["item", "takenAt", "reason", "group"]);
        assert!(!text.contains("protected"));
        let v = json(&text);
        assert_eq!(v["item"], "manifest");
        assert_eq!(v["group"], group.as_str());
        assert_eq!(
            read(d.path(), &format!(".history/manifest/{msid}.snap")),
            manifest_file
        );

        let text = read_str(d.path(), &format!(".history/groups/{group}.json"));
        assert_eq!(keys(&text), ["label", "createdAt", "snapshots"]);
        assert!(text.ends_with("}\n"));
        assert!(!text.contains("protected"));
        let v = json(&text);
        assert_eq!(v["label"], "Tidy up");
        assert_eq!(
            v["snapshots"],
            serde_json::json!([[a.as_str(), isid.as_str()], ["manifest", msid.as_str()]])
        );
        assert!(DateTime::parse_from_rfc3339(v["createdAt"].as_str().unwrap()).is_ok());
    }

    #[test]
    fn begin_operation_with_an_unknown_item_writes_nothing() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let before = tree(d.path());
        let r = p.begin_operation("op", &[a, Id::new()]);
        assert!(matches!(r, Err(ProjectError::NotFound(_))));
        assert_eq!(tree(d.path()), before);
    }

    // ---- snapshots_to_keep ----------------------------------------------

    #[test]
    fn a_protected_snapshot_does_not_stop_the_newest_unprotected_one_being_kept() {
        let now = utc(2026, 11, 30, 12);
        let named = Id::new();
        let timer = Id::new();
        let older = Id::new();
        // Same UTC day, 10 days old: the named one is the newest of the day.
        let snaps = [
            meta(&named, utc(2026, 11, 20, 18), true),
            meta(&timer, utc(2026, 11, 20, 12), false),
            meta(&older, utc(2026, 11, 20, 8), false),
        ];
        let keep = snapshots_to_keep(&snaps, now);
        assert!(keep.contains(&named));
        assert!(keep.contains(&timer));
        assert!(!keep.contains(&older));
    }

    #[test]
    fn the_24_hour_boundary() {
        let now = utc(2026, 11, 30, 12);
        let young = Id::new();
        let exactly = Id::new();
        let older_same_day = Id::new();
        let snaps = [
            meta(
                &young,
                now - Duration::hours(24) + Duration::seconds(1),
                false,
            ),
            meta(&exactly, now - Duration::hours(24), false),
            meta(
                &older_same_day,
                now - Duration::hours(24) - Duration::minutes(5),
                false,
            ),
        ];
        // `young` is under 24 hours old and kept for its age. The other two
        // are on the 29th: only the newer of them is kept.
        let keep = snapshots_to_keep(&snaps, now);
        assert!(keep.contains(&young));
        assert!(
            keep.contains(&exactly),
            "24 hours old is in the daily range"
        );
        assert!(!keep.contains(&older_same_day));
    }

    #[test]
    fn the_30_day_boundary() {
        let now = utc(2026, 11, 30, 12);
        // Exactly 30 days old is still in the daily range; one second more
        // is in the weekly range. All four fall in the ISO week of 26 Oct.
        let at_30 = Id::new();
        let day_mate = Id::new();
        let over_30 = Id::new();
        let week_mate = Id::new();
        let snaps = [
            meta(&at_30, now - Duration::days(30), false), // 2026-10-31 12:00
            meta(
                &day_mate,
                now - Duration::days(30) - Duration::hours(2),
                false,
            ), // 10:00
            meta(
                &over_30,
                now - Duration::days(30) - Duration::seconds(1),
                false,
            ),
            meta(&week_mate, utc(2026, 10, 30, 8), false), // same ISO week, other day
        ];
        let keep = snapshots_to_keep(&snaps, now);
        assert!(keep.contains(&at_30), "newest of its UTC day");
        // `day_mate` is 30 days and 2 hours old, so it is in the weekly range
        // with `over_30` and `week_mate`; `at_30` does not compete with them.
        assert!(keep.contains(&over_30), "newest of its ISO week");
        assert!(!keep.contains(&day_mate));
        assert!(!keep.contains(&week_mate));
    }

    #[test]
    fn two_snapshots_in_one_iso_week_across_a_month_boundary_keep_only_the_newest() {
        let now = utc(2027, 3, 1, 12);
        // 2026-09-30 (Wednesday) and 2026-10-01 (Thursday): the same ISO week.
        let wed = Id::new();
        let thu = Id::new();
        let other_week = Id::new();
        let snaps = [
            meta(&wed, utc(2026, 9, 30, 9), false),
            meta(&thu, utc(2026, 10, 1, 9), false),
            meta(&other_week, utc(2026, 10, 8, 9), false),
        ];
        let keep = snapshots_to_keep(&snaps, now);
        assert!(!keep.contains(&wed));
        assert!(keep.contains(&thu));
        assert!(keep.contains(&other_week));
    }

    #[test]
    fn equal_times_are_broken_by_id() {
        let now = utc(2026, 11, 30, 12);
        let mut ids = [Id::new(), Id::new()];
        ids.sort();
        let t = utc(2026, 11, 20, 8);
        let snaps = [meta(&ids[0], t, false), meta(&ids[1], t, false)];
        let keep = snapshots_to_keep(&snaps, now);
        assert_eq!(keep, HashSet::from([ids[1].clone()]), "the larger id wins");
        let reversed = [snaps[1].clone(), snaps[0].clone()];
        assert_eq!(
            snapshots_to_keep(&reversed, now),
            keep,
            "input order is irrelevant"
        );
    }

    // ---- restore ----------------------------------------------------------

    #[test]
    fn restoring_bytes_with_another_items_id_is_not_found_and_changes_nothing() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.create_document(&ms, "B", DocKind::Prose).unwrap();
        let snap = p
            .snapshot(HistoryTarget::Item(a.clone()), SnapshotReason::Timer)
            .unwrap();
        let b_bytes = read(d.path(), "manuscript/b.md");
        write(d.path(), &format!(".history/{a}/{snap}.snap"), &b_bytes);
        let before = read(d.path(), "manuscript/a.md");
        let r = p.restore_snapshot(&HistoryTarget::Item(a.clone()), &snap);
        assert!(matches!(r, Err(ProjectError::NotFound(id)) if id == a));
        assert_eq!(read(d.path(), "manuscript/a.md"), before);
        assert_eq!(read(d.path(), "manuscript/b.md"), b_bytes);
    }

    #[test]
    fn restoring_too_new_manifest_bytes_is_an_error_and_changes_nothing() {
        let (d, mut p, _ms) = setup();
        let snap = p
            .snapshot(HistoryTarget::Manifest, SnapshotReason::Named("x".into()))
            .unwrap();
        let current = read(d.path(), MANIFEST_FILE);
        let text = String::from_utf8(current.clone()).unwrap();
        assert!(text.contains("\"minReader\": 1"));
        write(
            d.path(),
            &format!(".history/manifest/{snap}.snap"),
            text.replace("\"minReader\": 1", "\"minReader\": 99")
                .as_bytes(),
        );
        let in_memory = p.manifest().to_json();
        let r = p.restore_snapshot(&HistoryTarget::Manifest, &snap);
        assert!(matches!(
            r,
            Err(ProjectError::Version(_)) | Err(ProjectError::Manifest(_))
        ));
        assert_eq!(read(d.path(), MANIFEST_FILE), current);
        assert_eq!(p.manifest().to_json(), in_memory);
    }

    #[test]
    fn restore_works_when_the_current_file_has_a_broken_header() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.save_body(&id, "good\n").unwrap();
        let snap = p
            .snapshot(
                HistoryTarget::Item(id.clone()),
                SnapshotReason::Named("x".into()),
            )
            .unwrap();
        let broken: &[u8] = b"---\nid: [unclosed\n---\nbroken \xff\n";
        write(d.path(), "manuscript/a.md", broken);
        p.restore_snapshot(&HistoryTarget::Item(id.clone()), &snap)
            .unwrap();
        assert_eq!(p.read_item(&id).unwrap().body, "good\n");
        let safety = p
            .list_snapshots(&HistoryTarget::Item(id.clone()))
            .unwrap()
            .into_iter()
            .find(|s| s.reason == SnapshotReason::Timer)
            .expect("a Timer safety snapshot");
        assert_eq!(
            read(d.path(), &format!(".history/{id}/{}.snap", safety.id)),
            broken
        );
    }

    #[test]
    fn restore_recreates_a_deleted_file_with_no_safety_snapshot() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.save_body(&id, "good\n").unwrap();
        let snap = p
            .snapshot(
                HistoryTarget::Item(id.clone()),
                SnapshotReason::Named("x".into()),
            )
            .unwrap();
        std::fs::remove_file(d.path().join("manuscript/a.md")).unwrap();
        p.restore_snapshot(&HistoryTarget::Item(id.clone()), &snap)
            .unwrap();
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/a.md");
        assert_eq!(p.read_item(&id).unwrap().body, "good\n");
        assert_eq!(p.list_snapshots(&HistoryTarget::Item(id)).unwrap().len(), 1);
    }

    #[test]
    fn restore_never_overwrites_another_items_file() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.create_document(&ms, "B", DocKind::Prose).unwrap();
        let snap = p
            .snapshot(HistoryTarget::Item(a.clone()), SnapshotReason::Timer)
            .unwrap();
        let b_bytes = read(d.path(), "manuscript/b.md");
        write(d.path(), "manuscript/a.md", &b_bytes);
        let r = p.restore_snapshot(&HistoryTarget::Item(a.clone()), &snap);
        assert!(matches!(r, Err(ProjectError::NotFound(id)) if id == a));
        assert_eq!(read(d.path(), "manuscript/a.md"), b_bytes);
        assert_eq!(
            p.list_snapshots(&HistoryTarget::Item(a)).unwrap().len(),
            1,
            "no safety snapshot of another item's file"
        );
    }

    #[test]
    fn restoring_a_manifest_replaces_the_one_in_memory() {
        let (d, mut p, ms) = setup();
        let snap = p
            .snapshot(HistoryTarget::Manifest, SnapshotReason::Named("x".into()))
            .unwrap();
        let before = p.manifest().to_json();
        p.create_folder(Some(&ms), "Part").unwrap();
        assert_ne!(p.manifest().to_json(), before);
        p.restore_snapshot(&HistoryTarget::Manifest, &snap).unwrap();
        assert_eq!(p.manifest().to_json(), before);
        assert_eq!(read_str(d.path(), MANIFEST_FILE), before);
    }

    #[test]
    fn restore_keeps_the_current_file_name_even_when_the_title_differs() {
        let (d, mut p, ms) = setup();
        let id = p.create_document(&ms, "Alpha", DocKind::Prose).unwrap();
        let snap = p
            .snapshot(HistoryTarget::Item(id.clone()), SnapshotReason::Timer)
            .unwrap();
        p.rename_item(&id, "Beta").unwrap();
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/beta.md");
        p.restore_snapshot(&HistoryTarget::Item(id.clone()), &snap)
            .unwrap();
        assert_eq!(p.read_item(&id).unwrap().header.title, "Alpha");
        assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/beta.md");
        assert!(d.path().join("manuscript/beta.md").exists());
        assert!(!d.path().join("manuscript/alpha.md").exists());
    }

    // ---- undo -------------------------------------------------------------

    /// Writes `chapter-01-arrival.md` by renaming on disk, then reopens so
    /// the index finds it by header.
    fn hand_named() -> (tempfile::TempDir, Project<FsStorage>, Id, Id) {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        p.save_body(&a, "a1\n").unwrap();
        drop(p);
        std::fs::rename(
            d.path().join("manuscript/arrival.md"),
            d.path().join("manuscript/chapter-01-arrival.md"),
        )
        .unwrap();
        let (p, _) = Project::open(FsStorage::new(d.path())).unwrap();
        assert_eq!(
            p.path_of(&a).unwrap().as_str(),
            "manuscript/chapter-01-arrival.md"
        );
        (d, p, ms, a)
    }

    #[test]
    fn undo_puts_back_a_hand_named_file() {
        let (d, mut p, _ms, a) = hand_named();
        let group = p
            .begin_operation("rename", std::slice::from_ref(&a))
            .unwrap();
        p.rename_item(&a, "The Arrival").unwrap();
        assert!(d.path().join("manuscript/the-arrival.md").exists());
        p.undo_operation(&group).unwrap();
        assert_eq!(
            p.path_of(&a).unwrap().as_str(),
            "manuscript/chapter-01-arrival.md"
        );
        assert!(d.path().join("manuscript/chapter-01-arrival.md").exists());
        assert!(!d.path().join("manuscript/the-arrival.md").exists());
        assert!(!d.path().join("manuscript/arrival.md").exists());
        assert_eq!(p.read_item(&a).unwrap().header.title, "Arrival");
    }

    #[test]
    fn undo_with_the_recorded_path_taken_uses_a_suffix_and_moves_companions() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        p.save_body(&a, "a1\n").unwrap();
        let group = p
            .begin_operation("rename", std::slice::from_ref(&a))
            .unwrap();
        p.rename_item(&a, "The Arrival").unwrap();
        write(d.path(), "manuscript/the-arrival.comments.json", b"[1]");
        write(d.path(), "manuscript/the-arrival.md.meta.json", b"{}");
        // Another file takes the old name.
        let other = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
        assert_eq!(p.path_of(&other).unwrap().as_str(), "manuscript/arrival.md");
        let other_bytes = read(d.path(), "manuscript/arrival.md");

        p.undo_operation(&group).unwrap();
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/arrival-2.md");
        assert_eq!(read(d.path(), "manuscript/arrival.md"), other_bytes);
        assert_eq!(
            p.read_item(&a).unwrap().body,
            "a1
"
        );
        assert!(!d.path().join("manuscript/the-arrival.md").exists());
        assert_eq!(read(d.path(), "manuscript/arrival-2.comments.json"), b"[1]");
        assert_eq!(read(d.path(), "manuscript/arrival-2.md.meta.json"), b"{}");
        assert!(
            !d.path()
                .join("manuscript/the-arrival.comments.json")
                .exists()
        );
        assert!(
            !d.path()
                .join("manuscript/the-arrival.md.meta.json")
                .exists()
        );
        assert_eq!(p.read_item(&other).unwrap().header.title, "Arrival");
    }

    #[test]
    fn undo_recreates_a_missing_parent_folder() {
        let (d, mut p, ms) = setup();
        let part = p.create_folder(Some(&ms), "Part").unwrap();
        let a = p.create_document(&part, "A", DocKind::Prose).unwrap();
        let group = p.begin_operation("move", std::slice::from_ref(&a)).unwrap();
        p.move_item(&a, Some(&ms), 0).unwrap();
        std::fs::remove_dir(d.path().join("manuscript/part")).unwrap();
        p.undo_operation(&group).unwrap();
        assert_eq!(p.path_of(&a).unwrap().as_str(), "manuscript/part/a.md");
        assert!(d.path().join("manuscript/part/a.md").exists());
        assert!(!d.path().join("manuscript/a.md").exists());
    }

    #[test]
    fn undo_with_a_trashed_item_is_an_error_and_changes_nothing() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let b = p.create_document(&ms, "B", DocKind::Prose).unwrap();
        let group = p.begin_operation("op", &[a.clone(), b.clone()]).unwrap();
        p.save_body(&a, "changed\n").unwrap();
        p.trash_item(&b).unwrap();
        let files = tree(d.path());
        let manifest = p.manifest().to_json();
        let a_path = p.path_of(&a).cloned();

        assert!(p.undo_operation(&group).is_err());
        assert_eq!(tree(d.path()), files, "no file changed, none added");
        assert_eq!(p.manifest().to_json(), manifest);
        assert_eq!(p.path_of(&a).cloned(), a_path);
        assert!(p.path_of(&b).is_none());
    }

    #[test]
    fn undo_with_an_unreadable_group_is_an_error_and_changes_nothing() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let group = p.begin_operation("op", std::slice::from_ref(&a)).unwrap();
        p.save_body(&a, "changed\n").unwrap();
        write(d.path(), &format!(".history/groups/{group}.json"), b"nope");
        let files = tree(d.path());
        assert!(p.undo_operation(&group).is_err());
        assert!(p.undo_operation(&Id::new()).is_err());
        assert_eq!(tree(d.path()), files);
    }

    #[test]
    fn undo_leaves_safety_snapshots_so_it_can_itself_be_reverted() {
        let (_d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.save_body(&a, "one\n").unwrap();
        let group = p.begin_operation("op", std::slice::from_ref(&a)).unwrap();
        p.save_body(&a, "two\n").unwrap();
        p.create_folder(Some(&ms), "Part").unwrap();
        let manifest_after = p.manifest().to_json();

        p.undo_operation(&group).unwrap();
        assert_eq!(p.read_item(&a).unwrap().body, "one\n");

        let snaps = p.list_snapshots(&HistoryTarget::Item(a.clone())).unwrap();
        let safety = snaps
            .iter()
            .find(|s| s.reason == SnapshotReason::Timer)
            .expect("a Timer safety snapshot of the item");
        p.restore_snapshot(&HistoryTarget::Item(a.clone()), &safety.id)
            .unwrap();
        assert_eq!(p.read_item(&a).unwrap().body, "two\n");

        let msnaps = p.list_snapshots(&HistoryTarget::Manifest).unwrap();
        let msafety = msnaps
            .iter()
            .find(|s| s.reason == SnapshotReason::Timer)
            .expect("a Timer safety snapshot of the manifest");
        p.restore_snapshot(&HistoryTarget::Manifest, &msafety.id)
            .unwrap();
        assert_eq!(p.manifest().to_json(), manifest_after);
    }

    // ---- listing and thinning -----------------------------------------

    #[test]
    fn list_snapshots_is_newest_first_with_ties_by_id_and_skips_broken_entries() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let target = HistoryTarget::Item(a.clone());
        let mk =
            |p: &mut Project<FsStorage>| p.snapshot(target.clone(), SnapshotReason::Timer).unwrap();
        let oldest = mk(&mut p);
        let tie_x = mk(&mut p);
        let tie_y = mk(&mut p);
        let no_snap = mk(&mut p);
        let bad_json = mk(&mut p);
        set_taken_at(d.path(), a.as_str(), &oldest, "2026-01-01T00:00:00Z");
        set_taken_at(d.path(), a.as_str(), &tie_x, "2026-02-01T00:00:00Z");
        set_taken_at(d.path(), a.as_str(), &tie_y, "2026-02-01T00:00:00Z");
        set_taken_at(d.path(), a.as_str(), &no_snap, "2026-03-01T00:00:00Z");
        std::fs::remove_file(d.path().join(format!(".history/{a}/{no_snap}.snap"))).unwrap();
        write(
            d.path(),
            &format!(".history/{a}/{bad_json}.json"),
            b"{ not json",
        );

        let got: Vec<Id> = p
            .list_snapshots(&target)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        let mut tied = [tie_x, tie_y];
        tied.sort();
        tied.reverse();
        assert_eq!(got, vec![tied[0].clone(), tied[1].clone(), oldest]);
        assert!(
            p.list_snapshots(&HistoryTarget::Manifest)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn thin_history_removes_old_unprotected_keeps_unreadable_and_groups() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        let target = HistoryTarget::Item(a.clone());
        let dir = a.as_str();
        let t =
            |p: &mut Project<FsStorage>| p.snapshot(target.clone(), SnapshotReason::Timer).unwrap();
        let old_early = t(&mut p);
        let old_late = t(&mut p);
        let unreadable = t(&mut p);
        let named = p
            .snapshot(target.clone(), SnapshotReason::Named("keep".into()))
            .unwrap();
        let group = p.begin_operation("op", std::slice::from_ref(&a)).unwrap();
        set_taken_at(d.path(), dir, &old_early, "2026-01-05T08:00:00Z");
        set_taken_at(d.path(), dir, &old_late, "2026-01-05T18:00:00Z");
        set_taken_at(d.path(), dir, &named, "2026-01-05T20:00:00Z");
        write(
            d.path(),
            &format!(".history/{dir}/{unreadable}.json"),
            b"garbage",
        );

        let now = Utc::now() + Duration::days(400);
        let removed = p.thin_history(now).unwrap();
        assert_eq!(removed, 1);
        let gone = |s: &Id| {
            !d.path().join(format!(".history/{dir}/{s}.snap")).exists()
                && !d.path().join(format!(".history/{dir}/{s}.json")).exists()
        };
        assert!(gone(&old_early));
        assert!(!gone(&old_late));
        assert!(
            d.path()
                .join(format!(".history/{dir}/{unreadable}.snap"))
                .exists()
        );
        assert!(
            d.path()
                .join(format!(".history/{dir}/{unreadable}.json"))
                .exists()
        );
        assert!(!gone(&named));
        assert!(
            d.path()
                .join(format!(".history/groups/{group}.json"))
                .exists()
        );
        let listed: Vec<Id> = p
            .list_snapshots(&target)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert!(listed.contains(&named) && listed.contains(&old_late));
        assert!(!listed.contains(&old_early));
        // The operation snapshot is still there too (it is protected).
        assert!(listed.iter().any(|s| {
            let v = json(&read_str(d.path(), &format!(".history/{dir}/{s}.json")));
            v["group"] == group.as_str()
        }));
    }

    #[test]
    fn thin_history_leaves_unclassifiable_files_alone() {
        let (d, mut p, ms) = setup();
        let a = p.create_document(&ms, "A", DocKind::Prose).unwrap();
        p.snapshot(HistoryTarget::Item(a), SnapshotReason::Timer)
            .unwrap();
        write(d.path(), ".history/stray.txt", b"x");
        write(d.path(), ".history/not-an-id/x.json", b"{}");
        write(d.path(), ".history/manifest/readme.txt", b"x");
        let before = tree(d.path());
        let now = Utc::now() + Duration::days(400);
        assert_eq!(p.thin_history(now).unwrap(), 0);
        assert_eq!(tree(d.path()), before);
    }
}
