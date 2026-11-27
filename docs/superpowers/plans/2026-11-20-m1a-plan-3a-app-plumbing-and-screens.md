# Milestone 1a, Plan 3a: App Plumbing and Screens — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A clickable Pulp. On first launch you create a Library. You can then create and open projects, build a tree of folders and documents (add, rename, reorder, move, trash), restore or permanently delete from the project and Library trash, and read a document in a placeholder pane. Pulp tells you plainly when files on disk didn't match, and when a project is too new to open.

**Architecture:**
- The **Tauri app crate** (`src-tauri`) is a thin boundary: it exposes `pulp-core` through Tauri commands.
  - Each open project sits behind its own `Mutex`, and every core call runs in `spawn_blocking`. This serialises calls per project without freezing the window.
  - Core errors become one serialisable `AppError { code, detail }`.
- The **React interface** (`src/`) calls those commands through one typed module, `src/app/core.ts`.
  - Every user action is a named command in a **command registry**. Menus, buttons and shortcuts all call the registry.
  - All interface text lives in `src/i18n/en.ts`.
- **Four small core additions** come first, each unit-tested in `pulp-core`:
  - the tree with titles
  - reorder within a folder
  - permanent deletion through the operating system's trash
  - the Library trash for whole projects

**Tech Stack:**
- Tauri 2, React 19 and TypeScript, as already in the repository.
- New Rust dependencies, pinned exactly at the version current when the task runs (`cargo add <crate>@<major>`, then pin `=x.y.z`):
  - `trash` (major 5), in `pulp-core`, for the operating system's trash
  - `tauri-plugin-dialog` (major 2), in `src-tauri`, for folder pickers
  - `pulp-core`, a path dependency of `src-tauri`
- New npm packages, pinned exactly at the version current when the task runs:
  - `@tauri-apps/plugin-dialog` (major 2), a runtime dependency
  - dev dependencies: `jsdom`, `@testing-library/react` and `@testing-library/user-event`
- Vitest, already present, for the interface tests. Tauri's `@tauri-apps/api/mocks` (`mockIPC`) stands in for the backend in them.

**Spec:**
- `docs/specs/2026-11-06-m1a-walking-skeleton.md`
  - §1: the 3a row and the mockup gate
  - §2: "The app serialises calls per project"
  - §3.4: `NotALibrary`
  - §3.7: trash, and permanent deletion in 3a
  - §4: reconcile reports, and the `TooNew` refusal
- `docs/specs/pulp-master-design.md`
  - §3: Library, protected folders, author
  - §4.7: the command registry
  - §4.9: found, missing, Library missing at start
  - §11: storage indicator
  - §16: trash, where permanent delete goes to the operating system's trash
  - §18.1: accessibility
  - §18.3: first launch
- **Approved mockups** (2026-11-20, approved by Gonçalo "as drawn"): https://claude.ai/artifact/KStxt54uWmQRSNN9LJg9sC
  - 1 First launch, 2 Library, 3 Project, 4 Project trash. Screens 5 and 6 (the editor) belong to 3b.
  - Colours, fonts and copy below are taken from them.

## Decisions taken for this plan

Each one is copied into the PR description.

- **D1. A too-new project is refused, not shown read-only.**
  - Why: `Manifest::load` refuses it before anything can be read, and the 3a row says "read-only refusal".
  - How it looks: the Library row keeps its "Read-only" badge. Opening it shows the `TooNew` message (Global Constraints).
  - **This changes one line of approved copy.** The mockup said "You can read it, but not change it". It becomes the refusal text. Confirm with Gonçalo at plan review.
- **D2. Reordering.** Folders reorder only within their own parent. Documents can also move to another folder.
  - Reordering is by the right-click menu (Move up, Move down, Move to folder…). Dragging comes later.
  - Moving a folder to a new parent needs a disk-folder move. That is left for later.
- **D3. Permanent deletion goes to the computer's trash**, through a new `Storage::discard` (master §16).
- **D4. The Library trash holds whole projects only** in 1a, since the Inbox is out of scope.
  - It reuses the project trash's `entry.json` shape from spec §3.7: `itemId` = the project id, `originalPath` = `Projects/<folder>`, `originalFolder` = null, `index` = 0, `files` = the folder name.
  - So no new file format is introduced.
- **D5. Settings for this computer only** live in `device.json`, in Tauri's app config folder:
  - the Library path
  - projects added from elsewhere ("Open a folder…")
  - last-opened times
  - Nothing here is written into the Library.
- **D6. A new project gets a document "Chapter 1"** in Manuscript, and opens with it selected.
  - The Write cursor itself is 3b.
- **D7. Missing items** stay in the tree, greyed, titled "Missing item", with the reason "Its file is missing" (master §4.9).
- **D8. One window** in 1a. The Library and project screens swap inside it.
- **D9. Shortcuts in 3a:** F2 renames and Delete moves to trash. Neither uses Ctrl+Alt (master §18.4).

## Global Constraints

