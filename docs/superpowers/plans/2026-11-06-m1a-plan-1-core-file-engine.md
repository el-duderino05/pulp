# Milestone 1a, Plan 1: Setup and Core File Engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A working development environment and public repository, plus `pulp-core`: a UI-free Rust library that safely creates, opens, edits, snapshots and trashes Pulp Libraries and projects on disk, fully tested on Windows, macOS and Linux.

**Architecture:** Cargo workspace with `crates/pulp-core` (all file logic, no Tauri dependency) and the Tauri app in `src-tauri` (scaffolded here, wired in Plan 3). Every file operation goes through a `Storage` trait over forward-slash relative paths; the desktop implementation is `FsStorage`. Document bodies are opaque text to the core.

**Tech Stack:** Rust (stable, MSVC toolchain on Windows), crates `ulid`, `unicode-normalization`, `serde`, `serde_json` (feature `preserve_order`), `serde_norway`, `indexmap` (feature `serde`), `chrono` (feature `serde`), `jsonschema`, `thiserror`; dev: `tempfile`. Tauri 2 + React + TypeScript (scaffold only).

**Spec:** `docs/specs/pulp-master-design.md` (v8.1; the v7, v8 and v8.1 changes were checked and do not affect this plan) and `docs/specs/2026-11-06-m1a-walking-skeleton.md` (the M1a spec; §3 file formats are authoritative for this plan).

## Global Constraints

- Every source file starts with `// SPDX-License-Identifier: MPL-2.0`.
- `pulp-core` must not depend on Tauri, any UI crate, or absolute paths; all I/O goes through `Storage` + `RelPath`.
- IDs are ULIDs: 26 characters, Crockford base32, uppercase.
- Text written by the core is UTF-8, LF line endings, Unicode NFC.
- File names follow the slug rule: accents removed from Latin letters only, forbidden characters `: ? * " < > | \ /` never appear, Windows reserved names avoided, max 60 characters before the extension, case-insensitive clash → `-2`, `-3`…
- Format version constants: `FORMAT_VERSION = 1`, `READER_VERSION = 1`.
- **"Pulp" is a working name.** Every name-bearing identifier the core writes to disk or into files is defined once in `crates/pulp-core/src/brand.rs` and used from there: never type `pulp` as a literal in other modules. Renaming before release must be a change to that one file plus its test.
- Item header keys in order: `format, minReader, id, type, kind, title, status, labels, synopsis, created`, then unknown keys in original order. No `modified` key.
- JSON files: 2-space pretty print, trailing newline, unknown keys preserved.
- Atomic save: temp file `~$<file-name>.pulp.tmp` in the same folder, flush, rename over target. Retry 5 times, 200 ms apart, for `Locked` and `Io` errors only.
- Snapshot thinning: all < 24 h; newest per UTC day up to 30 days; newest per ISO week after; named and operation-group snapshots never thinned.
- Every commit uses `git commit -s` (DCO sign-off).
- `cargo fmt --check` and `cargo clippy -- -D warnings` pass for `pulp-core`.

## Review Focus

1. **Titles that produce nothing usable** (only punctuation, only emoji, empty, whitespace) must give the file name `untitled`, never an empty or hidden name. Pinned in Task 3.
2. **Text arriving in another system's form** (macOS decomposed accents, Windows CRLF) must be saved as NFC + LF, and unknown header keys must survive the save in their original order. Pinned in Tasks 5 and 9.
3. **Ordinary Markdown files inside a project** (no Pulp header, for example a README someone dropped in) must never be modified, moved, or treated as items. Pinned in Task 8.
4. **Titles differing only by case or accents in one folder** ("Ana", "ana", "Aná") must get three distinct files, on case-insensitive Windows and macOS as well. Pinned in Task 9.
5. **A save that fails partway** must leave the original file's bytes exactly as they were and leave no temp file behind. Pinned in Task 4.

---

## File Structure

```
pulp/
├── Cargo.toml                         workspace (Task 1)
├── CLAUDE.md, CONTRIBUTING.md, README.md, LICENSE, CODE_OF_CONDUCT.md
├── .github/workflows/ci.yml           CI matrix (Task 1)
├── .github/ISSUE_TEMPLATE/            bug_report.md, feature_request.md (Task 1)
├── docs/specs/                        master spec, M1a spec
├── docs/superpowers/plans/            this plan
├── src/, src-tauri/                   Tauri scaffold (Task 1; used in Plan 3)
└── crates/pulp-core/
    ├── Cargo.toml
    ├── schemas/                       manifest, library, item-header JSON schemas (Task 6)
    ├── src/
    │   ├── lib.rs                     module list + re-exports
    │   ├── id.rs                      Id (Task 2)
    │   ├── text.rs                    normalize_text (Task 2)
    │   ├── filename.rs                slugify, unique_name (Task 3)
    │   ├── storage.rs                 RelPath, Storage, FsStorage, errors, retry (Task 4)
    │   ├── version.rs                 versions (Task 5)
    │   ├── header.rs                  ItemHeader, parse/render (Task 5)
    │   ├── manifest.rs                Manifest, Node, FolderNode (Task 6)
    │   ├── schema.rs                  schema validation (Task 6)
    │   ├── library.rs                 Library (Task 7)
    │   ├── project.rs                 Project create/open/scan (Task 8)
    │   ├── items.rs                   item and folder operations (Task 9)
    │   ├── trash.rs                   project trash (Task 10)
    │   ├── reconcile.rs               reconcile rules (Task 11)
    │   └── history.rs                 snapshots, thinning, operation groups (Task 12)
    └── tests/                         one integration test file per task
```

---

### Task 0: Development environment (done by Gonçalo, guided by Claude Code)

No code. Each step ends with a command whose output proves it worked. Run commands in **PowerShell**.

- [ ] **Step 1: Install Microsoft C++ Build Tools ("Desktop development with C++")**

Run:
```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--passive --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```
Expected: installer finishes without error (it may take 10–20 minutes). Restart the computer afterwards.

- [ ] **Step 2: Make Rust use the MSVC toolchain and update it**

Run: `rustup default stable-msvc` then `rustup update`
Expected: `default toolchain set to 'stable-x86_64-pc-windows-msvc'`.

- [ ] **Step 3: Prove Rust can build on this machine**

Run, in a scratch folder: `cargo new hello; cd hello; cargo run`
Expected: last line is `Hello, world!`. Delete the `hello` folder afterwards.

- [ ] **Step 4: Check Node and Git**

Run: `node --version` and `git --version`
Expected: Node `v20` or newer; any recent Git. If Node is older, install the current LTS from nodejs.org.

- [ ] **Step 5: Configure Git**

Run:
```powershell
git config --global user.name "Gonçalo <surname>"
git config --global user.email "<id>+<username>@users.noreply.github.com"
git config --global core.longpaths true
git config --global core.autocrlf false
```
Use the GitHub "noreply" address (GitHub › Settings › Emails) so your personal email never appears in public commits. `core.autocrlf false` stops Git from rewriting line endings; Pulp manages LF itself.
Expected: `git config --global --list` shows all four values.

- [ ] **Step 6: Create the public repository on GitHub**

