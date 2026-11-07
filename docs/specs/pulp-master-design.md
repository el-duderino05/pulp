# Pulp — Master Design Spec

**Status:** Draft for review (v8) · **Date:** 2026-11-06 · **Owner:** Gonçalo
**Working name:** "Pulp" (formerly "Quill"). Still to be checked against existing software (for example Red Hat's Pulp project and the PuLP Python library) before release. See §23.

This is the master spec: product vision, every decision taken so far, and the milestone plan. Each milestone gets its own short detailed spec when work on it starts (§25).

---

## 1. Vision and principles

Pulp is a free, open-source, community-developed writing app and project organiser for writers, students and creatives. It suits short and long work: fiction and non-fiction prose, screenplays and stage plays, poetry, essays, academic papers and journalism.

**Principles**, in priority order when they conflict:

1. **Your work outlives the app.** Projects are ordinary folders of plain, readable files.
2. **Robust.** No lost work: atomic saves, version history, layered trash, backups. **Pulp preserves content it doesn't understand** (from newer versions or future extensions) rather than deleting it.
3. **Light and minimal by default, extensible by the user.** Tools appear when the task calls for them.
4. **Runs everywhere.** Windows, macOS, Linux and Android in 1.0; iPad later (§24).
5. **No proprietary dependencies and no paywalled features.** Works fully offline. **Never sends data anywhere without asking** (§18.4).

**Non-goals for 1.0:** real-time collaboration, accounts or servers, code plugins.

**Permanent commitments:**

- Pulp never requires a server run by the project. Any future server is optional and run by whoever wants one (§24).
- **Pulp has no AI features**: no AI writing assistant, text generation, AI rewriting or AI analysis, in any version, built in or as an official plugin.

---

## 2. Platform and stack

| Part | Choice |
|---|---|
| App shell | Tauri 2 (Rust core, system WebView) |
| Interface | TypeScript + React |
| Text editor | TipTap (ProseMirror), three schemas (full / profile / note) built from an **extensible block registry** |
| Boards | React Flow (xyflow, MIT) |
| PDF and typesetting | Typst, embedded in the Rust core |
| Citations | CSL styles; CSL engine chosen at Milestone 3b: **hayagriva** (Rust, used by Typst) preferred, citeproc-js (CPAL/AGPL, requires visible attribution) as fallback. Bibliography stored as CSL-JSON. |
| Spell-check and thesaurus | Hunspell-format dictionaries and MyThes thesauruses. Checking engine in the Rust core if JavaScript proves too slow for large dictionaries (tested in Milestone 1a). |
| Storage | A **storage layer interface** that works with ordinary file paths (desktop) and system-granted folder access (Android, iOS) |
| Search and index | SQLite index in per-device app data (§4.6), rebuilt from files; a cache, never the source of truth |
| Optional add-on | Pandoc, for extra formats (LaTeX and others) |

Development happens on Windows 11. GitHub Actions builds macOS and Linux and runs the test suite (§21). Android is built locally with Android Studio's SDK.

---

## 3. Core concepts

- **Library:** Pulp's default home folder, holding `Projects/`, `Inbox/`, `Templates/`, `Backups/` and the library trash. Project folders can also be opened from anywhere on disk.
- **Project:** a folder with a manifest. A short piece can be a project with a single document.
- **Inbox:** where project-less (standalone) items live. It can have subfolders and has its own Shelf; deleted Inbox items go to the library trash. "Add to project" *moves* an item in; it is never copied, so no duplicates drift apart.
- **Protected folders:** Library, Projects, Inbox, Templates, Backups and Trash, plus each project's manifest and hidden folders, cannot be deleted, renamed or moved inside Pulp. On every start Pulp checks they exist, recreates any missing ones empty, and tells the user if something was unexpectedly gone.
- **Five item types**, each with its own icon and colour (types always differ by icon shape, never by colour alone):

| Type | Purpose | Engine | Stored as |
|---|---|---|---|
| **Writing document** | The actual writing | TipTap, full schema | Pulp Markdown (§4.3); Fountain for scripts |
| **Profile** | Structured reference: characters, places, items, contacts, sources | TipTap, profile schema | Pulp Markdown with headed sections |
| **Note** | Quick capture | TipTap, minimal schema | Pulp Markdown |
| **Board** | Visual canvas | React Flow | JSON + PNG preview |
| **Asset** | Images, PDFs, other files | Viewer only | Original file + `.meta.json` companion |

- **Item IDs** are time-ordered unique IDs (ULID or UUIDv7), so items created offline on different devices can never collide. The interface may show them shortened; files always store them in full.
- **Two layers.** The **item layer** is shared by all five types: permanent ID, tree position, metadata, links, history, trash, autosave. Each type then has its own **content engine**.
- **Folders** are plain containers with no body text and an optional **synopsis** written by the user. Recommended length is up to 300 characters, shown with a counter; the hard maximum is 1,000. It appears in the folder overview (the folder's synopsis plus its children's synopses), the Outliner and the side panel, and is never exported. Item synopses follow the same rule.
- **Author identity (no accounts):** an **author** is a person, with a name and ID set on first launch and stored in Library settings, so it is the same on all of that person's devices. It is recorded on comments, snapshots and Shelf fragments, so later collaboration can attribute work without changing any file formats.
- **Device ID:** each installation also has its own device ID, stored in per-device app data and used only to name per-device files (stats logs).

---

## 4. On disk

### 4.1 Layout

```
Pulp Library/
├── Projects/
│   └── The Storm/
│       ├── pulp-project.json        manifest: format version, ID, name, structure, project settings
│       ├── README.txt                (added on export) what Pulp is, where the writing lives
│       ├── manuscript/
│       │   ├── chapter-01-arrival.md
│       │   ├── chapter-01-arrival.comments.json
│       │   └── chapter-02-the-storm/
│       │       ├── scene-01-harbour.md
│       │       └── scene-02-lighthouse.md
│       ├── characters/  setting/  items/        profiles (folders come from the template)
│       ├── notes/
│       ├── boards/relationships.board.json (+ .png preview)
│       ├── shelf/                               saved fragments
│       ├── assets/  photo.jpg  photo.jpg.meta.json
│       ├── fonts/                               (optional) fonts that travel with the project
│       ├── bibliography.json                    CSL-JSON
│       ├── stats/<device-id>.json               per-device writing log
│       └── .templates/  .history/  .trash/      (hidden)
├── Inbox/   (with its own shelf/)
├── Templates/
├── Backups/
└── .trash/
```

### 4.2 One source of truth for each thing

- **Item metadata lives only in each item's own file**: the header of text files, inside board JSON, and in the `.meta.json` companion for assets (which can't hold a header). It is never duplicated in the manifest.
- **The manifest holds only structure and project settings:** format version, project ID and name, the tree (order, folders, variants), status, label and plotline lists, project-level settings (§19).
- **Folders live only in the manifest's tree.** Each folder entry holds `id`, `type`, `title`, `path`, `synopsis` and an ordered `children` list (by item ID). The merge engine treats each folder entry as an item by its ID, so edits to different folders never conflict.
- **Rebuild from files:** if the manifest is damaged or missing, Pulp rebuilds it from item headers. Only folder synopses and custom ordering can be lost, and Pulp says so. The manifest is also covered by atomic saves, snapshots and backups.
- Disk folders mirror the project tree. Ordering lives in the manifest, so file names never need number prefixes.
- **Folders renamed or moved outside Pulp** are detected on the next start (a known folder missing, an unknown folder with the same contents) and reconnected, keeping their ID and synopsis.
- **File names are readable and follow titles.** Pulp renames files when titles change, using the **file-name rule** (§4.8).
- **Companion files travel with their item:** an item's `.comments.json` or `.meta.json` and its Shelf fragments always move, rename, go to trash and restore together with it, including when an item moves from the Inbox into a project.
- The export `README.txt` mentions that folder synopses live in `pulp-project.json`.