- Every new `.rs`, `.ts` and `.tsx` file starts with an SPDX line for MPL-2.0: `// SPDX-License-Identifier: MPL-2.0`.
- Every commit uses `git commit -s` and ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- **`pulp-core` stays UI-free.** It never depends on Tauri, and all file access in it goes through `Storage`.
- **Nothing in the interface touches the disk directly.** Every file operation is a Tauri command calling `pulp-core`.
- **Text Pulp writes** is UTF-8, LF line endings and NFC (master §4.8). Titles typed by the user are passed to the core unchanged, and the core normalises them.
- **Never lose work silently.**
  - A failed operation leaves the tree as it was and shows a friendly message.
  - Pulp never creates a new Library when the configured one is missing (master §4.9).
- **Accessibility, as drawn in the mockups:**
  - real `<button>`, `<a>`, `<input>` and `<label>` elements
  - `aria-label` on every icon-only button
  - text contrast of at least 4.5:1
  - touch targets of at least 36 px, and 44 px for primary buttons
  - the tree works with the keyboard alone: arrows move, Enter opens, F2 renames, Delete trashes, Shift+F10 or the context-menu key opens the menu
- **Look (from the mockups):**
  - Colours:

    | Role | Colour |
    |---|---|
    | Ground | `#F7F5F0` |
    | Panel | `#EFECE5` |
    | Card | `#FFFFFF` |
    | Ink | `#1F1D1A` |
    | Muted text | `#5E5A53` |
    | Border | `#DDD8CE` |
    | Input border | `#B9B3A7` |
    | Accent | `#2F5D62` |
    | Accent tint (selection) | `#DCE6E4` |
    | Danger text | `#8A2A1F` |
    | Notice background | `#E4ECF4` |
    | Notice text | `#1C3550` |
    | Read-only badge | `#F6EBD3`, with text `#6B4A12` |

  - Fonts: interface in Inter; titles and document text in Gelasio. Both are bundled as local font files under `src/assets/fonts/`, never loaded from the network (master §1, principle 5).
- **Exact copy.** Every string lives in `src/i18n/en.ts`. These keys and texts are fixed:

  | Key | Text |
  |---|---|
  | `error.NotALibrary` | This folder isn't a Pulp Library. Choose the folder that has your Projects folder in it. |
  | `error.TooNew` | This was made with a newer version of Pulp. Update Pulp to open it. Nothing in it has been changed. |
  | `error.PermissionDenied` | Pulp isn't allowed to change this file right now. Another program, often antivirus or a sync app, may be using it. Try again in a moment. |
  | `error.Locked` | Another program is using this file. Close it, or wait for syncing to finish, then try again. |
  | `error.DiskFull` | The disk is full. Free some space, then try again. Nothing was lost. |
  | `error.NotFound` | Pulp couldn't find this item. It may have been moved or deleted outside Pulp. |
  | `error.Exists` | Something with that name is already there, so nothing was changed. |
  | `error.Other` | Something went wrong: {detail} |
  | `library.missing` | Pulp can't find your Library at {path}. If you moved it or unplugged a drive, show Pulp where it is now. |
  | `report.found` | Pulp found {n} document(s) that weren't listed and added them at the end of their folder. |
  | `report.missing` | {n} item(s) have lost their file. They stay in the tree, greyed, until the file comes back. |
  | `report.duplicates` | {n} exact duplicate(s) were moved to the trash. |
  | `report.copies` | {n} copied document(s) were given their own identity and marked "(copy)". |
  | `report.conflicts` | {n} document(s) have two versions from a sync service. Both are kept. |
  | `report.recreated` | Pulp recreated missing folders: {list}. |
  | `trash.confirmDelete` | Delete "{title}" from Pulp? It goes to your computer's trash. |
  | `trash.confirmEmpty` | Delete all {n} item(s) in this trash? They go to your computer's trash. |

  The rest of the copy is taken from the mockups as drawn.
- **CI:**
  - The existing `core` and `web` jobs stay green on Windows, macOS and Linux.
  - Task 5 adds an `app` job: fmt, clippy with `-D warnings`, and `cargo test -p pulp` on all three systems.
- **Tests:** never weaken a test to make it pass.

## Review Focus

1. **Titles with accents, emoji, forbidden characters (`: ? * " < > | \ /`) or great length** must keep their exact title in the tree after creating or renaming. The file name follows the core's rules.
   - Pinned in Task 1 (`tree_keeps_unicode_titles`) and Task 10 (`rename keeps the typed title`).
2. **A file held by OneDrive or antivirus** during rename, move or trash must give the friendly `PermissionDenied` or `Locked` message, and the tree must stay unchanged.
   - Pinned in Task 5 (`storage_causes_map_to_codes`) and Task 12 (`failed action keeps the tree and shows the message`).
3. **The Library folder is missing at start** (drive unplugged, folder renamed). Pulp must show the Locate screen and never create a new Library.
   - Pinned in Task 5 (`startup_reports_missing_library_without_creating_it`) and Task 8 (`missing library offers Locate only`).
4. **Two quick clicks on "+ Document"** must create two documents with distinct file names, with no error and no lost update to the manifest.
   - Pinned in Task 6 (`parallel_creates_are_serialised`).
5. **An empty or spaces-only name** for a project, document, folder or rename is refused in the interface. Renaming to empty restores the old name.
   - Pinned in Task 9 (`create is disabled for a blank name`) and Task 10 (`blank rename restores the old title`).

---

## File Structure