On github.com: New repository › name `pulp` › Public › tick **Add a README** › licence **Mozilla Public License 2.0** › Create. Then: Insights › Community standards › Code of conduct › Add › **Contributor Covenant** › commit to `main`. Then install the **DCO** GitHub App (github.com/apps/dco) on this repository.
Expected: the repository page shows README, LICENSE and CODE_OF_CONDUCT.md.

- [ ] **Step 7: Clone to a short path**

Run: `mkdir C:\dev; cd C:\dev; git clone https://github.com/<username>/pulp.git; cd pulp`
Expected: `C:\dev\pulp` contains the three files. A short path avoids Windows path-length problems.

---

### Task 1: Repository scaffold, community files and CI

**Files:**
- Create: `Cargo.toml` (workspace), `crates/pulp-core/**` (via `cargo new`), `src/**`, `src-tauri/**` (via create-tauri-app), `CLAUDE.md`, `CONTRIBUTING.md`, `.github/workflows/ci.yml`, `.github/ISSUE_TEMPLATE/bug_report.md`, `.github/ISSUE_TEMPLATE/feature_request.md`, `.gitignore` additions
- Copy in: `docs/specs/pulp-master-design.md`, `docs/specs/2026-11-06-m1a-walking-skeleton.md`, this plan

**Interfaces:**
- Produces: workspace where `cargo test -p pulp-core` and `npm run tauri dev` both work.

- [ ] **Step 1: Scaffold the Tauri app into the repository**

Run in `C:\dev\pulp`: `npm create tauri-app@latest .` and answer: project name `pulp`; identifier `io.github.<username>.pulp`; frontend language **TypeScript / JavaScript**; package manager **npm**; UI template **React**; UI flavour **TypeScript**. Allow it to use the non-empty folder. Then `npm install`.

- [ ] **Step 2: Verify the empty app runs**

Run: `npm run tauri dev`
Expected: a desktop window opens showing the Tauri + React welcome page. Close it.

- [ ] **Step 3: Create the workspace and the core crate**

Create root `Cargo.toml`:
```toml
[workspace]
members = ["crates/pulp-core", "src-tauri"]
resolver = "2"
```
Run: `cargo new --lib crates/pulp-core`. Delete `src-tauri/Cargo.lock` if present (the workspace lock file at the root replaces it). Add to `.gitignore`: `/target`.
Add dependencies in `crates/pulp-core`:
```powershell
cd crates/pulp-core
cargo add ulid unicode-normalization thiserror serde_norway jsonschema
cargo add serde --features derive
cargo add serde_json --features preserve_order
cargo add indexmap --features serde
cargo add chrono --features serde
cargo add --dev tempfile
cd ../..
```

- [ ] **Step 4: Verify the workspace builds and tests**

Run: `cargo test -p pulp-core`
Expected: the generated `it_works` test passes. Then `npm run tauri dev` still opens the window.

- [ ] **Step 5: Write the community and agent files**

`CONTRIBUTING.md` must state: the MPL-2.0 licence; every commit needs a DCO sign-off (`git commit -s`), with one sentence explaining what it certifies and a link to developercertificate.org; tests must pass (`cargo test -p pulp-core`); `cargo fmt` and `cargo clippy -- -D warnings` must be clean; new source files carry the SPDX line; the specs in `docs/specs/` are the source of truth for behaviour.

`CLAUDE.md` must state: what Pulp is (one paragraph); read `docs/specs/pulp-master-design.md` and the current milestone spec before changing behaviour; `pulp-core` stays UI-free and does all I/O through `Storage`; test-driven development; commit with `-s`; never weaken a test to make it pass; explain results to Gonçalo in plain language, since he does not read code.

Issue templates: `bug_report.md` (what happened, what you expected, steps, Pulp version, operating system) and `feature_request.md` (the problem, the proposed solution, which kind of writer it helps).

- [ ] **Step 6: Write the CI workflow**

`.github/workflows/ci.yml`:
```yaml
name: CI
on: [push, pull_request]
jobs:
  core:
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - run: cargo fmt -p pulp-core --check
      - run: cargo clippy -p pulp-core --all-targets -- -D warnings
      - run: cargo test -p pulp-core
```

- [ ] **Step 7: Commit, push and check CI**

```powershell
git add -A
git commit -s -m "chore: scaffold Tauri app, pulp-core crate, community files and CI"
git push
```
Expected: on GitHub › Actions, the CI run is green on all three systems, and the DCO check passes.

---

### Task 2: IDs and text normalisation

**Files:**
- Create: `crates/pulp-core/src/id.rs`, `crates/pulp-core/src/text.rs`, `crates/pulp-core/src/brand.rs`, `crates/pulp-core/tests/id_text.rs`
- Modify: `crates/pulp-core/src/lib.rs` (replace generated content with `pub mod brand; pub mod id; pub mod text;`, later tasks add their modules)

**Interfaces:**
- Produces: in `brand.rs`: `pub const APP_NAME: &str = "Pulp"; pub const MANIFEST_FILE: &str = "pulp-project.json"; pub const LINK_SCHEME: &str = "pulp"; pub const TEMP_SUFFIX: &str = ".pulp.tmp"; pub const PROJECT_ARCHIVE_EXT: &str = "pulp"; pub const TEMPLATE_EXT: &str = "pulptemplate";` (later milestones use the last three).
- Produces: `pub struct Id` (Clone, Eq, Hash, Ord, Debug, Display, Serialize/Deserialize as a string); `Id::new() -> Id`; `Id::parse(s: &str) -> Result<Id, IdError>` (accepts lowercase, stores uppercase); `Id::as_str(&self) -> &str`; `Id::short(&self) -> &str` (last 6 characters). `pub fn normalize_text(input: &str) -> String`.

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{id::Id, text::normalize_text};

#[test]
fn brand_identifiers_are_consistent() {
    use pulp_core::brand::*;
    assert_eq!(MANIFEST_FILE, format!("{}-project.json", LINK_SCHEME));
    assert_eq!(TEMP_SUFFIX, format!(".{}.tmp", PROJECT_ARCHIVE_EXT));
    assert_eq!(APP_NAME.to_lowercase(), LINK_SCHEME);
}

#[test]
fn new_ids_are_26_char_ulids_and_unique() {
    let a = Id::new();
    let b = Id::new();
    assert_eq!(a.as_str().len(), 26);
    assert_ne!(a, b);
    assert_eq!(Id::parse(a.as_str()).unwrap(), a);
}

#[test]
fn ids_sort_by_creation_time() {
    let a = Id::new();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let b = Id::new();
    assert!(a < b);
}

#[test]
fn parse_rejects_non_ulids_and_accepts_lowercase() {
    assert!(Id::parse("7k2m9q").is_err());
    assert!(Id::parse("").is_err());
    let a = Id::new();
    assert_eq!(Id::parse(&a.as_str().to_lowercase()).unwrap(), a);
}

#[test]
fn short_is_last_six_characters() {
    let a = Id::parse("01JC3Z8K7QW2M4N6P8R0T2V4X6").unwrap();
    assert_eq!(a.short(), "T2V4X6");
}