### 4.3 Pulp Markdown

Writing documents, profiles and notes use **Pulp Markdown**: Pandoc-flavoured Markdown plus a small set of documented conventions. Its exact definition is the first deliverable of Milestone 1a, and **conversion between the editor and the file must be lossless**, enforced by an automated round-trip test suite (§21). It covers at least:

- **Header:** YAML front matter with the format version, the item's metadata (§5) and its document-level settings (§19).
- **Styles:** how Title, Subtitle, Heading, Subheading, Section, Subsection, Body and Quote map to Markdown.
- **Inline formatting:** underline and small caps (Pandoc span classes), sub/superscript, strikethrough.
- **Poetry:** line breaks and indentation preserved exactly (Pandoc line blocks).
- **Internal links:** standard Markdown links carrying the item ID, for example `[Character B](pulp:01JC3Z8K7Q…)` (full ID, shortened here). They survive any rename, and other Markdown apps show them as normal links with readable text. In the editor, typing `[[` still inserts them.
- **Flags:** inline marked spans (stripped on export).
- **Footnotes, citations (`[@key]`), maths (LaTeX), tables, images** from `assets/`.
- **Profile fields:** short fields and character name fields are stored in the YAML header; headed sections are the body.
- **Checklists:** Markdown task lists (`- [ ]` / `- [x]`), used by notes.
- **Images:** referenced by **path** (`assets/photo.jpg`), so other Markdown apps can show them. Pulp updates every reference when an asset is renamed or moved.
- **Unknown content:** blocks or attributes Pulp doesn't recognise are preserved untouched (principle 2).

**On Export item and Publish**, `pulp:` links become plain text or relative file links, so exported files work in other apps.

**Comments are not stored in the Markdown file.** They live in a companion file (`chapter-03.comments.json`), anchored by the commented text plus a few words before and after, using the shared **text anchor** component (§4.7). If the text was edited outside Pulp and the anchor can't be found, the comment is marked **detached**, never lost.

**Fountain files** (screenplay, stage play) keep the header inside a comment block (`/* … */`) at the top, which other Fountain apps ignore. Links are stored as Fountain notes in a recognisable form, because `[[ … ]]` already means "note" in Fountain.

**Writing document kinds:** prose, poetry (`.md`), screenplay, stage play (`.fountain`).

### 4.4 Format versioning

- The manifest, every file header, template, content pack and `.pulp` file carries a **format version** and a **minimum reader version** (the oldest Pulp version that can safely open it).
- **Additive changes** (new optional fields, new block types) do not raise the minimum reader version, because older versions preserve what they don't understand (§1).
- Only **breaking changes** raise it. Pulp then **asks before upgrading** a project, warning that Pulp versions older than the new minimum (for example on another device) will no longer open it, and takes a backup first.
- Older Pulp versions **refuse to open projects above their reader version** (read-only view at most), never damaging them.
- Merge checks that both sides use compatible versions.

### 4.5 Assets

- **Assets** are copied or moved into `assets/`, per a setting: Always copy (default) / Always move / Ask each time.
- Each asset gets a `.meta.json` companion file holding its ID, labels and synopsis, plus optional **alt text, caption, credit/licence and focal point** (the part of an image to keep when it is cropped, for example on a phone held upright). Alt text and captions are used by exports and the pre-flight check (§14).
- **Large files:** files over 25 MB trigger a warning. Images over the threshold get three options:
  - **Screen:** longest side 1,600 px, about 85% quality;
  - **Print:** longest side 3,600 px (a full page at 300 dpi);
  - **Keep original.**
  - TIFF and BMP are also offered conversion to PNG or JPEG.
  - Shrinking only affects Pulp's copy. In move mode the original is first sent to the operating system's trash, and the dialog says so.
  - PDF, audio and video compression is deferred (§26).
- **Images placed in text** are stored once in `assets/` and referenced from the document; there is never a second hidden copy. Each asset shows **"Used in"** (document and paragraph), computed from the references. Deleting an asset that is in use triggers a warning. Removing an image from the text keeps the asset, and an "Unused assets" view helps clean up.

### 4.6 Per-device data and sync safety

- **Never stored inside projects:** the search index (SQLite databases in syncing folders get corrupted), device settings, window layout, clipboard history, caches. These live in Pulp's per-device app data folder.
- **Temporary files** used for atomic saves use a name pattern sync services ignore, in the same folder as the target file.
- **Failed saves** (locked files, disk full and others) are handled as in §4.9.
- **Files changed outside Pulp** are detected and reloaded. If the same file was also changed inside Pulp, the **conflict screen** opens (§17).

### 4.7 Shared mechanisms

Four mechanisms are built once and reused, so the app stays smaller and each is tested in one place:

| Mechanism | What it does | Used by |
|---|---|---|
| **Text anchor** | Remembers a passage by its text, a few words before and after, and its rough position; finds it again after edits, or reports it as detached | Comments, Shelf origin and restore |
| **Find, review, replace** | Finds matches across files, shows each with its sentence, lets the user tick or untick, applies the result | Find & replace, rename from profile, link-text updates |
| **Operation group** | Takes one grouped snapshot of every file an operation will touch; "Undo [operation]" restores exactly that group | Find & replace, rename, split on import, merge, conflict resolution, format upgrade |
| **Conflict screen** | Shows two versions of an item and lets the user keep one, the other or both, saving the loser to history | External changes, sync-service conflicted copies, `.pulp` import and merge |
| **Command registry** | Every action is registered once as a named command; menus, shortcuts and the quick switcher all call the same commands | The whole interface; later, plugins (§22) and a command-line tool (§24) |
| **Rules checker** | Checks text against a rules file (flagged words, sentence length, word limits, required sections, figure requirements) | Writing guidelines (§7.6), pre-flight check (§14), journal template packs |

**Core and interface are separate:** opening and saving projects, search, checks and publishing are core functions in Rust, called by the interface through a documented boundary. The same boundary later serves plugins and a command-line tool.

### 4.8 Text, file names and schemas

- **Text encoding:** every text file is UTF-8 with LF line endings, in one normalised Unicode form (NFC). This keeps search, comparisons and merge fingerprints identical across Windows, macOS, Linux and Android.
- **File-name rule:**
  - Latin-script titles drop accents in the file name ("Capítulo 1 — A Chegada" → `capitulo-1-a-chegada.md`); the title inside the file keeps them. Titles in other scripts (Cyrillic, Chinese…) keep their own script, normalised.
  - Characters forbidden on any supported system (`: ? * " < > | \ /`) are removed, Windows reserved names (`CON`, `AUX`, `NUL`…) are avoided, and names are capped in length to stay well within Windows path limits.
  - Names are compared ignoring case; a clash gets a numeric suffix (`-2`).