```
pulp/
├── .github/workflows/ci.yml                    new "app" job (Task 5)
├── crates/pulp-core/
│   ├── Cargo.toml                              + trash (Task 3)
│   ├── src/lib.rs                              + pub mod tree; pub mod library_trash (Tasks 1, 4)
│   ├── src/tree.rs                             TreeNode, NodeKind, Project::tree (Task 1)
│   ├── src/items.rs                            + Project::reorder (Task 2)
│   ├── src/storage.rs                          + Storage::discard (Task 3)
│   ├── src/trash.rs                            + delete_from_trash, empty_trash (Task 3)
│   ├── src/library.rs                          ProjectSummary.too_new + Serialize (Task 4)
│   ├── src/library_trash.rs                    Library trash (Task 4)
│   └── tests/{tree,reorder,discard,library_trash}.rs
├── src-tauri/
│   ├── Cargo.toml                              + pulp-core, tauri-plugin-dialog (Task 5)
│   ├── tauri.conf.json                         title "Pulp", 1200×760, min 900×600 (Task 5)
│   ├── capabilities/default.json               + dialog:default (Task 5)
│   └── src/
│       ├── lib.rs                              builder, state, handlers (Tasks 5, 6)
│       ├── error.rs                            AppError, ErrorCode (Task 5)
│       ├── device.rs                           DeviceConfig (Task 5)
│       ├── sync.rs                             sync_service (Task 5)
│       ├── state.rs                            AppState, with_project (Task 6)
│       ├── library_cmds.rs                     Library commands (Tasks 5, 6)
│       └── project_cmds.rs                     project commands (Task 6)
├── tsconfig.json / tsconfig.test.json          split (Task 7)
├── vitest.config.ts                            jsdom for *.test.tsx (Task 7)
└── src/
    ├── main.tsx, App.tsx                       screen switch (Task 7)
    ├── theme.css, assets/fonts/                tokens and bundled fonts (Task 7)
    ├── i18n/en.ts                              all strings, t() (Task 7)
    ├── app/core.ts                             typed invoke wrappers and DTO types (Task 7)
    ├── app/errors.ts                           friendlyMessage (Task 7)
    ├── app/commands.ts                         command registry (Task 7)
    ├── components/                             Icons, ContextMenu, ConfirmDialog, Notice,
    │                                           ErrorBanner, MoveDialog, Tree (Tasks 9–12)
    └── screens/                                Setup, LocateLibrary, Library, LibraryTrash,
                                                Project, ProjectTrash (Tasks 8–11)
```

---

### Task 1: The tree with titles (core)

**Files:**
- Create: `crates/pulp-core/src/tree.rs`, `crates/pulp-core/tests/tree.rs`
- Modify: `crates/pulp-core/src/lib.rs`

**Interfaces:**
- Consumes: `Project::manifest()`, `Project::read_item(&Id) -> Result<ItemFile, _>` (title in `header.title`), `Project::path_of`, `FolderNode.title`
- Produces:
  ```rust
  #[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")]
  pub struct TreeNode { pub id: Id, pub title: String, pub kind: NodeKind, pub children: Vec<TreeNode> }
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)] #[serde(rename_all = "camelCase")]
  pub enum NodeKind { Folder, Document, Missing }
  impl<S: Storage> Project<S> { pub fn tree(&self) -> Vec<TreeNode> }
  ```

- [ ] **Step 1: Write the failing tests** in `tests/tree.rs`, on a temp-dir project made with `Project::create`:
  - `new_project_tree_is_manuscript_only`: the tree is `[Folder "Manuscript", children []]`.
  - `tree_keeps_order_and_nesting`: create Chapter 1, Chapter 2 and folder "Part Two" containing "Scene". Expect the exact titles, kinds and order.
  - `tree_keeps_unicode_titles`: a document titled `"Capítulo 1: A Chegada? 🌊"` appears with exactly that title.
  - `missing_file_is_missing_kind`: delete a document's file with `std::fs`, reopen the project, and that node is `NodeKind::Missing` with title `""`.
  - `unreadable_header_uses_file_stem`: overwrite a document file with `"no header"`. The node is `Document` with the file stem as its title.
- [ ] **Step 2:** Run `cargo test -p pulp-core --test tree`. Expected: FAIL (`tree` not found).
- [ ] **Step 3: Implement `Project::tree`** in `src/tree.rs`. It walks `manifest().tree` recursively:
  - Folders take `FolderNode.title`.
  - Items take the header title through `read_item`.
  - An item with no index entry or no file is `Missing`. An item whose read fails otherwise is `Document`, titled with its file stem.
  - Add `pub mod tree;` to `lib.rs`.
- [ ] **Step 4:** Run `cargo test -p pulp-core`. Expected: all pass. Then run `cargo clippy -p pulp-core --all-targets -- -D warnings` and `cargo fmt -p pulp-core --check`, both clean.
- [ ] **Step 5: Commit:** `feat(core): tree with titles for the interface`

### Task 2: Reorder within a folder (core)

**Files:**
- Modify: `crates/pulp-core/src/items.rs`
- Create: `crates/pulp-core/tests/reorder.rs`