#[test]
fn normalize_converts_crlf_and_cr_to_lf() {
    assert_eq!(normalize_text("a\r\nb\rc\n"), "a\nb\nc\n");
}

#[test]
fn normalize_composes_decomposed_accents() {
    assert_eq!(normalize_text("Ac\u{0327}a\u{0303}o"), "Ação");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test id_text`
Expected: FAIL to compile (`id` and `text` modules don't exist).

- [ ] **Step 3: Implement `Id` in `src/id.rs` and `normalize_text` in `src/text.rs`**

`Id` wraps the `ulid` crate's string form. `normalize_text` replaces `\r\n` then lone `\r` with `\n`, then applies `unicode_normalization::UnicodeNormalization::nfc`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test id_text`
Expected: 7 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): ULID ids, text normalisation and name constants"
```

---

### Task 3: File-name rule

**Files:**
- Create: `crates/pulp-core/src/filename.rs`, `crates/pulp-core/tests/filename.rs`

**Interfaces:**
- Produces: `pub const MAX_SLUG_CHARS: usize = 60;` `pub fn slugify(title: &str) -> String;` `pub fn unique_name(slug: &str, ext: &str, existing: &[String]) -> String` (returns `slug.ext`, or `slug-2.ext`, `slug-3.ext`… comparing case-insensitively after NFC).

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::filename::{slugify, unique_name};

#[test]
fn latin_titles_lose_accents_and_punctuation() {
    assert_eq!(slugify("Capítulo 1 — A Chegada"), "capitulo-1-a-chegada");
    assert_eq!(slugify("Ação & Reação"), "acao-reacao");
    assert_eq!(slugify("Chapter 1: The Storm!"), "chapter-1-the-storm");
    assert_eq!(slugify("Straße"), "strasse");
    assert_eq!(slugify("Œuvre"), "oeuvre");
    assert_eq!(slugify("Łódź"), "lodz");
}

#[test]
fn other_scripts_keep_their_letters() {
    assert_eq!(slugify("Война и мир"), "война-и-мир");
    assert_eq!(slugify("東京物語"), "東京物語");
    assert_eq!(slugify("नमस्ते दुनिया"), "नमस्ते-दुनिया");
    assert_eq!(slugify("Café Москва"), "cafe-москва");
}

#[test]
fn unusable_titles_become_untitled() {
    assert_eq!(slugify(""), "untitled");
    assert_eq!(slugify("   "), "untitled");
    assert_eq!(slugify(":?*<>|"), "untitled");
    assert_eq!(slugify("🙂🙂"), "untitled");
}

#[test]
fn windows_reserved_names_are_avoided() {
    assert_eq!(slugify("CON"), "con-file");
    assert_eq!(slugify("aux"), "aux-file");
    assert_eq!(slugify("COM1"), "com1-file");
}

#[test]
fn long_titles_are_capped_without_trailing_dash() {
    let s = slugify(&"word ".repeat(40));
    assert!(s.chars().count() <= 60);
    assert!(!s.ends_with('-'));
}

#[test]
fn unique_name_adds_suffix_ignoring_case() {
    assert_eq!(unique_name("ana", "md", &[]), "ana.md");
    assert_eq!(unique_name("ana", "md", &["Ana.md".into()]), "ana-2.md");
    assert_eq!(unique_name("ana", "md", &["ana.md".into(), "ANA-2.md".into()]), "ana-3.md");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test filename`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `slugify` and `unique_name` in `src/filename.rs`**

`slugify` algorithm, in order:
1. Map special Latin letters that don't decompose: `ß→ss, æ→ae, œ→oe, ø→o, ł→l, đ→d, ð→d, þ→th` (and their uppercase forms).
2. Decompose to NFD; drop a combining mark only when the preceding base character is Latin (U+0041–U+024F); keep combining marks after non-Latin bases (needed for Devanagari, Bengali and others).
3. Lowercase.
4. Keep characters that are alphanumeric or combining marks; every run of anything else becomes one `-`.
5. Recompose to NFC; trim `-` and `.` from both ends.
6. Cap at `MAX_SLUG_CHARS` characters, then trim trailing `-` again.
7. Empty → `untitled`. Equal to `con, prn, aux, nul, com1`–`com9`, `lpt1`–`lpt9` → append `-file`.

Use `unicode_normalization::char::is_combining_mark` for step 2 and 4.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test filename`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): file-name rule"
```

---

### Task 4: Storage, atomic save and error handling

**Files:**
- Create: `crates/pulp-core/src/storage.rs`, `crates/pulp-core/tests/storage.rs`