- **Schemas:** each JSON format (manifest, `.meta.json`, `.comments.json`, `template.json`, board files, rules files) has a published, machine-readable schema. Pulp validates files on load against it, format upgrades are tested against it, and other tools can read and write Pulp projects correctly.

### 4.9 When the disk and Pulp disagree

Sync services, other apps and file managers will produce states Pulp didn't create. The rules:

| Situation | What Pulp does |
|---|---|
| **A valid item file is not in the manifest** (for example after a manifest sync conflict) | It is added to the tree **where it sits on disk** (at the end of that folder, creating the folder entry if needed), and a **"Found items"** notice lists it. Never ignored. |
| **The manifest lists an item whose file is missing** (for example mid-sync) | The item shows as **missing** (greyed, with a reason) and reconnects when the file arrives. Pulp never removes a manifest entry on its own; only the user can. |
| **Two files share one ID** | Identical content: one is kept. Different content from a sync conflict: conflict screen. A deliberate copy (for example duplicated in File Explorer): the newer-found file becomes a **new item with a fresh ID**, labelled "copy", with a notice. |
| **A file's header was stripped or damaged by another app** | Pulp **re-identifies** it by its known path (from the device's index) and restores the header, with a notice. |
| **The project is open on another device** | Each open session refreshes a small **presence marker** in the project about once a minute. Finding another device's recent marker triggers a warning; nothing is locked. |
| **Any save fails** (file locked by a sync service, disk full, permission denied, network drive offline) | Short retry, then a clear warning. The old file is untouched; the text stays in memory and in the recovery copy until a save succeeds. |
| **The Library folder is missing at start** (drive unplugged, folder moved) | Pulp asks the user to locate it, never silently creating a new empty one. A **"Move Library"** command moves it deliberately. |
| **A content pack used by a project is not installed** (theme, guideline pack, style sheet) | Falls back to the default, with a notice naming the missing pack. |
| **A font used by a project is missing** | Falls back with a visible notice. User-added fonts can be **copied into the project's `fonts/` folder** so they travel with it. |

---

## 5. Metadata