**Interfaces:**
- Consumes: `Manifest::parent_of`, `remove_node`, `insert_node`
- Produces: `impl<S: Storage> Project<S> { pub fn reorder(&mut self, id: &Id, index: usize) -> Result<(), ProjectError> }`
  - Moves the node, item or folder, to `index` in its **current** parent. The index is clamped and counted after removal.
  - It changes only the manifest, written atomically. Nothing moves on disk.

- [ ] **Step 1: Write the failing tests:**
  - `reorder_moves_a_document_up`: with [A, B, C], `reorder(C, 0)` gives [C, A, B]. Reopening keeps it.
  - `reorder_moves_a_folder`: two folders at the top level swap.
  - `reorder_clamps_index`: `reorder(A, 99)` puts A last.
  - `reorder_unknown_id_is_not_found`: the result is `Err(ProjectError::NotFound(_))`, and the manifest bytes are unchanged.
  - `reorder_never_touches_disk_paths`: every file path before equals every path after.
- [ ] **Step 2:** Run `cargo test -p pulp-core --test reorder`. Expected: FAIL.
- [ ] **Step 3: Implement `reorder`** in `items.rs`. Write the manifest the same way `move_item` does.
- [ ] **Step 4:** Run `cargo test -p pulp-core` and clippy. Expected: pass and clean.
- [ ] **Step 5: Commit:** `feat(core): reorder items and folders within their folder`

### Task 3: Permanent deletion through the computer's trash (core)

**Files:**
- Modify: `crates/pulp-core/Cargo.toml` (+ `trash`), `src/storage.rs`, `src/trash.rs`
- Create: `crates/pulp-core/tests/discard.rs`

**Interfaces:**
- Produces:
  - In `trait Storage`: `fn discard(&self, p: &RelPath) -> Result<(), StorageError>;`. It moves a file or a whole folder to the operating system's trash. `FsStorage` implements it with `trash::delete`, and maps failures through the existing OS-error mapping, or to `StorageError::Io` when there is no code.
    - On macOS it uses the `NSFileManager` delete method, not Finder, so it works without the Finder app.
  - `impl<S: Storage> Project<S>`:
    - `pub fn delete_from_trash(&mut self, entry: &Id) -> Result<(), ProjectError>` discards `.trash/<entry>`. An unknown entry gives `NotFound(entry)`.
    - `pub fn empty_trash(&mut self) -> Result<usize, ProjectError>` discards every entry and returns the count.
  - Every existing test `Storage` impl (`Flaky`, `ApfsLike`, `NoRename`) gains `discard` by delegating to its inner storage.

- [ ] **Step 1: Write the failing tests** in `tests/discard.rs`. They use a `Recording` storage wrapping `FsStorage`: its `discard` records the path and removes it with `std::fs::remove_dir_all` or `remove_file`. This keeps CI from depending on a desktop trash.
  - `delete_from_trash_discards_only_that_entry`: trash two documents, delete one entry. `recorded == [".trash/<id>"]`, and `list_trash()` holds the other.
  - `delete_unknown_entry_is_not_found`: the result is `Err(NotFound)`, and nothing is recorded.
  - `empty_trash_discards_all_and_counts`: the result is `Ok(2)`, and `list_trash()` is empty.
  - `#[ignore] fs_discard_moves_to_os_trash`: a real `FsStorage::discard` on a temp file. Afterwards the file no longer exists. Run by hand on Windows: `cargo test -p pulp-core --test discard -- --ignored`.
- [ ] **Step 2:** Run `cargo test -p pulp-core --test discard`. Expected: FAIL.
- [ ] **Step 3: Implement** `Storage::discard`, `FsStorage::discard`, `delete_from_trash` and `empty_trash`, plus `discard` in the existing test storages.
- [ ] **Step 4:** Run `cargo test -p pulp-core` and clippy. Expected: pass and clean. Then run the ignored test once on Windows: the file appears in the Recycle Bin.
- [ ] **Step 5: Commit:** `feat(core): permanent deletion goes to the computer's trash`

### Task 4: Library trash and the too-new flag (core)

**Files:**
- Create: `crates/pulp-core/src/library_trash.rs`, `crates/pulp-core/tests/library_trash.rs`
- Modify: `src/library.rs`, `src/trash.rs` (make `store_in_trash` and `load_entry` usable from `library_trash`, as `pub(crate)` at most), `src/lib.rs`

**Interfaces:**
- Consumes: `TrashEntry`, `store_in_trash`, `load_entry`, `Storage::discard`, `unique_name`
- Produces:
  - `ProjectSummary` gains `pub too_new: bool`, plus `#[derive(Serialize)] #[serde(rename_all = "camelCase")]`. `list_projects` sets it when the manifest's `minReader` is above `READER_VERSION`.
  - `#[derive(Debug, Clone, PartialEq, Serialize)] #[serde(rename_all = "camelCase")] pub struct LibraryTrashEntry { pub entry_id: Id, pub project_id: Id, pub name: String, pub deleted_at: String }`
  - `impl Library`:
    - `pub fn trash_project(&self, folder: &str) -> Result<Id, LibraryError>` moves `Projects/<folder>` into `.trash/<entry>/` with an `entry.json` shaped as decision D4.
    - `pub fn list_trash(&self) -> Result<Vec<LibraryTrashEntry>, LibraryError>` returns the newest first. The name is read from the trashed manifest, falling back to the folder name.
    - `pub fn restore_project(&self, entry: &Id) -> Result<PathBuf, LibraryError>` returns the project to `Projects/`, using a unique folder name on a clash.
    - `pub fn delete_from_trash(&self, entry: &Id) -> Result<(), LibraryError>`
    - `pub fn empty_trash(&self) -> Result<usize, LibraryError>`