**Interfaces:**
- Produces:
  - `pub struct RelPath` (Clone, Eq, Hash, Ord, Debug, Serialize/Deserialize as string): `RelPath::new(s: &str) -> Result<RelPath, StorageError>` (converts `\` to `/`; rejects empty, absolute, drive-letter and `..` paths); `as_str()`; `join(&self, name: &str) -> RelPath`; `parent(&self) -> Option<RelPath>`; `file_name(&self) -> &str`; `RelPath::root()` (empty path meaning the storage root).
  - `pub struct Entry { pub name: String, pub is_dir: bool, pub modified: std::time::SystemTime }`
  - `pub enum ErrorCause { NotFound, Locked, DiskFull, PermissionDenied, Io }`
  - `pub enum StorageError` (thiserror) with one variant per cause, each carrying the `RelPath` and a message; `fn cause(&self) -> ErrorCause`.
  - `pub trait Storage { fn read(&self, p: &RelPath) -> Result<Vec<u8>, StorageError>; fn write_atomic(&self, p: &RelPath, data: &[u8]) -> Result<(), StorageError>; fn list(&self, dir: &RelPath) -> Result<Vec<Entry>, StorageError>; fn create_dir_all(&self, p: &RelPath) -> Result<(), StorageError>; fn rename(&self, from: &RelPath, to: &RelPath) -> Result<(), StorageError>; fn remove_file(&self, p: &RelPath) -> Result<(), StorageError>; fn exists(&self, p: &RelPath) -> bool; }`
  - `pub struct FsStorage` with `FsStorage::new(root: impl Into<PathBuf>) -> FsStorage`; implements `Storage`.
  - `pub fn temp_name_for(file_name: &str) -> String` (returns `~$` + file name + `brand::TEMP_SUFFIX`, today `~$<file_name>.pulp.tmp`); `pub fn is_temp_name(name: &str) -> bool`.
  - `pub enum Os { Windows, Unix }`; `pub fn classify_os_error(code: i32, os: Os) -> ErrorCause`.
  - `pub fn with_retry<T>(attempts: u32, delay: Duration, op: impl FnMut() -> Result<T, StorageError>) -> Result<T, StorageError>` (retries only `Locked` and `Io`).
  - `pub const SAVE_RETRIES: u32 = 5; pub const RETRY_DELAY: Duration = Duration::from_millis(200);`

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::storage::*;
use std::{cell::Cell, time::Duration};

fn tmp() -> (tempfile::TempDir, FsStorage) {
    let d = tempfile::tempdir().unwrap();
    let s = FsStorage::new(d.path());
    (d, s)
}

#[test]
fn relpath_normalises_and_rejects_escapes() {
    assert_eq!(RelPath::new("a\\b.md").unwrap().as_str(), "a/b.md");
    assert!(RelPath::new("../x").is_err());
    assert!(RelPath::new("a/../../x").is_err());
    assert!(RelPath::new("/abs").is_err());
    assert!(RelPath::new("C:\\x").is_err());
}

#[test]
fn write_atomic_creates_replaces_and_leaves_no_temp_file() {
    let (_d, s) = tmp();
    let p = RelPath::new("doc.md").unwrap();
    s.write_atomic(&p, b"one").unwrap();
    s.write_atomic(&p, b"two").unwrap();
    assert_eq!(s.read(&p).unwrap(), b"two");
    let names: Vec<_> = s.list(&RelPath::root()).unwrap().into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec!["doc.md".to_string()]);
}

#[test]
fn failed_save_leaves_original_untouched() {
    let (d, s) = tmp();
    let p = RelPath::new("doc.md").unwrap();
    s.write_atomic(&p, b"original").unwrap();
    // Block the temp file path with a directory so the save cannot proceed.
    std::fs::create_dir(d.path().join(temp_name_for("doc.md"))).unwrap();
    assert!(s.write_atomic(&p, b"new").is_err());
    assert_eq!(s.read(&p).unwrap(), b"original");
}

#[test]
fn temp_names_are_recognised() {
    assert_eq!(temp_name_for("a.md"), "~$a.md.pulp.tmp");
    assert!(is_temp_name("~$a.md.pulp.tmp"));
    assert!(!is_temp_name("a.md"));
}

#[test]
fn os_errors_are_classified() {
    assert_eq!(classify_os_error(32, Os::Windows), ErrorCause::Locked);
    assert_eq!(classify_os_error(33, Os::Windows), ErrorCause::Locked);
    assert_eq!(classify_os_error(112, Os::Windows), ErrorCause::DiskFull);
    assert_eq!(classify_os_error(5, Os::Windows), ErrorCause::PermissionDenied);
    assert_eq!(classify_os_error(28, Os::Unix), ErrorCause::DiskFull);
    assert_eq!(classify_os_error(13, Os::Unix), ErrorCause::PermissionDenied);
}

#[test]
fn retry_repeats_locked_but_not_permission_errors() {
    let p = RelPath::new("x").unwrap();
    let calls = Cell::new(0);
    let r = with_retry(5, Duration::from_millis(1), || {
        calls.set(calls.get() + 1);
        if calls.get() < 3 { Err(StorageError::Locked(p.clone(), "busy".into())) } else { Ok(7) }
    });
    assert_eq!(r.unwrap(), 7);
    assert_eq!(calls.get(), 3);

    calls.set(0);
    let r: Result<(), _> = with_retry(5, Duration::from_millis(1), || {
        calls.set(calls.get() + 1);
        Err(StorageError::PermissionDenied(p.clone(), "no".into()))
    });
    assert!(r.is_err());
    assert_eq!(calls.get(), 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test storage`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/storage.rs`**

`FsStorage::write_atomic`: create the temp file with `create_new` (so an existing temp or directory makes it fail), write, `sync_all`, then `std::fs::rename` over the target (replaces on Windows too). On any failure, best-effort remove a temp file it created, and return the classified error. Wrap the whole write in `with_retry(SAVE_RETRIES, RETRY_DELAY, …)`. Map `io::Error` via `raw_os_error()` and `classify_os_error` (Windows: 32, 33 → Locked; 112, 39 → DiskFull; 5 → PermissionDenied; 2, 3 → NotFound. Unix: 28 → DiskFull; 1, 13 → PermissionDenied; 11, 26 → Locked; 2 → NotFound; anything else → Io). `list` excludes nothing (filtering is the caller's job).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test storage`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): storage layer with atomic save and retries"
```

---

### Task 5: Versions and item headers

**Files:**
- Create: `crates/pulp-core/src/version.rs`, `crates/pulp-core/src/header.rs`, `crates/pulp-core/tests/header.rs`

**Interfaces:**
- Consumes: `Id` (Task 2), `normalize_text` (Task 2).
- Produces:
  - `pub const FORMAT_VERSION: u32 = 1; pub const READER_VERSION: u32 = 1;` `pub fn check_readable(min_reader: u32) -> Result<(), VersionError>`; `VersionError::TooNew { required: u32, supported: u32 }`.
  - `pub enum ItemType { Document, Profile, Note }` (YAML: `document`, `profile`, `note`); `pub enum DocKind { Prose, Poetry, Screenplay, StagePlay }` (YAML: `prose`, `poetry`, `screenplay`, `stage-play`).
  - `pub struct ItemHeader { pub format: u32, pub min_reader: u32, pub id: Id, pub item_type: ItemType, pub kind: Option<DocKind>, pub title: String, pub status: Option<String>, pub labels: Vec<String>, pub synopsis: String, pub created: String, pub extra: IndexMap<String, serde_norway::Value> }` with YAML keys per Global Constraints (`minReader`, `type`); `extra` flattened.
  - `ItemHeader::new_document(title: &str, kind: DocKind) -> ItemHeader` (new ID, current versions, `created` = now in RFC 3339 UTC with `Z`).
  - `pub struct ItemFile { pub header: ItemHeader, pub body: String }`
  - `pub fn parse_markdown_item(text: &str) -> Result<ItemFile, HeaderError>`; `pub fn render_markdown_item(file: &ItemFile) -> String`; `HeaderError::{Missing, Invalid(String)}`.

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, version::*};

const SAMPLE: &str = "---\nformat: 1\nminReader: 1\nid: 01JC3Z8K7QW2M4N6P8R0T2V4X6\ntype: document\nkind: prose\ntitle: Arrival\nstatus: Draft\nlabels: []\nsynopsis: ''\ncreated: 2026-11-06T17:00:00Z\nfutureKey: keep me\nanotherKey:\n- 1\n- 2\n---\nBody text.\n";

#[test]
fn parse_reads_known_fields_and_body() {
    let f = parse_markdown_item(SAMPLE).unwrap();
    assert_eq!(f.header.title, "Arrival");
    assert_eq!(f.header.item_type, ItemType::Document);
    assert_eq!(f.header.kind, Some(DocKind::Prose));
    assert_eq!(f.header.status.as_deref(), Some("Draft"));
    assert_eq!(f.body, "Body text.\n");
}

#[test]
fn render_keeps_unknown_keys_in_order_after_known_keys() {
    let out = render_markdown_item(&parse_markdown_item(SAMPLE).unwrap());
    let pos = |k: &str| out.find(k).unwrap();
    assert!(pos("format:") < pos("minReader:"));
    assert!(pos("minReader:") < pos("id:"));
    assert!(pos("title:") < pos("created:"));
    assert!(pos("created:") < pos("futureKey:"));
    assert!(pos("futureKey:") < pos("anotherKey:"));
    assert!(out.contains("keep me"));
    assert!(out.ends_with("---\nBody text.\n"));
}

#[test]
fn render_parse_round_trip_is_stable() {
    let once = render_markdown_item(&parse_markdown_item(SAMPLE).unwrap());
    let twice = render_markdown_item(&parse_markdown_item(&once).unwrap());
    assert_eq!(once, twice);
}

#[test]
fn missing_or_broken_headers_are_reported() {
    assert!(matches!(parse_markdown_item("Just text"), Err(HeaderError::Missing)));
    assert!(matches!(parse_markdown_item("---\ntitle: [unclosed\n---\n"), Err(HeaderError::Invalid(_))));
}

#[test]
fn new_document_header_has_current_versions() {
    let h = ItemHeader::new_document("Arrival", DocKind::Prose);
    assert_eq!(h.format, FORMAT_VERSION);
    assert_eq!(h.min_reader, READER_VERSION);
    assert!(h.created.ends_with('Z'));
}

#[test]
fn newer_reader_versions_are_refused() {
    assert!(check_readable(1).is_ok());
    assert!(matches!(check_readable(2), Err(VersionError::TooNew { required: 2, supported: 1 })));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test header`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/version.rs` and `src/header.rs`**

Front matter: the text must start with `---\n` (after `normalize_text`); the header ends at the next line that is exactly `---`; the body is everything after that line's newline, unchanged. Render as `---\n` + YAML from `serde_norway` + `---\n` + body. Field declaration order in the struct gives the known-key order; `extra` is `#[serde(flatten)]`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test header`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): format versions and item headers"
```

---

### Task 6: Manifest and schemas

**Files:**
- Create: `crates/pulp-core/src/manifest.rs`, `crates/pulp-core/src/schema.rs`, `crates/pulp-core/schemas/manifest.schema.json`, `crates/pulp-core/schemas/library.schema.json`, `crates/pulp-core/schemas/item-header.schema.json`, `crates/pulp-core/tests/manifest.rs`

**Interfaces:**
- Consumes: `Id`.
- Produces:
  - `pub struct Manifest { pub format: u32, pub min_reader: u32, pub id: Id, pub name: String, pub tree: Vec<Node>, pub statuses: Vec<String>, pub labels: Vec<Label>, pub settings: serde_json::Map<String, Value>, pub extra: serde_json::Map<String, Value> }` (JSON keys camelCase; `extra` flattened).
  - `pub enum Node { Folder(FolderNode), Item { id: Id } }` serialised with tag `"node"`: `"folder"` / `"item"`.
  - `pub struct FolderNode { pub id: Id, pub title: String, pub path: String, pub synopsis: String, pub children: Vec<Node>, pub extra: serde_json::Map<String, Value> }`; `pub struct Label { pub name: String, pub color: String }`.
  - `Manifest::new(name: &str) -> Manifest` (statuses `Idea, Draft, Revised, Final`; tree = one folder titled `Manuscript` with path `manuscript`); `Manifest::load(text: &str) -> Result<Manifest, ManifestError>` (validates against the schema and checks `check_readable`); `to_json(&self) -> String`; `item_ids(&self) -> Vec<Id>`; `folder(&self, id: &Id) -> Option<&FolderNode>`; `folder_mut(&mut self, id: &Id) -> Option<&mut FolderNode>`; `folder_by_path(&self, path: &str) -> Option<&FolderNode>`; `parent_of(&self, item: &Id) -> Option<(Option<Id>, usize)>` (folder ID or `None` for top level, plus index); `insert_item(&mut self, folder: Option<&Id>, item: Id, index: Option<usize>) -> Result<(), ManifestError>`; `remove_node(&mut self, id: &Id) -> Option<Node>`.
  - `pub fn validate(schema: SchemaKind, value: &serde_json::Value) -> Result<(), Vec<String>>` with `pub enum SchemaKind { Manifest, Library, ItemHeader }`; schemas embedded with `include_str!`.

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{id::Id, manifest::*, schema::*};

#[test]
fn new_manifest_has_defaults() {
    let m = Manifest::new("The Storm");
    assert_eq!(m.statuses, vec!["Idea", "Draft", "Revised", "Final"]);
    match &m.tree[0] {
        Node::Folder(f) => { assert_eq!(f.title, "Manuscript"); assert_eq!(f.path, "manuscript"); }
        _ => panic!("first node must be the Manuscript folder"),
    }
}

#[test]
fn json_is_pretty_with_trailing_newline_and_round_trips() {
    let m = Manifest::new("The Storm");
    let j = m.to_json();
    assert!(j.ends_with("}\n"));
    assert!(j.contains("\n  \"format\": 1"));
    assert!(j.contains("\"minReader\": 1"));
    assert_eq!(Manifest::load(&j).unwrap().to_json(), j);
}

#[test]
fn unknown_keys_survive_load_and_save() {
    let mut v: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    v["futureTopKey"] = serde_json::json!({"a": 1});
    v["tree"][0]["futureFolderKey"] = serde_json::json!(true);
    let out = Manifest::load(&serde_json::to_string(&v).unwrap()).unwrap().to_json();
    assert!(out.contains("futureTopKey"));
    assert!(out.contains("futureFolderKey"));
}

#[test]
fn insert_find_and_remove_items() {
    let mut m = Manifest::new("X");
    let folder = match &m.tree[0] { Node::Folder(f) => f.id.clone(), _ => unreachable!() };
    let (a, b) = (Id::new(), Id::new());
    m.insert_item(Some(&folder), a.clone(), None).unwrap();
    m.insert_item(Some(&folder), b.clone(), Some(0)).unwrap();
    assert_eq!(m.item_ids(), vec![b.clone(), a.clone()]);
    assert_eq!(m.parent_of(&a), Some((Some(folder.clone()), 1)));
    assert!(m.remove_node(&b).is_some());
    assert_eq!(m.item_ids(), vec![a]);
}

#[test]
fn schemas_accept_valid_and_reject_invalid() {
    let good: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    assert!(validate(SchemaKind::Manifest, &good).is_ok());
    let mut bad = good.clone();
    bad.as_object_mut().unwrap().remove("id");
    assert!(validate(SchemaKind::Manifest, &bad).is_err());
}

#[test]
fn too_new_manifest_is_refused() {
    let mut v: serde_json::Value = serde_json::from_str(&Manifest::new("X").to_json()).unwrap();
    v["minReader"] = serde_json::json!(2);
    assert!(Manifest::load(&serde_json::to_string(&v).unwrap()).is_err());
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test manifest`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/manifest.rs`, `src/schema.rs` and the three schemas**

Schemas follow JSON Schema draft 2020-12, mirror the M1a spec §3.2–3.4, require the documented keys, and allow additional properties everywhere (so unknown keys pass). `ItemHeader` validation converts the YAML header to JSON before validating. `to_json` uses `serde_json::to_string_pretty` plus `"\n"`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test manifest`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): project manifest and JSON schemas"
```

---

### Task 7: Library

**Files:**
- Create: `crates/pulp-core/src/library.rs`, `crates/pulp-core/tests/library.rs`

**Interfaces:**
- Consumes: `FsStorage`, `Storage`, `RelPath`, `Id`, `slugify`, `unique_name`, `validate(SchemaKind::Library, …)`, `check_readable`; `Project::create` from Task 8 is used by `create_project` (write this method in Task 8, leaving it out here).
- Produces:
  - `pub const LIBRARY_DIRS: [&str; 5] = ["Projects", "Inbox", "Templates", "Backups", ".trash"];`
  - `pub struct Author { pub id: Id, pub name: String }`; `pub struct LibrarySettings { pub format: u32, pub min_reader: u32, pub author: Author, pub settings: serde_json::Map<String, Value>, pub extra: serde_json::Map<String, Value> }`
  - `pub struct Library { pub root: PathBuf, pub settings: LibrarySettings }`
  - `Library::create(root: &Path, author_name: &str) -> Result<Library, LibraryError>`; `Library::open(root: &Path) -> Result<(Library, Vec<String>), LibraryError>` (second value: names of protected folders that were recreated); `list_projects(&self) -> Result<Vec<ProjectSummary>, LibraryError>` with `pub struct ProjectSummary { pub folder: String, pub name: String, pub id: Id }`.
  - `pub enum LibraryError { NotALibrary(PathBuf), Storage(StorageError), Invalid(String), Version(VersionError) }`

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::library::*;

#[test]
fn create_makes_protected_folders_and_settings() {
    let d = tempfile::tempdir().unwrap();
    let lib = Library::create(d.path(), "Gonçalo").unwrap();
    for name in LIBRARY_DIRS { assert!(d.path().join(name).is_dir(), "{name} missing"); }
    assert!(d.path().join("library.json").is_file());
    assert_eq!(lib.settings.author.name, "Gonçalo");
}

#[test]
fn opening_a_folder_without_library_json_fails() {
    let d = tempfile::tempdir().unwrap();
    assert!(matches!(Library::open(d.path()), Err(LibraryError::NotALibrary(_))));
}

#[test]
fn open_recreates_missing_protected_folders_and_reports_them() {
    let d = tempfile::tempdir().unwrap();
    Library::create(d.path(), "G").unwrap();
    std::fs::remove_dir(d.path().join("Inbox")).unwrap();
    let (_lib, recreated) = Library::open(d.path()).unwrap();
    assert_eq!(recreated, vec!["Inbox".to_string()]);
    assert!(d.path().join("Inbox").is_dir());
}

#[test]
fn library_json_keeps_unknown_keys_and_author_id() {
    let d = tempfile::tempdir().unwrap();
    let id = Library::create(d.path(), "G").unwrap().settings.author.id;
    let p = d.path().join("library.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
    v["futureKey"] = serde_json::json!(1);
    std::fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let (lib, _) = Library::open(d.path()).unwrap();
    assert_eq!(lib.settings.author.id, id);
    assert!(lib.settings.extra.contains_key("futureKey"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test library`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/library.rs`**

`create` refuses a folder that already has `library.json` (`Invalid`). Settings are written with `write_atomic`, 2-space pretty JSON and a trailing newline.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test library`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): Library creation, opening and protected folders"
```

---

### Task 8: Project create, open and scan

**Files:**
- Create: `crates/pulp-core/src/project.rs`, `crates/pulp-core/tests/project.rs`
- Modify: `crates/pulp-core/src/library.rs` (add `create_project`)

**Interfaces:**
- Consumes: `Storage`, `RelPath`, `Manifest`, `parse_markdown_item`, `check_readable`, `slugify`, `unique_name`.
- Produces:
  - `pub const PROJECT_DIRS: [&str; 6] = ["manuscript", "notes", "assets", "shelf", ".history", ".trash"];` `pub use crate::brand::MANIFEST_FILE;` `pub const HIDDEN_DIRS: [&str; 3] = [".history", ".trash", ".templates"];`
  - `pub struct Project<S: Storage> { /* storage, manifest, index: HashMap<Id, RelPath> */ }`
  - `Project::create(storage: S, name: &str) -> Result<Project<S>, ProjectError>`; `Project::open(storage: S) -> Result<(Project<S>, ReconcileReport), ProjectError>` (in this task, `ReconcileReport` exists but only `recreated_dirs` is filled; Task 11 completes it); `manifest(&self) -> &Manifest`; `path_of(&self, id: &Id) -> Option<&RelPath>`; `storage(&self) -> &S`.
  - `pub struct ScannedItem { pub id: Id, pub path: RelPath, pub modified: SystemTime }`; `pub fn scan<S: Storage>(storage: &S) -> Result<Vec<ScannedItem>, ProjectError>` (walks every folder except `HIDDEN_DIRS` and temp files; only `.md` files with a valid header count).
  - `#[derive(Default)] pub struct ReconcileReport { pub found: Vec<Id>, pub missing: Vec<Id>, pub duplicates_trashed: Vec<RelPath>, pub copies_reassigned: Vec<(RelPath, Id)>, pub conflicts: Vec<(Id, Vec<RelPath>)>, pub recreated_dirs: Vec<String> }`
  - `pub enum ProjectError { Storage(StorageError), Header(HeaderError), Manifest(ManifestError), Version(VersionError), NotFound(Id), NotAFolder(Id) }`
  - In `library.rs`: `Library::create_project(&self, name: &str) -> Result<PathBuf, LibraryError>` (folder `Projects/<unique slug>`).

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, library::Library, project::*, storage::*};