- **Automatic:** ID, created and modified dates, word count.
- **Optional, all items:** title, status (editable list per project; default Idea → Draft → Revised → Final), labels (free-form, coloured), synopsis.
- **Archived** is a built-in status available in every project: archived items are hidden from the tree by default (a toggle shows them) but stay searchable, linkable and restorable. It hides without deleting.
- **Private** (any item or folder): private items are left out of `.pulp` exports, Publish and DOCX review copies unless explicitly ticked, and every export dialog says how many items it excluded. **Contact profiles are Private by default.** This is not encryption (§26); it prevents the most likely leak, which is sending.
- **Writing documents only:** word target, word maximum, deadline, "include in export", **Featuring** (the profiles appearing in this chapter or scene), and **POV** (optional link to the point-of-view character's profile).
- **Story date (writing documents):** when the scene happens *in the story*, as free text shown exactly as written ("the 3rd day of Frostmoon, Year 412"), plus an optional real date used only for sorting. Custom calendars come later (§26).
- **Plotlines (writing documents):** one or more plotlines from the project's plotline list (kept in the manifest like statuses and labels).
- **Scripts:** Featuring is recorded **per scene** inside the script, as well as per document (Milestone 4).
- **Profiles** show **Appears in** (from Featuring) and **Mentioned in** (from links) as two separate lists.
- **Variants (writing documents only):** alternate versions kept as sibling files (`chapter-12-ending.variant-b.md`), grouped under the document. The manifest records the active variant, a dropdown in the document header switches it, and Publish uses the active variant. Rules:
  - links and Featuring point to the **document**, never to a single variant;
  - word counts, targets and stats count only the **active** variant;
  - search covers every variant, labelled;
  - "Used in" and backlinks show which variant a reference comes from.

---

## 6. Templates and content packs

**One template format for every kind:** a folder containing `template.json` (format version, name, description, author, version, kind, preview image) and content in the normal item formats. It is shared as a single `.pulptemplate` file.

- Kinds: project, writing document, profile, note, board, style sheet, publish preset.
- **Template Library:** a screen listing everything in `Templates/`, filtered by kind, with previews. Built-in templates are read-only; editing one creates your own copy.
- **Using a template copies it, never links it.** Editing a template affects future uses only. Templates a project uses are also copied into its `.templates/`, so the project works on any device.
- Project templates are self-contained and bring their own profile and board templates.
- "Save as template" works on any item, folder, board or project. Templates are edited in the normal editor, under a "Template" banner.
- **Six built-in project templates:** Blank · Short piece · Novel · Non-fiction book · Screenplay / stage play · Academic paper.
- **Built-in document templates:** Prose chapter · Poem · Essay · Academic paper · Blog post · Diary entry · Screenplay · Stage play.
- **Board templates:** Ideas (one column per idea type), Timeline, Relationship map, Setting overview.
- **The Novel template** includes a **scene type** label set (Flashback, Dream, Prologue, Epilogue, Interlude), editable like any labels, and a **Story bible** profile with Premise, Themes and Setting sections.
- **Built-in profile templates** include a **Character interview**: a character profile whose sections are questions (for example "What does she fear most?", "What would she never forgive?"), alongside the standard character profile.

**Content packs** are the 1.0 extension mechanism. They contain no code, so they can't run anything. They use the same package format as templates, and are validated on install (§20).

| Pack type | Examples |
|---|---|
| Templates | project, document, profile, board, style sheet, publish preset |
| Language packs | dictionary, thesaurus, hyphenation, readability formula, language rules, script fonts |
| Fonts | user typefaces |
| Themes | colour schemes |
| Writing guidelines | rules files (§7.6) |
| Comment categories | review category sets |
| Interface translations | UI languages |

---

## 7. Writing documents

### 7.1 Modes

Write and Revise are switched inside each document (Ctrl+E). Profiles and notes have a single editing mode with Revise-style tools available. Publish is a separate workspace (§14).

| | **Write** | **Revise** |
|---|---|---|
| Purpose | Drafting only | Improving the text |
| Full screen | On by default (setting) | Off |
| Visible | Text, optional typewriter scrolling, thin target/timer bar | Project tree, side panel, comments, highlights, word count, status |
| Spell-check underlines | Off by default (toggle) | On by default (toggle) |
| Tools | — | Rewrite, Reorder, Comments, Flags, Guidelines, Shelf, synonyms |

**Undo:** undo history covers the whole editing session across autosaves. When a file is reloaded after an outside change, undo history for that document resets, and Pulp says so. A snapshot is always taken before the reload.

### 7.2 Revise tools

- **Rewrite:** select a sentence and try alternatives side by side, then pick one. **Unused alternatives go to the Shelf**, labelled with their source.
- **Reorder:** drag handles on paragraphs, with a toggle for sentence-level reordering.
- **Comments:** margin notes on a passage, stored in the companion file (§4.3). Each comment has an author (§3), date, optional **category** (default list: Clarity, Structure, Argument, Style, Grammar; editable, shareable as a pack), **threaded replies** and a **resolved** status. Comments never appear in exports unless "Include comments for review" is switched on for a DOCX export (§15.3).
- **Flags:** mark text as needing attention, shown as red text or red highlight (setting). A "Flags" button in the search bar and the quick switcher (Ctrl+O) jump between them. **Flags are removed automatically on export.**
- **Read aloud:** reads the selection or the document from the cursor, using the voices built into the operating system, so it works offline. Hearing prose read is a classic way to catch clumsy sentences.
- **Right-click menu:** spelling suggestions, synonyms, Add to dictionary, Comment, Flag, Send to Shelf, Link, Read aloud.

### 7.3 Shelf (save for later)

- One Shelf per project, with filters "This document / All", plus one Shelf for the Inbox.
- It holds cut passages and unused rewrite alternatives. Each fragment can be dragged back into any document.
- **Each fragment remembers its origin** (via the text anchor, §4.7): source document, author, date, the last few words before and first few words after the cut, and its rough position (for example "paragraph 14 of 40"). The card shows this ("Cut from Chapter 3, between '…the harbour lights' and 'Ana didn't answer…'").
- **Restore to original place** finds the surrounding words and reinserts the fragment between them. If that text has changed, it falls back to the recorded position and asks for confirmation.
- **Fragments are notes** of kind "fragment", stored in `shelf/` with their origin in the header. They get search, trash, history, links and merge like any note, with nothing built specially.

### 7.4 Formatting

- **Styles, not free formatting:** Title, Subtitle, Heading, Subheading, Section, Subsection, Body, Quote.
- **Direct formatting (prose and poetry):** bold, italic, underline, strikethrough, small caps, subscript, superscript, bulleted and numbered lists.
- **Direct formatting (scripts):** bold, italic, underline only, as supported natively by Fountain, so `.fountain` files stay clean for other screenwriting apps.
- **Poetry:** line breaks and indentation are kept exactly as typed.
- **Tables:** basic tables (rows, columns, header row) in prose documents.
- **Figures and tables with captions and cross-references** ("see Figure 3"), numbered automatically (Milestone 3b).
- **Multi-panel figures:** a figure holding a grid of images with sub-labels (a), (b), (c), one shared caption, and cross-references to individual panels ("Figure 2b") (Milestone 3b).
- **Editor appearance** (what you see while writing): font, size, line spacing and alignment per style. Set per project, with per-document override. Kept separate from **publish styles**, which belong to publish presets (§14). This way you can draft in a comfortable font and still submit in manuscript format.
- **Footnotes** are written once. Whether they come out as footnotes or endnotes is chosen at publish time.
- **Maths:** LaTeX syntax, shown rendered.
- **Images:** JPEG, PNG, GIF, TIFF, WebP, SVG, BMP, embedded from `assets/`.
- **Citations:** inserted from the project bibliography, with autocomplete while typing `[@` (§13).
- **Internal links:** typing `[[` opens a fuzzy search of the project. A link opens its target in the side panel. Links are bidirectional (backlinks). Storage format in §4.3.

### 7.5 Spell-check, thesaurus and suggestions

- Bundled languages: English, Spanish, French, Portuguese (Portugal), Portuguese (Brazil). Each dictionary's licence is verified before bundling.
- **Language packs** let users add any language (§6).
- **Any alphabet, from Milestone 1:** nothing assumes the Latin alphabet. Words and sentences are counted with the web engine's built-in language rules (`Intl.Segmenter`), which handle languages without spaces (Chinese, Japanese, Thai). Layout is built so it can mirror for right-to-left scripts. Fonts for other scripts (Noto) come with language packs rather than being bundled. Polished right-to-left support and further bundled packs come after 1.0 (§26).
- **Word-count rule** is documented in the app's help, since counts can differ slightly from Microsoft Word's.
- Language is a project default with per-document override.
- **Personal dictionary** at Library level (it follows the user across devices) and a **project dictionary**, to which names from profiles (characters, places, items) are **added automatically**.
- Spell-check applies to all text items (documents, profiles, notes); board cards later.
- Word suggestions while typing: off by default, with a toggle.
- Thesaurus is offline.

### 7.6 Writing guidelines

- These are content packs: **rules files** (the same format the pre-flight check uses, §4.7) listing flagged words, maximum sentence length, passive voice, adverbs and readability thresholds.
- Their findings appear as highlights in Revise mode, and only when the user activates the pack.
- Built-in packs: Plain style (Orwell's rules), Hemingway-style, ASD-STE100-style, Readability. The Readability pack only adds highlights in the text; whether scores are shown at all is controlled by the single "Show readability" switch (§7.7).
- **Readability** uses the formula matching the document's language:
  - English: Flesch Reading Ease and Flesch-Kincaid Grade Level.
  - Spanish: Fernández Huerta / Szigriszt-Pazos.
  - French: Kandel–Moles.
  - Portuguese: Flesch adaptation.
  - Non-English languages show the reading-ease score only.

### 7.7 Stats (document settings → Stats)

- **Counts:** words, characters, sentences, paragraphs, average sentence and word length, longest sentence.
- **Time:** reading time; speaking time and estimated page count for scripts.
- **Style:** most repeated words, vocabulary variety, dialogue share (fiction).
- **Progress:** words today, this session, per day. **Net words, counted in both modes**, with an optional split by mode.
- **Not counted as writing:** imports, merges, snapshot restores, Shelf restores and template creation. Large pastes (over 200 words) are shown separately as "pasted".
- **Readability** is hidden by default behind one **"Show readability"** switch, which controls Stats, graphs and the score display everywhere. It comes with a short explanation:
  - it measures sentence and word length;
  - it suits academic, organisational and news writing, and is less meaningful for creative writing;
  - it does not measure quality.
  - Results are described in words ("comfortable for most adult readers"); the number shows on hover.

---

## 8. Script editor (screenplay and stage play)

- Element cycling with Enter and Tab: scene heading → action → character → dialogue → parenthetical → transition.
- **Autocomplete of character names from character profiles** and of locations from setting profiles.
- Automatic scene numbering and a scene navigator.
- **Per-scene Featuring** (§5).
- **Stage plays** follow theatre conventions (act and scene headings, stage directions, character names), handled by the editor and by the Play preset at publish time.
- Stored as Fountain; import and export FDX.
- Formatting: bold, italic, underline only (§7.4).
- Later: revision colours.

---

## 9. Profiles and notes

**Profiles:**

- Optional short fields at the top: a portrait image and a few one-line fields.
- **Character profiles** have optional **name fields**: first name, surname, full name, and nicknames/other names. They power renaming (below), script autocomplete and more accurate "Mentioned in".
- **Academic projects** have an **Article info** profile: authors (with ORCID), affiliations, abstract, keywords and funding, as structured fields that feed the title page at publish time (Milestone 3b).
- Then headed sections that can be added, removed and renamed.
- Limited formatting, internal links, hyperlinks and embedded images.
- Appears in / Mentioned in lists (§5).
- Any profile can be saved as a template.
- Example (character): About, Backstory, Relevance, Themes, Subtext, Narrative arc, Inspirations, Remarks.

**Renaming from a profile** (built on the find, review, replace engine, §4.7): when a profile's name changes (for example first name "Ana" → "Joana"), Pulp asks where to apply it:

- This profile only
- Links only (always safe: links store the profile's ID)
- **Whole project, with review** (default)
- Selected folders or chapters

For a project-wide rename, matches are shown with their surrounding sentence, in three groups:

| Group | Contains | Default |
|---|---|---|
| **Certain** | Links pointing to this profile | Ticked |
| **Likely** | Plain-text matches in documents that feature this profile, where no other profile shares the name | Ticked |
| **Ambiguous** | Names shared with another profile (two Anas, a family surname), documents featuring both, or names that are common words ("Rose", "Will") | Unticked; reviewed one by one |

- Name fields tell Pulp **which part** of the name changed, so only that part is searched; listed nicknames can be included.
- Whole words only, and case is preserved (the script cue `ANA` becomes `JOANA`).
- The rename is an operation group (§4.7): one grouped snapshot first, and "Undo rename" restores it in one step.
- Optional final step: turn the remaining plain mentions into links, so the next rename is fully automatic.
- Featuring is stored by ID, so it never needs changing.
- Works for every profile type (characters, places, items, contacts).

**Contact and source profiles** are Private by default (§5). When the first one is created, Pulp shows a one-time notice that profiles are plain files (and synced if the project is in a sync folder), so confidential sources need care until per-project encryption arrives (§26).

**Notes:**

- Plain text with light formatting, clickable checkboxes, internal links, links to project images, and hyperlinks that open in the default browser.

---

## 10. Boards

**In 1.0:**

- Card types: text, checklist, image, web link. **Link previews (title and image) are fetched only when you add the link**, and can be switched off entirely (§18.4). Links open in the browser.
- **Project-item cards:** a live preview of a profile, document or note that opens on click.
- Arrows between cards, with optional labels.
- Nested boards, colours, grouping.
- **Quick capture** (Ctrl+Shift+I) adds an idea card to the project's Ideas board from anywhere.
- **Promote a card** into a note, profile or document; the card stays behind as a link.
- Board PNG previews are generated when a board is closed or exported, not on every save.
- Templates: Ideas, Timeline, Relationship map, Setting overview.

**Deferred:** freehand drawing and pen input, shapes, card ratings.

**Excluded:** embedded video player (videos open in the browser), due dates on cards.

---

## 11. Navigation, search and project views

- **Project tree** (binder): manuscript order, drag-and-drop, item icons and colours.
- **Outliner:** the manuscript as a sortable table (title, synopsis, status, words, POV, Featuring). Separate from Boards.
- **Plot grid:** a second Outliner view, with documents as rows and chosen characters, plotlines or labels as columns, filled from Featuring, POV, plotlines and labels. Shows at a glance who appears where and which plotlines advance.
- **Story timeline:** scenes in **story order** (by story date) instead of manuscript order, for flashbacks and non-linear books. Scenes without a sortable date are listed separately.
- **Project map:** a picture of how documents and profiles link to each other, drawn automatically from links, Featuring and POV, using the Boards engine. Read-only; clicking opens an item.
- **Storage indicator:** the project screen always shows where the project lives on disk, and flags when it is inside a sync folder (OneDrive, Dropbox, iCloud Drive, Syncthing), so it is never unclear where work is kept.
- **Full-text search:** documents (all variants), profiles, notes, Shelf, comments, board card text, asset captions and alt text, and the bibliography, with filters by type.
- **Find & replace across the project:** documents, profiles, notes and link text, using the find, review, replace engine (§4.7), with options for whole words and case. Runs as an operation group, so it can be undone in one step.
- **Quick switcher (Ctrl+O):** items, headings, comments and flags. This covers WriteMonkey-style "jumps". Typing `>` first lists **commands** instead (any action in Pulp, from the command registry, §4.7).
- **Side panel:** metadata, Featuring, backlinks, and linked items opened beside the text.
- **Very long documents:** past about 50,000 words, Pulp suggests splitting the document (without blocking), since typing performance is guaranteed up to 20,000 words per document (§21).
- Links stay within one project in 1.0. Broken links are flagged, never deleted. When an Inbox note moves into a project, its links move with it.

---

## 12. Targets, sessions, focus, deadlines and graphs

- **Targets** (stored on items): word target and maximum per document; word target per project.
- **Sessions** (temporary): an optional timer and/or word goal.
- **Focus mode** = a session plus a soft lock:
  - full screen, interface hidden, duration in 5-minute steps;
  - leaving early needs a deliberate confirmation;
  - time away is recorded in the session summary.
- **Deadlines:** on projects and documents, using the device clock. In-app only in 1.0 (a deadlines list and reminders on opening). Pulp shows days left and words needed per day.
- **Goal planner:** deadline + target + writing days and days off give a daily target that recalculates as you go.
- **Graphs:**
  - words per chapter, coloured by status, with target and average lines;
  - progress line against the ideal path to the deadline;
  - calendar heatmap (streaks optional, off by default);
  - session summary (words, time, pace, time away);
  - readability per chapter (only when shown).
- **Stats storage:** one log file per device in `stats/`, recording author ID. Graphs add them together, so logs never conflict during a merge.

---

## 13. References

- Each project has its own bibliography in `bibliography.json` (CSL-JSON). It works offline on any device.
- **Citations are formatted from data**, so changing the citation style never requires editing the text.
- **Citation styles** use CSL (about 10,000 available: APA, MLA, Chicago, Harvard, NP 405, ABNT…). Style is set as a Library default, with project and document overrides; a publish preset uses that style unless changed there.
- **Adding sources:**
  - manual entry;
  - DOI lookup (contacts a server only when you trigger it);
  - import from **BibTeX** (including JabRef libraries), **RIS** and **EndNote XML**;
  - a live picker from **Zotero** when it runs on the same computer. Zotero is optional, never required.
- **Autocomplete** while typing `[@`, searching author, title and year.
- **Open the cited PDF:** a bibliography entry can link to a PDF in the project's assets (or to Zotero's attachment when Zotero is running). Right-click a citation › Open PDF.
- The bibliography is generated automatically at publish time.
- The CSL engine is chosen at Milestone 3b (§2), including any required attribution.

---

## 14. Publish workspace

Opened from the main menu, or from inside a document or project with that source already filled in.

1. **Source:** a single document, Inbox items, or a project.
2. **Contents:** pre-filled from manuscript order, include-in-export settings and active variants. Drag to reorder; untick to leave out.
3. **Format:**
   - preset and publish styles;
   - front and back matter, title page, table of contents;
   - page size, margins, headers;
   - page numbers, with "omit on this page" (title and chapter-opening pages);
   - footnotes or endnotes, citation style.
4. **Preview:** source and output side by side. Device previews:
   - print: A4, Letter, A5, 6×9" paperback;
   - e-reader, tablet, phone.
5. **Pre-flight check:** before exporting, Pulp lists problems:
   - preset rules, written as rules files (§4.7): word limits per section, required sections, figure format and resolution;
   - missing alt text, unresolved citations, broken links, leftover flags, detached comments.
   - Problems are warnings; you can still export.
6. **Export.**

- **Built-in presets:** Book (print + ebook), Play / Screenplay, Academic article, News article, Blog post.
- **Presets are templates.** Save your own recipe, and it becomes one click next time.
- Private items are left out unless ticked (§5). Flags are always stripped from exports. Comments are stripped too, unless **"Include comments for review"** is switched on for a DOCX export (§15.3).
- HTML export uses **templates**, so blog and web layouts can be added as content packs.
- **Web story export:** one self-contained folder (HTML, images, styles) that can be hosted anywhere (GitHub Pages, a personal site) with no server or subscription. One simple, clean template in v1.

---

## 15. Getting files in and out

Four flows, each with one job.

### 15.1 Add files (bring something in)

Drag files onto any folder in the tree (for example *Project Z › Act 2*) or into the Inbox, or right-click › *Add files…*.

| File type | Can become |
|---|---|
| Fountain, FDX | Script document, note, or asset |
| Markdown, TXT, DOCX, ODT, RTF, HTML | Writing document, note, or asset (read-only original) |
| PDF, EPUB, images, audio, video, anything else | Asset only |

- For editable files, Pulp **asks every time by default** (writing document / note / asset). A "Remember for this file type" option exists, changeable in settings.
- **Split on import** (optional): split a long file into separate documents, either at headings (choosing which level becomes chapters and, optionally, which level becomes scenes inside them) or at a scene-break marker such as `***`. Runs as an operation group (§4.7).
- Images dropped into text are stored in `assets/` and referenced (§4.5).
- Imported HTML and Markdown are **sanitised** (§20).
- PDF content is never converted to text. PDFs are assets.
- **Import report:** every import ends with a short list of anything not carried over (for example FDX revision colours, DOCX text boxes).
- **Tracked changes in DOCX:** converted into comments ("Prof. Silva suggests: '…' → '…'"), so nothing is lost or silently accepted. Before comments exist (Milestone 2), the original text is imported and the import report lists the tracked changes.
- Import quality for ODT and RTF without Pandoc is tested in Milestone 1a. If it isn't good enough, that format moves to the Pandoc add-on.

### 15.2 Export item (send one item out quickly)

Right-click › *Export as…*. Raw content, default styling, no layout. Internally this is Publish with a built-in **Plain** preset and no workspace, so there is a single export pipeline.

- Documents and notes: Markdown, TXT, DOCX.
- Scripts: Fountain, FDX.
- Assets: the original file.

### 15.3 Publish (formatted output)

Anything involving layout or combining several documents happens in the Publish workspace (§14).

| Built in (all platforms) | Pandoc add-on (desktop) |
|---|---|
| PDF (Typst), EPUB, DOCX, ODT, HTML, FDX, Markdown | LaTeX, others |

- **DOCX review round-trip:** Pulp comments (with authors and replies) export as Word comments; comments and replies come back when the reviewed DOCX is added back to the project.

### 15.4 Project transfer

- Export and import `.pulp` with merge (§17).
- **New project from folder:** turn any folder of Markdown, Fountain or text files (an Obsidian vault, an old drafts folder) into a Pulp project.

---

## 16. Safety

- **Autosave** about 2 seconds after typing stops, and when switching documents or closing. Writes are **atomic**: a temporary file is written, then swapped in. This applies to every file Pulp writes, including the manifest.
- **Snapshots** are separate from autosave. A snapshot is taken:
  - every 10 minutes of active editing;
  - when a document is closed or a session ends;
  - **before any risky operation**, as an operation group (§4.7): merge, restore, split on import, find & replace, rename, reload after an outside change, format upgrade.
- **Snapshot thinning:** everything from the last 24 hours, then one per day for 30 days, then one per week. **Named snapshots** (Ctrl+Shift+S) are kept until deleted. Restore is in 1.0; visual comparison comes later.
- **Project backups:** a zip **at most once a day** (on the first close that day), keeping the last 10. The `.history` folder is excluded, since snapshots are their own safety net. Default location: `Backups/` in the Library. Pulp offers to move backups outside a sync folder if it detects one, to save cloud storage.
- **Trash:**
  - project trash (`.trash/`, any item type) and library trash (whole projects, Inbox items);
  - never auto-empties by default (optional auto-empty after 30 days);
  - restore returns items to their original place, with links reconnected;
  - permanent delete sends files to the operating system's trash.
- **Crash recovery:** unsaved text is mirrored to a local recovery copy in per-device app data and offered back after a crash.
- **Mobile:** Pulp saves immediately whenever it goes to the background, since Android and iPadOS can close background apps at any moment.

---

## 17. Moving projects, syncing and merging

- **Export project:** produces one `.pulp` file, a zip of the project folder plus `README.txt`. "Include version history" is on by default; "Include trash" is off by default. It opens with any zip tool.
- **Import:** unpacks into the Library (validated, §20). If the same project already exists, Pulp offers to merge.
- **Conflict screen first, full merge later:** from Milestone 1b, the conflict screen (§4.7) handles a file changed in two places (keep mine / keep theirs / keep both, loser saved to history). Milestone 5 extends it into the full merge below.
- **Merge** pairs items **by ID** and compares content fingerprints:

| Situation | Result |
|---|---|
| Identical | Skipped |
| On one side only | Added |
| Only one side changed (detected through history fingerprints) | Taken automatically |
| Both changed, or deleted on one side and edited on the other | User chooses: this / other / both, with "apply to all" |
| No history available to tell which side changed | User chooses |

- Comments merge per comment (by ID), so a reviewer's comments and replies are added without conflicting with the author's. This automatic comment merge applies from the moment comments exist (Milestone 2), including in the conflict screen.
- **Private items** are excluded from `.pulp` exports unless ticked (§5).
- A summary appears before anything is applied, and every replaced version goes into history.
- "Keep most recent" is available but is not the default.
- The same engine handles **sync-service conflicted copies** (OneDrive, Dropbox, Syncthing).
- **Peer review with Pulp users:** a reviewer comments on the exported `.pulp` file, and the merge brings their comments back.

**Sync by platform:**

| Platform | How projects sync |
|---|---|
| Windows, macOS, Linux | Any sync folder (OneDrive, Dropbox, iCloud Drive, Syncthing…) |
| Android | Projects live in a folder chosen through the system folder picker. **Syncthing** is the recommended free sync, since the OneDrive and Dropbox Android apps don't keep a synced folder on the device. `.pulp` transfer and merge is the universal fallback. |
| iPad (later) | Folders chosen through the Files app, including iCloud Drive, OneDrive and Dropbox |

---

## 18. Interface

### 18.1 Look and feel

- **Themes:** Light / Sepia / Dark, plus a separate high-contrast toggle that works with all three. Themes are content packs.
- **Full screen:** F11 (Windows, Linux) / Ctrl+Cmd+F (macOS).
- **Windows:** one project per window; several windows allowed on desktop.
- **Tablet layout:** same features; the project tree and side panel become slide-out drawers, with larger touch targets.
- **Clipboard:**
  - copy-on-select available, off by default;
  - Ctrl+V pastes plain text;
  - Ctrl+Shift+V opens a menu with clipboard history (last 10, kept while Pulp is open, never written to disk) and "paste with formatting".
  - Double-click selects a word, triple-click a paragraph.
- **Fonts bundled** (free, matching the widths of common commercial fonts):
  - Liberation Serif (in place of Times New Roman)
  - Liberation Sans (in place of Arial / Helvetica)
  - Carlito (in place of Calibri)
  - TeX Gyre Bonum (in place of Bookman)
  - Gelasio (in place of Georgia)
  - EB Garamond
  - Courier Prime
  - free typewriter faces (Pica, Elite, Letter Gothic styles)
  - Atkinson Hyperlegible, OpenDyslexic
  - Installed system fonts are also listed, and users can add their own.
- **Accessibility (1.0):** full keyboard navigation, screen-reader labels, adjustable interface size, reduced-motion setting, dyslexia-friendly fonts.
- **Interface languages:** English and Portuguese (Portugal) in 1.0. All interface text lives in translation files, and translations are content packs.

### 18.2 First launch and new projects

1. Interface language.
2. Author name (§3).
3. Library location, with a short explanation of sync folders.
4. Update checks: ask (§18.4).
5. Optional sample project.

**New projects open straight into their first document**, cursor ready, with the template's structure visible in the tree. A project never starts on a blank screen with no obvious place to write.

### 18.3 Default shortcuts

Ctrl = Cmd on macOS. All are configurable, with clash detection.

| Shortcut | Action | Shortcut | Action |
|---|---|---|---|
| Ctrl+K | Insert link (as in Word, Google Docs, LibreOffice) | Ctrl+Shift+L | Send to Shelf |
| Ctrl+O | Quick switcher | Ctrl+Shift+I | Quick-capture idea (inside Pulp) |
| Ctrl+F | Search document | Ctrl+1…6 / Ctrl+0 | Styles / Body |
| Ctrl+Shift+F | Search / replace in project | Ctrl+\ | Side panel |
| Ctrl+E | Write ⇄ Revise | Ctrl+Shift+S | Named snapshot |
| F11 | Full screen | Ctrl+Shift+P | Publish |
| Ctrl+Shift+V | Paste menu | Ctrl+, | Settings |
| Ctrl+Shift+M | Comment | Ctrl+Shift+B | Insert citation |
| Ctrl+Shift+R | Flag | `[[` / `[@` | Insert link / citation (typed alternatives) |

- **Keyboard layouts:** defaults avoid Ctrl+Alt+letter, because on Windows AltGr equals Ctrl+Alt and types characters on many European layouts (for example "ś" on Polish, "µ" on German). All defaults are tested on Portuguese, Spanish, French, German and Polish layouts.
- The `[[` and `[@` triggers are configurable, since `[` needs AltGr on Portuguese, Spanish, French and German keyboards.

### 18.4 Network and privacy

Pulp works fully offline. It contacts the network only:

- for **update checks**, if the user agreed on first launch (changeable any time);
- for **link previews**, when a web link is added to a board (can be switched off);
- for **DOI lookup**, when the user triggers it;
- for **Zotero**, on the same computer only.

No telemetry, no analytics. Error logs stay on the device; the user can choose to attach them to a bug report.

---

## 19. Settings scope

Each setting lives in exactly one place:

| Scope | Stored in | Examples |
|---|---|---|
| **Device** | per-device app data | device ID, theme, high contrast, shortcuts, window layout, interface size, update checks |
| **Library** | Library settings file | author name and ID, asset copy/move default, "remember for this file type" choices, "Show readability", default citation style, personal dictionary |
| **Project** (travels with it) | manifest and project files | editor appearance, spell-check language, project dictionary, citation style override, statuses, labels, comment categories, active guideline packs |
| **Document** (travels with it) | file header | appearance, language and citation style overrides, targets, deadline, include in export |

---

## 20. Security

Pulp opens files from outside (Markdown, HTML, DOCX, `.pulp`, templates, packs) inside a WebView with disk access, so:

- **Sanitisation:** raw HTML and scripts in imported or externally edited files are never executed. Only safe formatting is displayed.
- **Archive safety:** every zip Pulp opens (`.pulp`, `.pulptemplate`, packs) is checked so no file can be written outside its target folder ("zip slip"), with limits on unpacked size and file count ("zip bombs").
- **Least privilege:** Tauri's permission system grants the interface only the commands it needs; file access goes through the storage layer, scoped to the Library and opened projects.
- **Content packs** are validated on install: known file types only, no executable content.
- **External links** always open in the system browser, never inside Pulp.

---

## 21. Quality and testing

Gonçalo does not review code directly, so automated tests are the main proof that the software works. They run on every change via GitHub Actions on Windows, macOS and Linux.

**Always tested:**

- **Pulp Markdown round-trip:** editor → file → editor is lossless for every supported element, including unknown content.
- **Merge:** every row of the merge table (§17), including comments and folder entries.
- **Autosave and crash safety:** interrupted saves never corrupt files; recovery copies restore correctly; locked-file retries.
- **Format versioning:** upgrades of older projects, refusal of newer ones.
- **Every import format**, against a library of sample files.
- **Archive safety and sanitisation** (§20).
- **Manifest rebuild** from files.
- **Shared mechanisms** (§4.7): text anchor re-finding after edits, find-review-replace, operation-group undo, conflict screen, rules checker.
- **Disk disagreements** (§4.9): every row of the table, including found items, missing items, duplicate IDs, header re-identification, presence markers and every save-failure cause.
- **Reader versions:** additive changes keep old versions working; breaking changes ask first.
- **Private items** never appear in exports unless ticked.
- **Text and file names** (§4.8): NFC normalisation, the file-name rule (accents, forbidden characters, reserved names, case clashes), schema validation of every JSON file.
- **Default shortcuts** on Portuguese, Spanish, French, German and Polish keyboard layouts.

**Performance targets** (make "lightweight" testable):

- A 300,000-word, 500-document project opens in under 3 seconds on a mid-range laptop.
- Typing never lags, including in a single 20,000-word document.
- Search across that project returns in under 1 second.

---

## 22. Extensibility

- **1.0:** content packs only (§6). They are safe because they contain no code.
- **v2:** code plugins with a permission system, built on the command registry and the core boundary (§4.7), designed knowing that iOS restricts apps that download and run new code (plugins may be unavailable on iPad).
- **Community contributions:** templates, guideline packs, language packs, themes and translations, each with its own content licence (for example Creative Commons, or the OFL for fonts).

---

## 23. Licence, name, distribution and community

- **Code licence:** MPL-2.0.
- **Trademark policy** on the final name: the code is open to all, but nobody can ship or host a product under the name.
- **Name:** "Pulp" is a working name only. Before any public release, the final name must be checked against existing software and the EU trademark register (EUIPO). The `.pulp` and `.pulptemplate` extensions change with it.
- **Dependency licences** are tracked in a list and checked before each release (code libraries, fonts, dictionaries, CSL styles).
- **Recommended:** a short consultation with an open-source lawyer before the public 1.0.
- **Distribution:**
  - desktop: GitHub Releases with Tauri's signed auto-updater (desktop only; update checks need consent, §18.4);
  - Linux: Flatpak and AppImage;
  - Android: F-Droid and Google Play;
  - iPad (later): App Store.
- **Code signing:** Windows certificate and Apple developer account, before public release.

**Community and governance** (from Milestone 1a):

- The code repository is **public from the start**, under the working name. Its name is kept easy to change; GitHub redirects renamed repositories.
- Contributions carry a **DCO sign-off** (Developer Certificate of Origin: a one-line statement in each commit that the contributor has the right to submit it), keeping the licence clean without a formal contributor agreement.
- **CONTRIBUTING guide, code of conduct, issue templates and a public roadmap** (based on §25).
- **User guide:** written alongside each milestone, covering what that milestone adds, plus the sample project offered at first launch.

---

## 24. Future readiness

Decisions in 1.0 that keep later directions cheap.

**iPad (after 1.0; iPhone possibly later as a companion for quick capture, notes and reading):**

- the storage layer already handles system-granted folder access;
- nothing desktop-only in the core (Pandoc stays optional);
- the updater is a per-platform switch;
- the tablet layout is shared with Android;
- needs a Mac with Xcode and a macOS CI runner for builds and signing.

**Collaboration (staged, with no central Pulp servers):**

1. Asynchronous: shared project folders + merge + threaded comments with authors. Mostly possible with 1.0.
2. Live editing on the same network, peer-to-peer, using Yjs with TipTap.
3. Optional self-hosted sync servers run by institutions (universities, newsrooms).

What 1.0 does now to keep this open: local author identity (§3), threaded comments, every edit going through the editor's normal change mechanism, the storage layer behind an interface.

**Multimedia and blogging:**

- new block types (audio, video, embeds, galleries, pull quotes, callouts) added through the editor's block registry;
- a "Story" document kind with scrollytelling blocks, building on the web story export;
- audio and video subtitle tracks (WebVTT), image focal points used in responsive layouts;
- HTML export templates for blog and web layouts;
- direct publishing to platforms (WordPress, Ghost…) via v2 plugins.

**Academic:** JATS XML export (the standard publishers accept); community **journal template packs** carrying a journal's rules for the pre-flight check.

**Boards and interactive fiction:** export a Timeline board as an interactive timeline; branching, interactive stories with export to Twine's open format.

**Command-line tool:** because core functions are separate from the interface (§4.7), a `pulp` command-line tool can later open, check and publish projects without the app (for example, publishing a paper automatically on every change).

**Forward compatibility:** format versioning (§4.4), published schemas (§4.8) and "preserve what you don't understand" (§1) protect files across all of the above.

---

## 25. Milestones

Each milestone produces something usable and starts with its own short detailed spec.

| # | Milestone | Contents |
|---|---|---|
| 1a | **Walking skeleton** | Public repository with DCO, CONTRIBUTING and code of conduct; Pulp Markdown definition and round-trip tests; ID format, text encoding, file-name rule and schemas (§4.8); format and reader versions (§4.4); the disk-disagreement rules for found items, missing items, duplicate IDs, failed saves and a missing Library (§4.9); command registry and core boundary; storage layer; Library, one project, tree, prose documents (Write/Revise shell), autosave, snapshots, trash, protected folders, format versioning, per-device data, first launch. **Early technical tests:** spell-check speed, ODT/RTF import quality, Android open-and-save. Goal: Gonçalo writes in Pulp within weeks. |
| 1b | **Foundation** | Shared mechanisms (text anchor, find-review-replace, operation groups, conflict screen); notes, profiles, links and backlinks, Archived status, storage indicator, search, find & replace, quick switcher, themes, spell-check, tables, Add files (with split on import), Export item, New project from folder, image handling and "Used in", asset details (alt text, caption, credit, focal point), manifest rebuild, crash recovery, presence markers, header re-identification, Private flag, personal and project dictionaries, import report, long-document suggestion, security rules. Windows, macOS and Linux via CI. From day one: language-independent word counting, translation files, keyboard navigation, screen-reader labels. |
| 2 | **Writer's toolkit** | Rename from profile (with name fields and review groups), POV, story date and plotline fields, scene types, Story bible and Character interview templates, read aloud, Shelf (with origin and restore), Rewrite, Reorder, comments (threaded, companion files, automatic merge by ID, tracked changes converted to comments), missing-pack fallback, flags, rules checker and writing guidelines, thesaurus, targets, sessions, focus mode, stats and graphs, Template Library and built-in templates, content packs, interface translation (EN/PT). |
| 3a | **Publish** | Publish workspace and presets (Book, Academic article, News article, Blog post, Plain), pre-flight check (on the rules checker), Typst PDF, EPUB/DOCX/ODT/HTML/Markdown, device previews, footnotes and endnotes, project fonts and missing-font fallback, web story export. |
| 3b | **Academic** | References (BibTeX/RIS/EndNote XML import, Zotero, autocomplete, open cited PDF, style hierarchy), Article info profile, maths, figure and table captions with cross-references, multi-panel figures, DOCX review round-trip. |
| 4 | **Scripts & Boards** | Script editor (with per-scene Featuring), Fountain/FDX import and export (including FDX publish), Play / Screenplay preset, plot grid, story timeline, project map, Boards (with Ideas board and quick capture), Outliner, variants. |
| 5 | **Travel** | Android (with save-on-background), `.pulp` export and import, full merge engine (extending the conflict screen). → **Release 1.0** |

Every milestone also updates the user guide (§23) and the always-tested list (§21).

---

## 26. Deferred to v2 and later

- Code plugins
- Per-project encryption (the design leaves room for it). **First priority for v2**, for journalists' sources and private writing.
- Split view
- System notifications
- Cross-project links
- Board drawing and pen input
- Card ratings
- Snapshot comparison (diff)
- Script revision colours
- Track changes (and DOCX track-changes round-trip)
- "Send manuscript to board" as cards
- iPad; iPhone companion
- Collaboration stages 2 and 3 (§24)
- Multimedia block types, Story document kind and direct blog publishing (§24)
- JATS export and journal template packs (§24)
- Interactive timeline export and branching stories (§24)
- Direct import of Scrivener projects (`.scriv`) and Bibisco projects
- Multi-voice read aloud (a different voice per character, set in character profiles)
- More board templates: flow map
- Custom story calendars (as a content pack format) for story dates
- Series: several projects sharing one set of profiles (needs cross-project links)
- Life events on character profiles, shown on the story timeline
- Compression of PDF, audio and video assets (likely as an optional add-on, since the tools involved are large)
- Polished right-to-left support and further bundled language packs

---

## 27. Open items

- **Final product name** (blocks trademark policy, file extensions and store listings; not needed before Milestone 1).
- **CSL engine** (hayagriva or citeproc-js): decided at Milestone 3b.
- **ODT/RTF import path** (built in or Pandoc add-on): decided by the Milestone 1a test.
- **Spell-check engine** (JavaScript or Rust): decided by the Milestone 1a test.
- **Exact readability wording**: drafted in Milestone 2.
- **Built-in guideline pack contents**: drafted in Milestone 2.