- [ ] **Step 1: Write the failing tests:**
  - `trash_and_restore_project_round_trip`: the project leaves `list_projects`, appears in `list_trash` with its name, and comes back after restore. The manifest bytes are identical.
  - `restore_on_clash_gets_unique_folder`: create a new project with the same folder name, then restore. The restored folder is `<name>-2`.
  - `delete_and_empty_use_discard`: run both through the `Recording` storage pattern from Task 3. The entry folder is gone.
  - `too_new_project_is_flagged`: a project whose manifest has `"minReader": 99` is listed with `too_new == true`. Others are `false`.
  - `unknown_entry_is_reported`: `restore_project` and `delete_from_trash` on a random `Id` return an error, and nothing changes.
- [ ] **Step 2:** Run `cargo test -p pulp-core --test library_trash`. Expected: FAIL.
- [ ] **Step 3: Implement.** If `Library` methods need a storage seam for the `Recording` test, add `pub(crate) fn with_storage<S: Storage>` helpers. Keep `FsStorage` as the public path.
- [ ] **Step 4:** Run `cargo test -p pulp-core` and clippy. Expected: pass and clean.
- [ ] **Step 5: Commit:** `feat(core): Library trash for whole projects; flag too-new projects`

### Task 5: App backend: errors, device settings, Library commands, CI

**Files:**
- Modify: `src-tauri/Cargo.toml`, `tauri.conf.json`, `capabilities/default.json`, `src-tauri/src/lib.rs` (remove `greet`), `.github/workflows/ci.yml`
- Create: `src-tauri/src/error.rs`, `device.rs`, `sync.rs`, `library_cmds.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize)] pub struct AppError { pub code: ErrorCode, pub detail: String }`, with `#[serde(rename_all = "camelCase")]`
    - `pub enum ErrorCode { NotALibrary, TooNew, PermissionDenied, Locked, DiskFull, NotFound, Exists, Invalid, Other }` serialises as its variant name.
    - `impl From<LibraryError>` and `impl From<ProjectError> for AppError`:
      - `VersionError::TooNew` maps to `TooNew`.
      - A storage error maps through `cause()`, giving `PermissionDenied`, `Locked`, `DiskFull` or `NotFound`.
      - `ProjectError::Exists` maps to `Exists`, `LibraryError::Invalid` to `Invalid`, and anything else to `Other`.
      - `detail` is the error's `Display` text.
  - `#[derive(Default, Serialize, Deserialize)] pub struct DeviceConfig { pub library_path: Option<PathBuf>, pub external_projects: Vec<PathBuf>, pub last_opened: BTreeMap<String, String> }`
    - `load(path) -> DeviceConfig` returns the default when the file is missing or unreadable.
    - `save(&self, path) -> Result<(), AppError>` writes atomically through `FsStorage::write_atomic`.
  - `pub fn sync_service(path: &Path) -> Option<&'static str>`. It returns `"OneDrive"` for any path component starting `OneDrive`, `"Dropbox"` for `Dropbox`, and `"iCloud Drive"` for `iCloud Drive`, `iCloudDrive` or `Mobile Documents`. It returns `"Syncthing"` when any ancestor holds a `.stfolder` entry.
  - Commands (`async`, with core work in `tauri::async_runtime::spawn_blocking`):
    - `startup() -> Startup`, tagged `#[serde(tag = "state", rename_all = "camelCase")]`, with these cases:
      - `FirstLaunch { default_path: String }`: no `library_path` is set. The default is `<Documents>/Pulp`.
      - `Ready { library: LibraryInfo }`
      - `LibraryMissing { path: String }`: the configured folder doesn't exist.
      - `Failed { error: AppError }`: for example `NotALibrary` or `TooNew`.
    - `create_library(path: String, author: String) -> LibraryInfo` saves `library_path`.
    - `open_library(path: String) -> LibraryInfo` is used by Locate, and saves `library_path`.
    - `LibraryInfo { root: String, author: String, sync: Option<String>, recreated: Vec<String> }`
  - The Library, once open, lives in `AppState.library: Mutex<Option<Library>>`.

- [ ] **Step 1: Write the failing unit tests** (in-module `#[cfg(test)]`, no Tauri runtime):
  - `storage_causes_map_to_codes`: each `ErrorCause` and `TooNew` gives the right `ErrorCode`.
  - `device_config_round_trips_and_defaults_when_missing`
  - `sync_service_detects_known_folders`: Windows-style and Unix-style paths for each service. A plain `Documents/Pulp` is `None`.
  - `startup_reports_missing_library_without_creating_it`: put a temp path that doesn't exist into the config, then call the inner `startup_with(config)` function. The result is `LibraryMissing`, and the path still does not exist.
  - `startup_first_launch_when_unset`
