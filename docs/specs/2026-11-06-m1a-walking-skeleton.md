# Milestone 1a — Walking Skeleton: Detailed Spec

**Status:** Draft for review · **Date:** 2026-11-06 · **Parent:** `docs/specs/pulp-master-design.md` (v8.1), §25 row 1a

Goal: a Pulp you can write in. One Library, projects with a tree of folders and prose documents, an editor with Write/Revise shells, autosave, snapshots and trash, all on the file foundations that every later milestone depends on.

This spec only adds what the master spec leaves open for 1a: architecture, exact file formats and the split into plans. Where it is silent, the master spec governs.

---

## 1. Delivery: four plans

| Plan | Delivers | Depends on |
|---|---|---|
| **1. Setup and core file engine** | Dev environment, repository and community files, CI, and the Rust `pulp-core` library: IDs, text and file-name rules, storage, versions, headers, manifest, Library, projects, reconcile rules, item operations, snapshots, operation groups, trash | — |
| **2. Pulp Markdown** | The written Pulp Markdown definition (prose subset for 1a) and the TypeScript converter between editor and Markdown body, with the round-trip test suite | Plan 1 (header handled by core) |
| **3. App shell** | Tauri commands over `pulp-core`, command registry, first launch, Library and project screens (a new project opens straight into a first document, "Chapter 1", cursor ready), tree, editor with Write/Revise shells, autosave wiring, snapshot timer, trash view, conflict-free single-device use | Plans 1 and 2 |
| **4. Technical tests** | Spell-check speed (JavaScript vs Rust), ODT/RTF import quality, Android open-and-save | Plan 1 |

Plan 4 can run in parallel with Plans 2 and 3. Each plan ends with green tests on Windows, macOS and Linux in CI.

---

## 2. Architecture

```
pulp/                         (repository root, Cargo workspace)
├── Cargo.toml                workspace: crates/pulp-core, src-tauri
├── crates/pulp-core/         ALL file logic. No Tauri, no UI. Unit-tested.
├── src-tauri/                thin Tauri app: commands that call pulp-core
├── src/                      React + TypeScript interface (Plans 2–3)
├── docs/specs/               master spec and milestone specs
├── docs/superpowers/plans/   implementation plans
└── .github/                  CI workflow, issue templates
```

- **The core never touches absolute paths directly.** Every file operation goes through the `Storage` trait using `RelPath` (forward-slash, relative to the Library or project root). Desktop uses `FsStorage`; Android and iPad get their own implementations later (master §2, §24).
- **The core treats document bodies as opaque text.** It owns headers, file names, the manifest and safety; Plan 2's TypeScript converter owns the body's Markdown.
- **Name-bearing identifiers live in one place.** "Pulp" is a working name (master §23), but it appears in on-disk formats: the manifest file name, the `pulp:` link scheme, temp files, and later the `.pulp` and `.pulptemplate` extensions. All are constants in `pulp-core`'s `brand` module, so a rename before release is a one-file change.
- **The core is synchronous and single-threaded per project.** The app (Plan 3) serialises calls per project.

---

## 3. File formats (format version 1)

### 3.1 IDs

ULIDs: 26 characters, Crockford base32, uppercase (for example `01JC3Z8K7QW2M4N6P8R0T2V4X6`). Used for items, folders, projects, authors, devices, snapshots, trash entries and operation groups. Links are `pulp:<ULID>`.

### 3.2 Item header (Markdown files)

YAML front matter, keys in this order, unknown keys preserved in their original order after the known ones:

```yaml
---
format: 1
minReader: 1
id: 01JC3Z8K7QW2M4N6P8R0T2V4X6
type: document          # document | profile | note
kind: prose             # documents only: prose | poetry | screenplay | stage-play
title: Arrival
status: Draft           # optional
labels: []
synopsis: ""
created: 2026-11-06T17:00:00Z
---
```

- No `modified` key: modification time comes from the file system, so saving doesn't churn the header.
- Word count is computed, never stored.
- A `.md` file without a valid header is **not a Pulp item**: it is ignored and never modified in 1a ("New project from folder" handles such files in 1b).

### 3.3 Manifest (`pulp-project.json`)

```json
{
  "format": 1,
  "minReader": 1,
  "id": "01JC…",
  "name": "The Storm",
  "tree": [
    { "node": "folder", "id": "01JC…", "title": "Manuscript", "path": "manuscript",
      "synopsis": "", "children": [ { "node": "item", "id": "01JC…" } ] }
  ],
  "statuses": ["Idea", "Draft", "Revised", "Final"],
  "labels": [],
  "settings": {}
}
```

- Items are referenced **by ID only**; their paths come from scanning file headers (master §4.2).
- Folder `path` is relative to the project root and always matches a disk folder.
- Unknown keys anywhere are preserved.
- JSON is written pretty-printed with 2-space indentation and a trailing newline (stable diffs).

### 3.4 Library settings (`library.json` at the Library root)

```json
{ "format": 1, "minReader": 1,
  "author": { "id": "01JC…", "name": "Gonçalo" },
  "settings": {} }
```

A folder without `library.json` is not a Library: opening it fails with `NotALibrary`, so the app asks the user to locate the real one (master §4.9).