fn write_item(root: &std::path::Path, rel: &str, title: &str) -> pulp_core::id::Id {
    let h = ItemHeader::new_document(title, DocKind::Prose);
    let id = h.id.clone();
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, render_markdown_item(&ItemFile { header: h, body: "x\n".into() })).unwrap();
    id
}

#[test]
fn create_writes_manifest_and_folders() {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "The Storm").unwrap();
    assert!(d.path().join(MANIFEST_FILE).is_file());
    for dir in PROJECT_DIRS { assert!(d.path().join(dir).is_dir(), "{dir} missing"); }
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
    std::fs::write(d.path().join("manuscript").join(temp_name_for("arrival.md")), "junk").unwrap();
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test project`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/project.rs` and `Library::create_project`**

`open` reads and loads the manifest (refusing too-new projects), recreates missing `.history`/`.trash` only (not user folders), scans, and builds the ID→path index from the scan. Files are read only; nothing is rewritten in this task.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test project`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): project creation, opening and scanning"
```

---

### Task 9: Item and folder operations

**Files:**
- Create: `crates/pulp-core/src/items.rs`, `crates/pulp-core/tests/items.rs`

**Interfaces:**
- Consumes: `Project`, `Manifest`, header functions, `slugify`, `unique_name`, `normalize_text`.
- Produces (methods on `Project<S>`, all saving the manifest atomically when it changes):
  - `create_document(&mut self, folder: &Id, title: &str, kind: DocKind) -> Result<Id, ProjectError>`
  - `create_folder(&mut self, parent: Option<&Id>, title: &str) -> Result<Id, ProjectError>` (disk folder = parent path + unique slug)
  - `read_item(&self, id: &Id) -> Result<ItemFile, ProjectError>`
  - `save_body(&mut self, id: &Id, body: &str) -> Result<(), ProjectError>` (re-reads the current file from disk, keeps its header including unknown keys, replaces the body with the normalised new body)
  - `rename_item(&mut self, id: &Id, title: &str) -> Result<RelPath, ProjectError>` (updates header title and file name; carries companions)
  - `rename_folder(&mut self, id: &Id, title: &str) -> Result<(), ProjectError>` (renames the disk folder, updates its `path` and the paths of nested folders and indexed items)
  - `move_item(&mut self, id: &Id, to_folder: Option<&Id>, index: usize) -> Result<(), ProjectError>` (moves the file and companions when the folder changes; reorders otherwise)
  - `pub fn companions_of(path: &RelPath) -> Vec<RelPath>` (`<stem>.comments.json` and `<file>.meta.json` next to it)

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, manifest::Node, project::*, storage::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, pulp_core::id::Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] { Node::Folder(f) => f.id.clone(), _ => unreachable!() };
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
    let names: Vec<_> = [a, b, c].iter().map(|i| p.path_of(i).unwrap().as_str().to_string()).collect();
    assert_eq!(names, vec!["manuscript/ana.md", "manuscript/ana-2.md", "manuscript/ana-3.md"]);
}