- [ ] **Step 2:** Run `cargo test -p pulp`. Expected: FAIL.
- [ ] **Step 3: Implement** the files above. Each command is a thin wrapper over an inner `fn ..._with(...)` that the tests call.
  - Register `tauri_plugin_dialog::init()`, and add `dialog:default` to the capabilities.
  - Window: title `Pulp`, 1200×760, minimum 900×600.
- [ ] **Step 4: Add the CI `app` job.** It matches `core`, runs `-p pulp`, and on Ubuntu first installs `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`.
  - Run `cargo test -p pulp` and `cargo clippy -p pulp --all-targets -- -D warnings`. Expected: pass and clean.
- [ ] **Step 5: Commit:** `feat(app): error mapping, device settings and Library startup commands`

### Task 6: App backend: project sessions and the remaining commands

**Files:**
- Create: `src-tauri/src/state.rs`, `src-tauri/src/project_cmds.rs`
- Modify: `src-tauri/src/library_cmds.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: Tasks 1–5
- Produces:
  - `AppState { config_path, device: Mutex<DeviceConfig>, library: Mutex<Option<Library>>, projects: Mutex<HashMap<String, Arc<Mutex<Project<FsStorage>>>>> }`, keyed by project id.
    - `fn with_project<T>(state, id, f: impl FnOnce(&mut Project<FsStorage>) -> Result<T, ProjectError>) -> Result<T, AppError>` locks only that project.
  - Library commands:
    - `list_projects() -> Vec<ProjectRow>`, where `ProjectRow { id, name, path, too_new, last_opened: Option<String>, external: bool }` (camelCase)
    - `create_project(name) -> OpenedProject`: creates the project, adds the "Chapter 1" document in Manuscript (D6), and opens it.
    - `add_external_project(path) -> ProjectRow`: refuses a folder without `pulp-project.json` with `Invalid`.
    - `forget_external_project(path)`
    - `trash_project(path)`: only for projects under `Projects/`.
    - `list_library_trash`, `restore_project(entry) -> ProjectRow`, `delete_library_trash_entry(entry)`, `empty_library_trash() -> usize`
  - Project commands (each takes `project: String`, the id):
    - `open_project(path) -> OpenedProject { id, name, path, sync, tree: Vec<TreeNode>, report: ReportView, first_document: Option<Id> }`. It records `last_opened`.
      - `ReportView { found: Vec<String>, missing: Vec<String>, duplicates: Vec<String>, copies: Vec<String>, conflicts: Vec<String>, recreated: Vec<String> }` holds titles or paths, ready to show.
    - `close_project`
    - `project_tree -> Vec<TreeNode>`
    - `create_document(folder: Id, title) -> Id`
    - `create_folder(parent: Option<Id>, title) -> Id`
    - `rename_node(id, title)`: dispatches to `rename_folder` or `rename_item`.
    - `reorder_node(id, index)`
    - `move_document(id, folder: Option<Id>, index)`
    - `trash_node(id) -> Id`: dispatches to `trash_folder` or `trash_item`.
    - `list_project_trash -> Vec<TrashRow { entry_id, title, is_folder, item_count, from: Option<String>, deleted_at }>`. The title comes from the trashed file's header via `parse_markdown_item`, or from `folder_node.title`.
    - `restore_trash_entry(entry)`, `delete_trash_entry(entry)`, `empty_project_trash -> usize`
    - `read_document(id) -> { title, body }`

- [ ] **Step 1: Write the failing unit tests**, calling the inner `_with` functions on a temp Library:
  - `create_project_opens_on_chapter_1`: the tree is Manuscript › "Chapter 1", and `first_document` is that id.
  - `parallel_creates_are_serialised`: two threads each call `create_document_with(.., "Scene")` 20 times on the same project. All 40 ids are in the tree after reopening, and the file names are distinct.
  - `trash_row_titles_come_from_headers`: trash "Chapter 1". The trash row title is `"Chapter 1"`, and `from == Some("Manuscript")`.
  - `too_new_open_is_refused`: the result is `AppError { code: TooNew, .. }`, and the manifest bytes are unchanged.
  - `report_view_names_found_items`: add a valid item file by hand, then open. `found == ["<its title>"]`.
- [ ] **Step 2:** Run `cargo test -p pulp`. Expected: FAIL.
- [ ] **Step 3: Implement**, and register every command in `generate_handler!`.
- [ ] **Step 4:** Run `cargo test -p pulp` and clippy. Expected: pass and clean.
- [ ] **Step 5: Commit:** `feat(app): project sessions and tree, trash and Library commands`

### Task 7: Interface foundation: config split, strings, core client, command registry, theme

**Files:**
- Create: `tsconfig.test.json`, `src/i18n/en.ts`, `src/app/core.ts`, `src/app/errors.ts`, `src/app/commands.ts`, `src/theme.css`, `src/assets/fonts/*` (Inter and Gelasio, OFL, with their licence files)
- Modify: `tsconfig.json`, `package.json` (`typecheck` runs both configs), `vitest.config.ts` (jsdom for `*.test.tsx`), `src/main.tsx`, `src/App.tsx`, and delete `src/App.css`
- Test: `src/app/commands.test.ts`, `src/app/errors.test.ts`, `src/i18n/en.test.ts`

**Interfaces:**
- Produces:
  - `t(key: StringKey, vars?: Record<string, string | number>): string` replaces `{name}` placeholders.
  - `core.ts` provides one typed function per Task 5–6 command, for example `createDocument(project: string, folder: string, title: string): Promise<string>`, using `invoke`. It also exports the DTO types `TreeNode`, `ProjectRow`, `OpenedProject`, `ReportView`, `TrashRow`, `LibraryTrashEntry`, `Startup`, `AppError`.
  - `friendlyMessage(e: unknown): string`. An `AppError` with a known code gives `t('error.<code>')`, `Other` and `Invalid` give `error.Other` with the detail, and anything else gives `error.Other` with `String(e)`.
  - The command registry:
    ```ts
    type Command = { id: string; label: StringKey; shortcut?: string; run: () => void | Promise<void>; enabled?: () => boolean }
    registerCommand(c: Command): () => void   // returns an unregister function
    runCommand(id: string): Promise<void>     // throws on an unknown id; does nothing if disabled
    listCommands(): Command[]
    handleShortcut(e: KeyboardEvent): boolean // true if it ran a command
    ```

- [ ] **Step 1: Write the failing tests:**
  - `commands.test.ts`:
    - register, then `runCommand` calls `run`
    - a disabled command doesn't run
    - an unknown id throws
    - `handleShortcut` runs the command bound to `F2`
    - unregister removes the command
  - `errors.test.ts`: each code from the Global Constraints table gives its exact text, and `Other` includes the detail.
  - `en.test.ts`: `t('report.found', { n: 1 })` equals the exact sentence. Every key used in the table exists.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement.** Then split the TypeScript config, which is the carry-over from PR #4:
  - `tsconfig.json` excludes `src/**/*.test.ts(x)`, `src/markdown/arbitrary.ts` and `src/markdown/test-builders.ts`, and sets no Node types.
  - `tsconfig.test.json` extends it, includes everything, and adds `"types": ["node"]`.
  - Remove the `/// <reference types="node" />` lines.
  - `App.tsx` switches on the `startup()` result between Setup, LocateLibrary, Library and Project, as stubs until Tasks 8–11.