### 3.5 New project layout

`create_project` makes: `pulp-project.json`, `manuscript/` (as the tree's first folder, title "Manuscript"), `notes/`, `assets/`, `shelf/`, `.history/`, `.trash/`. Only `manuscript` is a tree folder at creation; the others are created on disk for later milestones.

### 3.6 History (`.history/`)

- Item snapshot: `.history/<item-id>/<snapshot-id>.snap` (exact file bytes) plus `<snapshot-id>.json` (`{ "item", "path", "takenAt", "reason", "name"?, "group"? }`). `path` is the item's relative path at snapshot time, stored exactly as on disk (not normalised); undo puts the file back there.
- Manifest snapshots use the directory `.history/manifest/`.
- Operation group: `.history/groups/<group-id>.json` (`{ "label", "createdAt", "snapshots": [[target, snapshot-id], …] }`).
- **Thinning** (master §16): keep everything under 24 hours old; between 24 hours and 30 days keep the newest per UTC calendar day; older keep the newest per ISO week; **named snapshots and snapshots belonging to an operation group are never thinned**.

### 3.7 Trash (`.trash/`)

`.trash/<entry-id>/` holds the item's file, its companions, and `entry.json`:
`{ "format": 1, "itemId", "originalPath", "originalFolder", "index", "deletedAt", "files": [...], "folderNode"? }`.
`folderNode` is present only when a whole folder was trashed; it holds that folder's manifest node, so restoring puts the folder back with its contents and order.

Restoring returns the item to its original folder and position (clamped), or to the top level if that folder no longer exists, using the file-name rule on clashes. Permanent deletion and the Library-level trash are Plan 3.

### 3.8 Companion files

An item at `dir/name.md` has possible companions `dir/name.comments.json` (Milestone 2) and, for assets, `dir/file.ext.meta.json` (1b). From 1a, every move, rename, trash and restore carries existing companions along (master §4.2).

### 3.9 Temporary files and retries

- Atomic save writes `~$<file-name>.pulp.tmp` in the same folder, flushes it to disk, then renames it over the target. Names starting with `~$` are treated as temporary by OneDrive and Dropbox.
- A failed write is retried **5 times, 200 ms apart**, then reported with its cause (`Locked`, `DiskFull`, `PermissionDenied`, `Io`). The original file is never touched by a failed save.
- On Windows, a rename or move that fails with error 5 ("access denied", usually antivirus or indexing holding the file for a moment) is retried the same way, including the final rename of a save. If it still fails, it is reported as `PermissionDenied`.

---

## 4. Reconcile rules implemented in 1a

On every project open (master §4.9):

| Case | 1a behaviour |
|---|---|
| Valid item file not in manifest | Added at the end of the folder node whose `path` equals the file's directory (folder nodes created for missing directories, titled with the directory name); reported as **found** |
| Manifest item with no file | Kept in the manifest, reported as **missing** |
| Two files, same ID, identical bytes | The newer file is moved to the project trash; reported |
| Two files, same ID, one with a sync-conflict name (`.sync-conflict-` or `conflicted copy`, case-insensitive) | Both kept, reported as a **conflict** for the conflict screen (1b) |
| Two files, same ID, different content otherwise | The file with the older modification time keeps the ID; the other gets a new ID and " (copy)" appended to its title; reported |
| Project `minReader` above this Pulp | Open refused with `TooNew` (read-only view is Plan 3) |
| Missing standard folders (`.history`, `.trash`) | Recreated; reported |

Presence markers, header re-identification, missing packs and fonts belong to 1b and later.

---

## 5. Out of scope for 1a

Notes and profiles as editors (headers support their `type` already), links, search, themes beyond one default, spell-check in the editor, imports and exports, the conflict screen, backups, presence markers, Android build (only the technical test), comments, Shelf.

---

## 6. Done when

- On Windows, Gonçalo can create a Library and a project (which opens straight into its first document), add folders and documents, write in Write and Revise modes, rename, reorder, trash and restore items, and restore a snapshot.
- Closing Pulp mid-typing loses at most the last ~2 seconds, and never corrupts a file.
- CI is green on Windows, macOS and Linux.
- The four technical tests have written results in `docs/specs/`.

---

## 7. Known limits

Accepted for 1a. Each should be revisited before the public release.

- **Unknown header values can change form on save.** Header keys Pulp does not know are kept, but their values are rewritten in a standard form: `1.10` becomes `1.1`, and comments and quote style are lost.
- **Renames can replace their target.** `Storage::rename` is not a non-replacing rename. The guard is the exists-check before every rename and every new file or folder; nothing is overwritten unless another program creates the target in the instant between check and rename.
- **Two disk folders whose names differ only in Unicode normalisation** (one NFC, one NFD, for example left by a sync tool) under one manifest folder: renaming or trashing the folder acts on one disk folder but updates the index for both, and a later restore of that folder can be refused as a duplicate until the other folder's items are trashed. No content is lost.
- **Restoring a trash entry into an existing tree node** does not check that the node is an item. A hand-edited `entry.json` or manifest whose item id equals a folder id could leave a stray index entry.