#[test]
fn save_body_normalises_text_and_keeps_unknown_header_keys() {
    let (d, mut p, ms) = setup();
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    let f = d.path().join("manuscript/arrival.md");
    let raw = std::fs::read_to_string(&f).unwrap().replacen("---\n", "---\nfutureKey: keep me\n", 1);
    std::fs::write(&f, raw).unwrap();
    p.save_body(&id, "Line 1\r\nAc\u{0327}a\u{0303}o\r\n").unwrap();
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
    assert!(d.path().join("manuscript/the-arrival.comments.json").is_file());
    assert_eq!(p.read_item(&id).unwrap().header.title, "The Arrival");
}

#[test]
fn folders_create_rename_and_items_move_between_them() {
    let (d, mut p, ms) = setup();
    let part = p.create_folder(Some(&ms), "Part One").unwrap();
    assert!(d.path().join("manuscript/part-one").is_dir());
    let id = p.create_document(&ms, "Arrival", DocKind::Prose).unwrap();
    p.move_item(&id, Some(&part), 0).unwrap();
    assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/part-one/arrival.md");
    p.rename_folder(&part, "Part I").unwrap();
    assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/part-i/arrival.md");
    assert!(d.path().join("manuscript/part-i/arrival.md").is_file());
}

#[test]
fn unknown_ids_are_reported() {
    let (_d, p, _ms) = setup();
    assert!(matches!(p.read_item(&pulp_core::id::Id::new()), Err(ProjectError::NotFound(_))));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test items`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/items.rs`**

File names come from `slugify(title)` + `unique_name` against the target folder's current entries (excluding the item's own current name on rename). Every write uses `write_atomic`; every move uses `Storage::rename`, companions first-to-last in the same call sequence. The manifest is saved after each successful file operation.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test items`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): item and folder operations"
```