- [ ] **Step 4:** Run `npm run typecheck && npm test && npm run build`. Expected: all pass, and `grep -r "node:fs" dist` finds nothing, so the test helper stays out of the bundle.
- [ ] **Step 5: Commit:** `feat(ui): strings, core client, command registry, theme; split test tsconfig`

### Task 8: First launch and Locate Library screens

**Files:**
- Create: `src/screens/Setup.tsx`, `src/screens/LocateLibrary.tsx`, and their `.test.tsx` files

**Interfaces:**
- Consumes: `createLibrary`, `openLibrary`, and the dialog plugin's `open({ directory: true })`
- Produces: `<Setup defaultPath onDone={(lib: LibraryInfo) => void} />`, `<LocateLibrary path onDone />`

- [ ] **Step 1: Write the failing tests** (`mockIPC`, Testing Library):
  - `setup submits name and path`: type "Gonçalo", keep the default path, and click "Create my Library". `create_library` is called with `{ path: "<default>", author: "Gonçalo" }`.
  - `setup needs a name`: the button is disabled while the name is blank or spaces.
  - `missing library offers Locate only`: the screen shows `t('library.missing')` with the path, and has a "Locate…" button. No button creates a Library.
  - `locating a non-library shows the friendly message`: the backend rejects with `NotALibrary`, and `error.NotALibrary` is shown.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** to match mockup 1. "Choose…" opens the folder picker.
- [ ] **Step 4:** Run `npm run typecheck && npm test`. Expected: pass.
- [ ] **Step 5: Commit:** `feat(ui): first launch and Locate Library`

### Task 9: Library screen and Library trash

**Files:**
- Create: `src/screens/Library.tsx`, `src/screens/LibraryTrash.tsx`, `src/components/{Icons,ContextMenu,ConfirmDialog}.tsx`, and their tests

**Interfaces:**
- Consumes: `listProjects`, `createProject`, `addExternalProject`, `forgetExternalProject`, `trashProject`, `listLibraryTrash`, `restoreProject`, `deleteLibraryTrashEntry`, `emptyLibraryTrash`, and the registry
- Produces: `<Library onOpen={(p: OpenedProject) => void} />`, `<LibraryTrash onBack />`, and `<ConfirmDialog title message confirmLabel danger onConfirm onCancel />`, which Task 11 reuses
- Commands registered: `library.newProject`, `library.openFolder`, `library.showTrash`

- [ ] **Step 1: Write the failing tests:**
  - `lists projects with last opened`: the rows show name and "Opened …", and a too-new row shows the "Read-only" badge.
  - `create is disabled for a blank name`
  - `new project opens it`: `create_project` is called, then `onOpen` with the result.
  - `too-new project shows the refusal`: clicking it shows `error.TooNew`.
  - `row menu trashes a Library project, forgets an external one`
  - `library trash restore and delete-forever confirm first`: "Delete forever…" opens the confirm dialog with `trash.confirmDelete`. Only "Delete" calls the command.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** to match mockup 2. The Library trash uses the layout of mockup 4.
- [ ] **Step 4:** Run `npm run typecheck && npm test`. Expected: pass.
- [ ] **Step 5: Commit:** `feat(ui): Library screen and Library trash`

### Task 10: Project screen: tree, actions and placeholder pane

**Files:**
- Create: `src/screens/Project.tsx`, `src/components/{Tree,MoveDialog,Notice}.tsx`, and their tests

**Interfaces:**
- Consumes: `projectTree`, `createDocument`, `createFolder`, `renameNode`, `reorderNode`, `moveDocument`, `trashNode`, `readDocument`, `closeProject`, and the registry
- Produces: `<Project opened={OpenedProject} onBack />`, `<Tree nodes selectedId onSelect onAction />`, `<Notice report={ReportView} />`
- Commands registered: `project.newDocument`, `project.newFolder`, `project.rename` (F2), `project.moveUp`, `project.moveDown`, `project.moveToFolder`, `project.trash` (Delete), `project.showTrash`, `nav.library`

**Behaviour:**
- After every action, the tree is replaced with the result of `projectTree` and is never patched locally.
- "+ Document" goes into the selected folder, or into the parent of the selected document, or into Manuscript. It is titled "Untitled", then rename opens on it.
- Rename happens inline. Enter commits, Esc cancels, and a blank name restores the old title.
- Move up and Move down call `reorderNode` with index ±1. They are disabled at the ends.
- "Move to folder…" is offered for documents only (D2).
- Trashing the open document clears the pane.
- The pane shows "Preview · the editor arrives in Plan 3b", the title in Gelasio, and the body split on blank lines into paragraphs. The body is shown as text, never as HTML.
- The storage line is "Saved in {path}", with " · inside {sync}" when a sync service is detected.

- [ ] **Step 1: Write the failing tests:**
  - `opens on first document`: Chapter 1 is selected and its body is shown.
  - `rename keeps the typed title`: F2, type `Capítulo 1: A Chegada? 🌊`, Enter. `rename_node` is called with exactly that title.
  - `blank rename restores the old title`
  - `move up is disabled for the first item; reorder sends index-1`
  - `move to folder lists folders and calls move_document`
  - `trash clears the pane when the open item is trashed`
  - `missing items are greyed and can't be opened`
  - `notice shows the report counts and details`, using the exact `report.*` strings
  - `body text is shown as text`: a body with `<b>x</b>` renders those characters literally.
  - `keyboard: arrows move selection, Enter opens, Shift+F10 opens the menu`
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** to match mockup 3.
- [ ] **Step 4:** Run `npm run typecheck && npm test`. Expected: pass.
- [ ] **Step 5: Commit:** `feat(ui): project screen with tree actions and preview pane`

### Task 11: Project trash view

**Files:**
- Create: `src/screens/ProjectTrash.tsx` and its test

**Interfaces:**
- Consumes: `listProjectTrash`, `restoreTrashEntry`, `deleteTrashEntry`, `emptyProjectTrash`, and `ConfirmDialog` from Task 9
- Produces: `<ProjectTrash project={string} onBack />`. The sidebar shows "Trash · {n}".

- [ ] **Step 1: Write the failing tests:**
  - `rows show title, origin and date`: "From Manuscript · deleted …", and folders read "(folder, {n} documents)".
  - `restore calls the command and removes the row`
  - `delete forever and empty trash confirm first`, with the exact `trash.confirmDelete` and `trash.confirmEmpty` texts.
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement** to match mockup 4.
- [ ] **Step 4:** Run `npm run typecheck && npm test`. Expected: pass.
- [ ] **Step 5: Commit:** `feat(ui): project trash`

### Task 12: Friendly failures everywhere, and a check on Windows

**Files:**
- Create: `src/components/ErrorBanner.tsx` and its test
- Modify: the screens from Tasks 8–11, which route every rejected command through `friendlyMessage` into the `ErrorBanner`

- [ ] **Step 1: Write the failing tests:**
  - `failed action keeps the tree and shows the message`: `rename_node` rejects with `Locked`. The tree is unchanged, and `error.Locked` is shown in a `role="alert"` banner.
  - `banner can be dismissed`
- [ ] **Step 2:** Run `npm test`. Expected: FAIL.
- [ ] **Step 3: Implement**, and wire the banner into every screen.
- [ ] **Step 4: Verify.**
  - Run `npm run typecheck && npm test && cargo test --workspace`. Expected: all pass.
  - Then run `npm run tauri dev` on Windows and walk through the "Done when" items that 3a covers:
    1. create a Library
    2. create a project, which opens on Chapter 1
    3. add folders and documents
    4. rename, reorder, move and trash
    5. restore, delete forever and find the file in the Recycle Bin
    6. trash and restore a project from the Library
  - Record the result in the ledger.
- [ ] **Step 5: Commit:** `feat(ui): friendly error banner across screens`

---

## Handoff to Plan 3b

- **Replace the placeholder pane** in `Project.tsx` with the editor. `read_document` returns `{ title, body }`, and 3b adds a `save_body` command over `Project::save_body`.
- **Use the registry** for the new commands: Ctrl+E for Write and Revise, Ctrl+Shift+S for a named snapshot.
- **The per-project `Mutex`** already serialises calls, so autosave needs no queue of its own. Debounce conversion off the keystroke path (carry-over from PR #4).
- **Open question for 3b:** should Write mode dim earlier paragraphs? The mockup drew dimming, but master §7.1 says "focus highlight" is off by default.