---

### Task 10: Project trash

**Files:**
- Create: `crates/pulp-core/src/trash.rs`, `crates/pulp-core/tests/trash.rs`

**Interfaces:**
- Consumes: `Project`, `companions_of`, `unique_name`, `Manifest::parent_of`/`remove_node`/`insert_item`.
- Produces:
  - `pub struct TrashEntry { pub format: u32, pub entry_id: Id, pub item_id: Id, pub original_path: RelPath, pub original_folder: Option<Id>, pub index: usize, pub deleted_at: String, pub files: Vec<String>, pub folder_node: Option<FolderNode> }` (JSON camelCase in `.trash/<entry-id>/entry.json`)
  - `pub fn store_in_trash<S: Storage>(storage: &S, files: &[RelPath], entry: TrashEntry) -> Result<Id, ProjectError>` (low-level; used by reconcile in Task 11)
  - On `Project<S>`: `trash_item(&mut self, id: &Id) -> Result<Id, ProjectError>`; `trash_folder(&mut self, id: &Id) -> Result<Id, ProjectError>` (whole disk folder and its node, including children); `list_trash(&self) -> Result<Vec<TrashEntry>, ProjectError>`; `restore_from_trash(&mut self, entry: &Id) -> Result<(), ProjectError>`

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, manifest::Node, project::*, storage::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, pulp_core::id::Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] { Node::Folder(f) => f.id.clone(), _ => unreachable!() };
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test trash`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/trash.rs`**

Item restore with a missing original folder goes to the project root (top-level tree, file at the root folder). Folder trash moves the disk folder to `.trash/<entry-id>/<folder-name>` and stores the node in `folder_node`; restore re-inserts the node at its original position (or top level) and moves the folder back with a unique name, updating paths.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test trash`
Expected: 5 passed.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): project trash with restore"
```

---

### Task 11: Reconcile rules

**Files:**
- Create: `crates/pulp-core/src/reconcile.rs`, `crates/pulp-core/tests/reconcile.rs`
- Modify: `crates/pulp-core/src/project.rs` (`open` calls `reconcile` and saves the manifest if it changed)

**Interfaces:**
- Consumes: `scan`, `ScannedItem`, `Manifest`, `store_in_trash`, header parse/render.
- Produces: `pub fn is_sync_conflict_name(name: &str) -> bool` (contains `.sync-conflict-` or `conflicted copy`, case-insensitive); `pub fn reconcile<S: Storage>(storage: &S, manifest: &mut Manifest, scanned: Vec<ScannedItem>) -> Result<(HashMap<Id, RelPath>, ReconcileReport), ProjectError>`; `Project::open` now returns the full report.

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use pulp_core::{header::*, project::*, reconcile::is_sync_conflict_name, storage::*};
use std::path::Path;

fn write_item_with(root: &Path, rel: &str, header: ItemHeader, body: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, render_markdown_item(&ItemFile { header, body: body.into() })).unwrap();
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
        let ms = p.manifest().folder_by_path("manuscript").unwrap().id.clone();
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
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_item_with(d.path(), "manuscript/a-copy.md", h, "same\n");
    let (_p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.duplicates_trashed.len(), 1);
    assert_eq!(report.duplicates_trashed[0].as_str(), "manuscript/a-copy.md");
    assert!(d.path().join("manuscript/a.md").is_file());
    assert!(!d.path().join("manuscript/a-copy.md").exists());
}

#[test]
fn sync_conflict_copies_are_reported_not_changed() {
    let d = new_project();
    let h = ItemHeader::new_document("A", DocKind::Prose);
    let id = h.id.clone();
    write_item_with(d.path(), "manuscript/a.md", h.clone(), "mine\n");
    write_item_with(d.path(), "manuscript/a.sync-conflict-20261106-120000-ABCDEFG.md", h, "theirs\n");
    let (_p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.conflicts.len(), 1);
    assert_eq!(report.conflicts[0].0, id);
    assert!(is_sync_conflict_name("Chapter (Gonçalo's conflicted copy 2026-11-06).md"));
}

#[test]
fn different_duplicates_give_the_newer_file_a_fresh_id() {
    let d = new_project();
    let h = ItemHeader::new_document("A", DocKind::Prose);
    let id = h.id.clone();
    write_item_with(d.path(), "manuscript/a.md", h.clone(), "original\n");
    std::thread::sleep(std::time::Duration::from_millis(20));
    write_item_with(d.path(), "manuscript/a-2.md", h, "edited copy\n");
    let (p, report) = Project::open(FsStorage::new(d.path())).unwrap();
    assert_eq!(report.copies_reassigned.len(), 1);
    let (path, new_id) = &report.copies_reassigned[0];
    assert_eq!(path.as_str(), "manuscript/a-2.md");
    assert_ne!(new_id, &id);
    assert_eq!(p.path_of(&id).unwrap().as_str(), "manuscript/a.md");
    assert_eq!(p.read_item(new_id).unwrap().header.title, "A (copy)");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test reconcile`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/reconcile.rs` and wire it into `Project::open`**

Order: group scanned items by ID; resolve groups with more than one file (identical bytes → `store_in_trash` the newer by `modified`; a sync-conflict name in the group → report as conflict, index the non-conflict file; otherwise the older keeps the ID and each newer file gets a fresh ID, a " (copy)" title and is rewritten atomically); then add found IDs at the end of the folder node whose `path` equals their directory; if no node matches, create folder nodes for each missing directory level, nested under the node of the nearest existing parent directory and titled with the directory name (top level if the file is at the project root); then list missing IDs. Save the manifest only if it changed.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test reconcile`
Expected: 5 passed. Then run the whole suite: `cargo test -p pulp-core`, expecting all tests to pass.

- [ ] **Step 5: Commit**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): reconcile rules for found, missing and duplicate items"
```

---

### Task 12: Snapshots, thinning and operation groups

**Files:**
- Create: `crates/pulp-core/src/history.rs`, `crates/pulp-core/tests/history.rs`

**Interfaces:**
- Consumes: `Project`, `Storage`, `Id`, `slugify`/`unique_name` (to re-derive file names after undo), `chrono`.
- Produces:
  - `pub enum HistoryTarget { Item(Id), Manifest }`
  - `pub enum SnapshotReason { Timer, Close, Named(String), Operation(Id) }`
  - `pub struct SnapshotInfo { pub id: Id, pub target: HistoryTarget, pub taken_at: DateTime<Utc>, pub reason: SnapshotReason }`
  - `#[derive(Clone, Debug)] pub struct SnapshotMeta { pub id: Id, pub taken_at: DateTime<Utc>, pub protected: bool }`; `pub fn snapshots_to_keep(snaps: &[SnapshotMeta], now: DateTime<Utc>) -> HashSet<Id>`
  - On `Project<S>`: `snapshot(&mut self, target: HistoryTarget, reason: SnapshotReason) -> Result<Id, ProjectError>`; `list_snapshots(&self, target: &HistoryTarget) -> Result<Vec<SnapshotInfo>, ProjectError>` (newest first); `restore_snapshot(&mut self, target: &HistoryTarget, snapshot: &Id) -> Result<(), ProjectError>` (takes a `Timer` snapshot of the current state first); `begin_operation(&mut self, label: &str, items: &[Id]) -> Result<Id, ProjectError>` (snapshots the items and the manifest under one group); `undo_operation(&mut self, group: &Id) -> Result<(), ProjectError>`; `thin_history(&mut self, now: DateTime<Utc>) -> Result<usize, ProjectError>`

- [ ] **Step 1: Write the failing tests**

```rust
// SPDX-License-Identifier: MPL-2.0
use chrono::{Duration, TimeZone, Utc};
use pulp_core::{header::*, history::*, id::Id, manifest::Node, project::*, storage::*};

fn setup() -> (tempfile::TempDir, Project<FsStorage>, Id) {
    let d = tempfile::tempdir().unwrap();
    let p = Project::create(FsStorage::new(d.path()), "X").unwrap();
    let ms = match &p.manifest().tree[0] { Node::Folder(f) => f.id.clone(), _ => unreachable!() };
    (d, p, ms)
}

#[test]
fn snapshot_and_restore_an_item() {
    let (_d, mut p, ms) = setup();
    let id = p.create_document(&ms, "A", DocKind::Prose).unwrap();
    p.save_body(&id, "first\n").unwrap();
    let snap = p.snapshot(HistoryTarget::Item(id.clone()), SnapshotReason::Named("before".into())).unwrap();
    p.save_body(&id, "second\n").unwrap();
    p.restore_snapshot(&HistoryTarget::Item(id.clone()), &snap).unwrap();
    assert_eq!(p.read_item(&id).unwrap().body, "first\n");
    // restoring took a safety snapshot of "second" first
    assert_eq!(p.list_snapshots(&HistoryTarget::Item(id)).unwrap().len(), 2);
}

#[test]
fn thinning_keeps_the_documented_set() {
    let now = Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap();
    let m = |h: Duration, protected| SnapshotMeta { id: Id::new(), taken_at: now - h, protected };
    let at = |y, mo, d, h, protected| SnapshotMeta { id: Id::new(), taken_at: Utc.with_ymd_and_hms(y, mo, d, h, 0, 0).unwrap(), protected };
    let a = m(Duration::hours(1), false);
    let b = m(Duration::hours(23), false);
    let c = at(2026, 11, 20, 8, false);   // same day as d, older
    let d = at(2026, 11, 20, 18, false);
    let e = at(2026, 9, 1, 10, false);    // same ISO week as f, older
    let f = at(2026, 9, 2, 10, false);
    let g = at(2026, 9, 1, 9, true);      // named: always kept
    let all = vec![a.clone(), b.clone(), c.clone(), d.clone(), e.clone(), f.clone(), g.clone()];
    let keep = snapshots_to_keep(&all, now);
    let expected: std::collections::HashSet<_> = [a.id, b.id, d.id, f.id, g.id].into_iter().collect();
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
    let group = p.begin_operation("Rename and edit", &[a.clone(), b.clone()]).unwrap();
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
    p.begin_operation("op", &[a.clone()]).unwrap();
    let far_future = Utc::now() + Duration::days(400);
    p.thin_history(far_future).unwrap();
    assert_eq!(p.list_snapshots(&HistoryTarget::Item(a)).unwrap().len(), 1);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p pulp-core --test history`
Expected: FAIL to compile.

- [ ] **Step 3: Implement `src/history.rs`**

Storage layout per M1a spec §3.6. `snapshots_to_keep`: protected → keep; age < 24 h → keep; age ≤ 30 days → keep the newest per UTC date; older → keep the newest per ISO week (`iso_week()` year + week). Snapshots belonging to a group are written with `protected: true` in their metadata. `undo_operation` restores the manifest first, then each item's bytes to its current indexed path, then renames each item's file to match its restored title (slug rule) and rebuilds the index, so names follow titles again.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p pulp-core --test history`
Expected: 4 passed. Then `cargo test -p pulp-core`, `cargo fmt -p pulp-core --check` and `cargo clippy -p pulp-core --all-targets -- -D warnings`, all clean.

- [ ] **Step 5: Commit and push**

```powershell
git add crates/pulp-core
git commit -s -m "feat(core): snapshots, thinning and operation groups"
git push
```
Expected: CI green on Windows, macOS and Linux.
