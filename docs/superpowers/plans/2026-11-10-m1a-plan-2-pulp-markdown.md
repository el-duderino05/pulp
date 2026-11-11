# Milestone 1a, Plan 2: Pulp Markdown — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The written definition of Pulp Markdown for prose (the 1a subset), and a TypeScript converter between the editor's document and the Markdown body of an item file. A round-trip test suite proves the conversion is lossless and never changes content it does not understand.

**Architecture:** The converter is a UI-free TypeScript module in `src/markdown/`. It does not depend on React, TipTap or Tauri. The editor document is ProseMirror JSON, the same shape TipTap's `getJSON()` produces, and is validated by a `prosemirror-model` schema that Plan 3's TipTap setup must match. Parsing uses `markdown-it` with `markdown-it-sub`, `markdown-it-sup` and a small Pulp rule for bracketed spans. Serialising is Pulp's own code, so the canonical form is exactly what this plan defines. The YAML header is not part of this plan: `pulp-core` owns it (Plan 1) and hands Plan 3 the body as a string.

**Tech Stack:** TypeScript (strict, the existing `tsconfig.json`), Node 24 in CI.
- Runtime dependencies, pinned exactly: `markdown-it@15.0.3`, `markdown-it-sub@2.0.0`, `markdown-it-sup@2.0.0`, `prosemirror-model@1.25.12`.
- Dev dependencies, pinned exactly: `vitest@5.0.5`, `fast-check@4.10.2`.

**Spec:**
- `docs/specs/2026-11-06-m1a-walking-skeleton.md`: §1, the Plan 2 row; §2, "the core treats document bodies as opaque text".
- `docs/specs/pulp-master-design.md` (v8.1):
  - §4.3 Pulp Markdown
  - §7.4 Formatting
  - §4.8 Text
  - §21, the round-trip requirement and performance targets
  - §1, principle 2: preserve what you don't understand

## Global Constraints

- Every `.ts` file starts with `// SPDX-License-Identifier: MPL-2.0`.
- **Where the code lives:** everything in this plan is in `src/markdown/`. It never imports React, TipTap, Tauri or anything in `src/` outside `src/markdown/`.
- **Dependencies** are exactly those in Tech Stack, at exactly those versions, with no `^` or `~` ranges. Add any other dependency only by a ledgered ruling.
- **Text the converter writes** is UTF-8, LF line endings, Unicode NFC. The body ends with exactly one `\n`, except that an empty document's body is `""`.
- **Blocks** are separated by exactly one blank line.
- **Names shared with Plan 3's editor:**
  - nodes `doc`, `paragraph`, `heading`, `blockquote`, `bulletList`, `orderedList`, `listItem`, `rawBlock`, `hardBreak`, `text`
  - marks `bold`, `italic`, `strike`, `underline`, `smallCaps`, `subscript`, `superscript`, in that order (`MARK_ORDER`)
- **Style mapping (master §7.4):**
  - Title, Subtitle, Heading, Subheading, Section and Subsection are `heading` levels 1–6.
  - Body is `paragraph`.
  - Quote is `blockquote`.
- **Lossless:** for every editor document `d`, `parseBody(serializeBody(d))` deep-equals `normalizeDoc(d)`. Every change `normalizeDoc` makes is listed in Task 1's definition and is invisible or nearly so.
- **Never change what it doesn't understand:** a top-level block is turned into editable content only if writing it back reproduces its source text exactly. Otherwise it is kept, byte for byte, as a `rawBlock`. The only change Pulp may make to a file's text outside editable blocks is to blank lines between blocks.
- **Version floor:** `npm run typecheck` and `npm test` pass on Windows, macOS and Linux in CI. The existing Rust CI job stays as it is.
- **Tests:**
  - Never weaken a test to make it pass.
  - A property-test failure is fixed in the code, never by narrowing the generator, unless a ledgered ruling adds a normalisation to Task 1's definition.
- Every commit uses `git commit -s`.

## Review Focus

1. **Ordinary prose that looks like Markdown syntax** must come back exactly. Examples:
   - `2 * 3`, `C# & F#`, `$5`, an email `ana@x.pt`
   - a dialogue paragraph starting with a hyphen (`- Olá`, common in Portuguese prose)
   - a paragraph starting `1984. That year`

   Pinned in Task 3 (`escapes`, `line-start characters`) and Task 6 (property P1).
2. **Formatting that ends on punctuation next to a letter** (`**Stop,**then`), or starts after a letter on punctuation, must still be bold after reopening. This is CommonMark's flanking rule. Pinned in Task 3 (`flanking close`, `flanking open`) and Task 6 (P1).
3. **A file edited in another app** must never be altered beyond blank lines between blocks. Examples: Obsidian, Typora or VS Code, using `_italic_`, `* bullets`, hard-wrapped lines, tables, links or HTML. Pinned in Task 5 and Task 6 (P4).
4. **Decomposed and non-Latin text** must round-trip as NFC, exactly. Examples: macOS NFD accents, emoji with skin-tone modifiers and joiners, Greek, CJK, Arabic. Pinned in Task 4 (`NFD input`) and Task 6 (fixture `scripts.md`, generator alphabet).
5. **A writer's blank paragraphs and spaces at the start or end of a line** (manual indents) must survive a save and reopen. Pinned in Task 3 (`empty paragraph kept`, `spaces at line edges`).

---

## File Structure

```
pulp/
├── package.json                       deps, "typecheck" and "test" scripts (Task 2)
├── vitest.config.ts                   test config (Task 2)
├── .github/workflows/ci.yml           new "web" job (Task 2)
├── docs/specs/pulp-markdown.md        the Pulp Markdown definition (Task 1)
└── src/markdown/
    ├── model.ts                       PulpNode, PulpDoc, PulpMarkName, MARK_ORDER (Task 2)
    ├── schema.ts                      pulpSchema, checkDoc (Task 2)
    ├── normalize.ts                   normalizeDoc, UnsupportedContentError (Task 2)
    ├── test-builders.ts               doc, p, h, quote, ul, ol, li, t, br, raw (Task 2)
    ├── serialize.ts                   serializeBlocks, serializeBody (Task 3)
    ├── cases.ts                       CANONICAL_CASES, shared by Tasks 3 and 4 (Task 3)
    ├── spans.ts                       markdown-it rule for [x]{.underline} / {.smallcaps} (Task 4)
    ├── vendor.d.ts                    type declarations for markdown-it-sub / -sup (Task 4)
    ├── parse.ts                       parseBody (Tasks 4 and 5)
    ├── index.ts                       public API (Task 6)
    ├── arbitrary.ts                   fast-check generators (Task 6)
    ├── fixtures/*.md                  canonical and foreign sample bodies (Task 6)
    └── *.test.ts                      one test file per module
```

---

### Task 1: The Pulp Markdown definition

**Files:**
- Create: `docs/specs/pulp-markdown.md`

**Interfaces:**
- Produces: the authoritative definition that Tasks 2–6 implement and the owner reads. Section numbers below are the ones later tasks cite.

The document is written for writers and contributors, in plain language, with one short example per rule. Its content is fixed by this task. Wording is the implementer's.

- [ ] **Step 1: Write `docs/specs/pulp-markdown.md` with these sections**

1. **Scope.** This defines the prose subset for Milestone 1a. Later milestones add the rest of master §4.3:
   - links, flags, footnotes, citations, maths, tables, images
   - poetry line blocks, checklists, profile fields

   Until then, those appear in files only as protected blocks (§8).
2. **The file.** The YAML header is defined in the M1a spec §3.2 and handled by the core. This document defines the body, which starts on the line after the closing `---`.
3. **How a body is read.**
   - Reading follows CommonMark 0.31.2 plus these Pandoc extensions:
     - strikethrough `~~x~~`
     - subscript `~x~`
     - superscript `^x^`
     - bracketed spans with exactly the classes `.underline` or `.smallcaps`
   - The reference implementation is markdown-it 15 with markdown-it-sub, markdown-it-sup and Pulp's span rule.
   - Pulp always writes bodies that CommonMark and Pandoc read the same way. For files Pulp did not write, where the two disagree, Pulp follows CommonMark.
4. **Elements.** A table with these columns: style or format, editor node or mark, canonical Markdown.

   | Style or format | Editor node or mark | Canonical Markdown |
   |---|---|---|
   | Title, Subtitle, Heading, Subheading, Section, Subsection | `heading` levels 1–6 | `# x` … `###### x` |
   | Body | `paragraph` | `x` |
   | Quote | `blockquote` (paragraphs only) | `> x`, and `>` alone between paragraphs |
   | Bulleted list | `bulletList` | `- x` |
   | Numbered list | `orderedList` | `1. x` |
   | Bold | `bold` | `**x**` |
   | Italic | `italic` | `*x*` |
   | Strikethrough | `strike` | `~~x~~` |
   | Underline | `underline` | `[x]{.underline}` |
   | Small caps | `smallCaps` | `[x]{.smallcaps}` |
   | Subscript | `subscript` | `~x~` |
   | Superscript | `superscript` | `^x^` |
   | Line break | `hardBreak` | `\` at the end of the line |

   Marks nest in `MARK_ORDER`, outermost first: bold, italic, strike, underline, smallCaps, subscript, superscript. Subscript and superscript are always innermost.
5. **Text and escaping.** A literal character is written as follows:
   - (a) These characters are always backslash-escaped: `` \ ` * _ [ ] { } < > ~ ^ $ @ | & # ``.
   - (b) At the start of a line, meaning the start of a block's text or right after a line break:
     - `-`, `+`, `=` and `:` are backslash-escaped.
     - If the line starts with 1–9 digits followed by `.` or `)`, that `.` or `)` is escaped.
   - (c) A space or tab that is the first or last character of a line is written `&#32;` or `&#9;`.
   - (d) Inside subscript or superscript, every ASCII punctuation character is backslash-escaped, and nothing else is.
   - (e) If a run of `*` or `~~` delimiters would not be read as opening or closing because of CommonMark's flanking rules (§6.2), the neighbouring literal character outside the run is written as a decimal character reference (`&#110;` for `n`). Example: `**word,**&#110;ext`.
6. **Paragraph details.**
   - An empty paragraph is written `&nbsp;`. A paragraph whose source is exactly `&nbsp;` reads back as empty, while a real no-break space character stays a character.
   - A document that is a single empty paragraph has the body `""`.
   - A line break is `\` followed by a new line.
   - Headings never contain line breaks.
7. **Lists.**
   - **Markers:** bullet lists use `-`, ordered lists use `N.`. The second, fourth and further adjacent lists of the same kind use `*` or `N)` instead, so they stay separate lists.
   - **Numbering** starts at the list's `start` and counts up.
   - **Indentation:** an item's continuation lines and nested blocks are indented by the width of its own marker (`- ` = 2, `10. ` = 4).
   - **Loose lists:** a list is loose (blank lines between items and between an item's blocks) if any item has two or more paragraphs, or a nested numbered list that doesn't start at 1. Otherwise it is tight.
   - **Item content:** an item starts with a paragraph, followed by paragraphs or lists.
8. **Content Pulp doesn't understand.**
   - A top-level block becomes editable only if Pulp would write it back exactly as it is in the file. Otherwise it is kept as a protected raw block, byte for byte.
   - Lines that belong to no block become raw blocks too. Example: link reference definitions.
   - Blank lines between blocks are written as exactly one.
   - Example: a table, a code block, `_italic_` and a hard-wrapped paragraph each stay as written.
   - Two protected blocks that end up next to each other (for example, after the paragraph between them is deleted) may be read back as one protected block. Their text is unchanged.
9. **What the editor normalises.** These are the exact `normalizeDoc` rules from Task 2, in plain language:
   - NFC
   - new lines become line breaks
   - formatting never starts or ends on whitespace
   - subscript and superscript never cover whitespace
   - superscript wins over subscript
   - strikethrough is removed where it touches subscript
   - trailing line breaks are dropped
   - an empty document is one empty paragraph
10. **Limits in 1a.**
    - Files written in another style are kept but not editable. This includes `_italic_`, `*` bullets and hard-wrapped lines; editing them arrives with "New project from folder" in 1b.
    - Pandoc collapses runs of spaces when publishing; Pulp's own files keep them.
    - A list placed directly after a protected list with the same marker is read back as part of that protected block. Its text is kept, but it is no longer editable.
11. **Examples.** Link to `src/markdown/fixtures/`. Every fixture there is tested.

- [ ] **Step 2: Check the document against this task**

Run: `grep -c "^## " docs/specs/pulp-markdown.md`
Expected: `11`.

- [ ] **Step 3: Commit**

```bash
git add docs/specs/pulp-markdown.md
git commit -s -m "docs(spec): define Pulp Markdown for prose (M1a)"
```

---

### Task 2: Toolchain, document model, schema and normalisation

**Files:**
- Modify: `package.json`, `.github/workflows/ci.yml`
- Create: `vitest.config.ts`, `src/markdown/model.ts`, `src/markdown/schema.ts`, `src/markdown/normalize.ts`, `src/markdown/test-builders.ts`
- Test: `src/markdown/schema.test.ts`, `src/markdown/normalize.test.ts`

**Interfaces:**
- Produces, in `model.ts`:
  - `type PulpMarkName = 'bold' | 'italic' | 'strike' | 'underline' | 'smallCaps' | 'subscript' | 'superscript'`
  - `const MARK_ORDER: readonly PulpMarkName[]`, in exactly that order
  - `interface PulpMark { type: PulpMarkName }`
  - `interface PulpNode { type: string; attrs?: Record<string, unknown>; content?: PulpNode[]; marks?: PulpMark[]; text?: string }`
  - `type PulpDoc = PulpNode & { type: 'doc' }`
- Produces, in `schema.ts`:
  - `const pulpSchema: Schema` (prosemirror-model)
  - `function checkDoc(doc: PulpDoc): void`, which throws on an invalid document
- Produces, in `normalize.ts`:
  - `class UnsupportedContentError extends Error`
  - `function normalizeDoc(doc: PulpDoc): PulpDoc`
- Produces, in `test-builders.ts`. All of these return normalised ProseMirror JSON (`content` omitted when empty, `marks` omitted when empty, marks in `MARK_ORDER`):
  - `doc(...blocks)`, `p(...inline)`, `h(level, ...inline)`, `quote(...paragraphs)`
  - `ul(...items)`, `ol(start, ...items)`, `li(...blocks)`
  - `t(text, ...marks: PulpMarkName[])`, `br()`, `raw(markdown)`

The schema, in `prosemirror-model` content expressions:

| Node | Content | Attrs |
|---|---|---|
| `doc` | `block+` | — |
| `paragraph` (group block) | `inline*` | — |
| `heading` (group block) | `text*` | `level` (1–6, required) |
| `blockquote` (group block) | `paragraph+` | — |
| `bulletList` (group block) | `listItem+` | — |
| `orderedList` (group block) | `listItem+` | `start` (default 1) |
| `listItem` | `paragraph (paragraph \| bulletList \| orderedList)*` | — |
| `rawBlock` (group block, atom) | — | `markdown` (string, required) |
| `hardBreak` (group inline) | — | — |
| `text` (group inline) | — | — |

The marks are defined in `MARK_ORDER`. `subscript` excludes `superscript` and `strike`. `superscript` excludes `subscript`. `strike` excludes `subscript`. Headings allow all marks.

- [ ] **Step 1: Add the toolchain**

Add `markdown-it@15.0.3`, `markdown-it-sub@2.0.0`, `markdown-it-sup@2.0.0` and `prosemirror-model@1.25.12` to `dependencies`. Add `vitest@5.0.5` and `fast-check@4.10.2` to `devDependencies`. Use exact versions and `npm install`, so `package-lock.json` updates.

Add these scripts:
- `"typecheck": "tsc -p tsconfig.json"`
- `"test": "vitest run"`

Write `vitest.config.ts`, using `defineConfig` from `vitest/config`, with `test: { include: ['src/**/*.test.ts'], environment: 'node' }`.

Add a `web` job to `.github/workflows/ci.yml`:

```yaml
  web:
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 24
          cache: npm
      - run: npm ci
      - run: npm run typecheck
      - run: npm test
```

- [ ] **Step 2: Write the failing schema tests** in `src/markdown/schema.test.ts`

```ts
import { describe, expect, test } from 'vitest';
import { checkDoc, pulpSchema } from './schema';
import { MARK_ORDER } from './model';
import { doc, p, h, quote, ul, ol, li, t, br, raw } from './test-builders';

describe('pulpSchema', () => {
  test('mark order', () => {
    expect(MARK_ORDER).toEqual(['bold', 'italic', 'strike', 'underline', 'smallCaps', 'subscript', 'superscript']);
    expect(Object.keys(pulpSchema.marks)).toEqual([...MARK_ORDER]);
  });

  test('accepts every element and keeps its JSON form', () => {
    const d = doc(
      h(1, t('Title', 'bold')), p(t('a'), br(), t('b', 'italic', 'underline')),
      quote(p(t('q')), p()), ul(li(p(t('x')), ol(2, li(p(t('y')))))), raw('| a |'),
    );
    expect(() => checkDoc(d)).not.toThrow();
    expect(pulpSchema.nodeFromJSON(d).toJSON()).toEqual(d);
  });

  test.each([
    ['heading level 7', doc({ type: 'heading', attrs: { level: 7 }, content: [t('x')] })],
    ['list inside a quote', doc({ type: 'blockquote', content: [ul(li(p(t('x'))))] })],
    ['list item starting with a list', doc(ul({ type: 'listItem', content: [ul(li(p(t('x'))))] }))],
    ['line break in a heading', doc({ type: 'heading', attrs: { level: 1 }, content: [t('a'), br()] })],
    ['subscript with superscript', doc(p({ type: 'text', text: 'x', marks: [{ type: 'subscript' }, { type: 'superscript' }] }))],
    ['empty document', { type: 'doc', content: [] }],
  ])('rejects %s', (_name, bad) => {
    expect(() => checkDoc(bad as never)).toThrow();
  });
});
```

`Node.check()` does not check attribute ranges, and may not check excluded mark pairs. `checkDoc` adds explicit checks for:
- heading `level` being an integer 1–6
- `start` being an integer 0–999 999 999
- `markdown` being a string
- no text carrying two marks that exclude each other

- [ ] **Step 3: Write the failing normalisation tests** in `src/markdown/normalize.test.ts`

```ts
import { describe, expect, test } from 'vitest';
import { normalizeDoc, UnsupportedContentError } from './normalize';
import { doc, p, h, ul, ol, li, t, br } from './test-builders';

const n = normalizeDoc;

describe('normalizeDoc', () => {
  test('NFC and NUL', () => {
    expect(n(doc(p(t('Cafe\u0301 \u0000'))))).toEqual(doc(p(t('Café \uFFFD'))));
  });
  test('new lines become line breaks in paragraphs and spaces in headings', () => {
    expect(n(doc(p(t('a\r\nb\rc\nd'))))).toEqual(doc(p(t('a'), br(), t('b'), br(), t('c'), br(), t('d'))));
    expect(n(doc(h(2, t('a\nb'))))).toEqual(doc(h(2, t('a b'))));
    expect(n(doc({ type: 'heading', attrs: { level: 2 }, content: [t('a'), br(), t('b')] }))).toEqual(doc(h(2, t('a b'))));
  });
  test('formatting never starts or ends on whitespace', () => {
    expect(n(doc(p(t('x'), t(' bold\u00a0', 'bold'), t('y'))))).toEqual(doc(p(t('x '), t('bold', 'bold'), t('\u00a0y'))));
  });
  test('subscript and superscript never cover whitespace', () => {
    expect(n(doc(p(t('a b', 'subscript'))))).toEqual(doc(p(t('a', 'subscript'), t(' '), t('b', 'subscript'))));
  });
  test('superscript wins over subscript; strike yields to subscript and its touching character', () => {
    const both = { type: 'text', text: 'x', marks: [{ type: 'subscript' }, { type: 'superscript' }] };
    expect(n(doc(p(both as never)))).toEqual(doc(p(t('x', 'superscript'))));
    expect(n(doc(p(t('ab', 'strike'), t('2', 'subscript'))))).toEqual(doc(p(t('a', 'strike'), t('b'), t('2', 'subscript'))));
    const strikeSub = { type: 'text', text: '2', marks: [{ type: 'strike' }, { type: 'subscript' }] };
    expect(n(doc(p(strikeSub as never)))).toEqual(doc(p(t('2', 'subscript'))));
  });
  test('line breaks carry no marks; trailing line breaks are dropped', () => {
    const markedBr = { type: 'hardBreak', marks: [{ type: 'bold' }] };
    expect(n(doc(p(t('a', 'bold'), markedBr as never, t('b', 'bold'), br(), br())))).toEqual(doc(p(t('a', 'bold'), br(), t('b', 'bold'))));
  });
  test('merges neighbours, drops empty text, sorts and dedupes marks', () => {
    const messy = { type: 'text', text: 'c', marks: [{ type: 'italic' }, { type: 'bold' }, { type: 'italic' }] };
    expect(n(doc(p(t('a'), t(''), t('b'), messy as never)))).toEqual(doc(p(t('ab'), t('c', 'bold', 'italic'))));
  });
  test('attributes: unknown dropped, start clamped, empty document gets a paragraph', () => {
    expect(n(doc(ol(-3, li(p(t('x'))))))).toEqual(doc(ol(0, li(p(t('x'))))));
    expect(n(doc({ type: 'orderedList', attrs: { start: 1, type: null }, content: [li(p(t('x')))] }))).toEqual(doc(ol(1, li(p(t('x'))))));
    expect(n({ type: 'doc' })).toEqual(doc(p()));
    expect(n(doc(ul({ type: 'listItem' })))).toEqual(doc(ul(li(p()))));
  });
  test('unknown node or mark types throw', () => {
    expect(() => n(doc({ type: 'image' }))).toThrow(UnsupportedContentError);
    expect(() => n(doc(p({ type: 'text', text: 'x', marks: [{ type: 'link' }] } as never)))).toThrow(UnsupportedContentError);
  });
  test('is idempotent', () => {
    const d = doc(p(t(' a ', 'bold'), br(), t('b\nc', 'subscript')), h(1, t('x\ny')));
    expect(n(n(d))).toEqual(n(d));
  });
});
```

- [ ] **Step 4: Run the tests to see them fail**

Run: `npm test`
Expected: FAIL, with modules not found (`./schema`, `./normalize`, `./test-builders`).

- [ ] **Step 5: Implement `model.ts`, `schema.ts`, `test-builders.ts` and `normalizeDoc(doc: PulpDoc): PulpDoc`**

`normalizeDoc` applies the rules the tests pin, in this order:

1. Types: an unknown node or mark type throws `UnsupportedContentError`.
2. Attributes and text:
   - Unknown attributes are dropped.
   - `level` is clamped to 1–6 and `start` to 0–999 999 999 (integers).
   - Text is NFC, and U+0000 becomes U+FFFD.
3. New lines and line breaks: inside text they become `hardBreak` in paragraphs and a space in headings. A `hardBreak` in a heading also becomes a space.
4. Marks: deduplicated and sorted. Superscript removes subscript. Subscript removes strike.
5. Whitespace and marks, using `/\s/u`:
   - Subscript and superscript are removed from whitespace characters.
   - Every mark is removed from whitespace at the start and end of each of its ranges. A range is a run of adjacent characters with that mark; a `hardBreak` ends it.
6. Strike next to subscript: strike is removed from one character with strike that directly touches a subscript character.
7. Line breaks: `hardBreak` loses its marks, and trailing `hardBreak`s of a paragraph are dropped.
8. Text nodes: empty ones are removed and adjacent equal-mark ones are merged.
9. Empty containers: an empty `doc` gets `[p()]`, and an empty `listItem` gets `[p()]`.
10. Finally, `checkDoc` runs on the result.

The output is in ProseMirror JSON form.

- [ ] **Step 6: Run the tests and the type check**

Run: `npm test && npm run typecheck`
Expected: PASS; `tsc` reports no errors.

- [ ] **Step 7: Commit**

```bash
git add package.json package-lock.json vitest.config.ts .github/workflows/ci.yml src/markdown
git commit -s -m "feat(markdown): document model, schema and normalisation"
```

---

### Task 3: Serializer

**Files:**
- Create: `src/markdown/serialize.ts`, `src/markdown/cases.ts`
- Test: `src/markdown/serialize.test.ts`

**Interfaces:**
- Consumes: `PulpDoc`, `MARK_ORDER`, `normalizeDoc`, test builders (Task 2).
- Produces:
  - `serializeBlocks(doc: PulpDoc): string[]` — one canonical string per top-level block of `normalizeDoc(doc)`, with no trailing new line. It returns `[]` when that document is exactly one empty paragraph.
  - `serializeBody(doc: PulpDoc): string` — `''` when `serializeBlocks` is empty, otherwise `blocks.join('\n\n') + '\n'`.
  - `CANONICAL_CASES: { name: string; doc: PulpDoc; md: string }[]` in `cases.ts`.

The rules are Task 1's definition, §4–§7 and §8 for `rawBlock`, which is written verbatim.

For inline content, walk the children and keep a stack of open marks:
- For each text node, close marks from the top of the stack until every open mark is in its set.
- Then open the missing marks in `MARK_ORDER`.
- Close everything before a `hardBreak` and at the end of the block.

When rendering, apply rule §5(e) to each maximal run of adjacent `*` / `~~` delimiters:
- A run that closes something must be right-flanking.
- A run that opens something must be left-flanking.
- If either test fails, rewrite the literal neighbour on the failing side as `&#N;` (N is the decimal code point).

Use markdown-it's own `utils.isPunctChar`, `utils.isMdAsciiPunct` and `utils.isWhiteSpace` for these tests, so they match the parser exactly.

- [ ] **Step 1: Write `cases.ts` and the failing test**

```ts
// cases.ts (after the SPDX line)
import type { PulpDoc } from './model';
import { doc, p, h, quote, ul, ol, li, t, br, raw } from './test-builders';

export const CANONICAL_CASES: { name: string; doc: PulpDoc; md: string }[] = [
  { name: 'plain paragraph', doc: doc(p(t('Hello, world.'))), md: 'Hello, world.\n' },
  { name: 'empty document', doc: doc(p()), md: '' },
  { name: 'two paragraphs', doc: doc(p(t('One.')), p(t('Two.'))), md: 'One.\n\nTwo.\n' },
  { name: 'empty paragraph kept', doc: doc(p(t('A')), p(), p(t('B'))), md: 'A\n\n&nbsp;\n\nB\n' },
  { name: 'all six styles', doc: doc(h(1, t('Title')), h(2, t('Subtitle')), h(3, t('Heading')), h(4, t('Subheading')), h(5, t('Section')), h(6, t('Subsection')), p(t('Body.'))),
    md: '# Title\n\n## Subtitle\n\n### Heading\n\n#### Subheading\n\n##### Section\n\n###### Subsection\n\nBody.\n' },
  { name: 'quote', doc: doc(quote(p(t('First.')), p(t('Second.')))), md: '> First.\n>\n> Second.\n' },
  { name: 'inline marks', doc: doc(p(t('b', 'bold'), t(' '), t('i', 'italic'), t(' '), t('u', 'underline'), t(' '), t('s', 'strike'), t(' '), t('c', 'smallCaps'), t(' H'), t('2', 'subscript'), t('O x'), t('2', 'superscript'))),
    md: '**b** *i* [u]{.underline} ~~s~~ [c]{.smallcaps} H~2~O x^2^\n' },
  { name: 'bold italic', doc: doc(p(t('both', 'bold', 'italic'))), md: '***both***\n' },
  { name: 'underline small caps', doc: doc(p(t('x', 'underline', 'smallCaps'))), md: '[[x]{.smallcaps}]{.underline}\n' },
  { name: 'escapes', doc: doc(p(t('2 * 3 = 6 & [x] $5 @me #1 a_b ~ ^ | < > { } \\ `'))),
    md: '2 \\* 3 = 6 \\& \\[x\\] \\$5 \\@me \\#1 a\\_b \\~ \\^ \\| \\< \\> \\{ \\} \\\\ \\`\n' },
  { name: 'line-start characters', doc: doc(p(t('- Olá')), p(t('+ plus')), p(t('= equals')), p(t(': colon')), p(t('1984. A year')), p(t('7) seven'))),
    md: '\\- Olá\n\n\\+ plus\n\n\\= equals\n\n\\: colon\n\n1984\\. A year\n\n7\\) seven\n' },
  { name: 'spaces at line edges', doc: doc(p(t('  indented')), p(t('trailing '))), md: '&#32; indented\n\ntrailing&#32;\n' },
  { name: 'hard break', doc: doc(p(t('Line one'), br(), t('- line two'))), md: 'Line one\\\n\\- line two\n' },
  { name: 'bullet list', doc: doc(ul(li(p(t('a'))), li(p(t('b'))))), md: '- a\n- b\n' },
  { name: 'nested list', doc: doc(ul(li(p(t('a')), ul(li(p(t('b'))))))), md: '- a\n  - b\n' },
  { name: 'loose list', doc: doc(ul(li(p(t('a')), p(t('b'))), li(p(t('c'))))), md: '- a\n\n  b\n\n- c\n' },
  { name: 'ordered list', doc: doc(ol(3, li(p(t('x'))), li(p(t('y'))))), md: '3. x\n4. y\n' },
  { name: 'wide numbers', doc: doc(ol(9, li(p(t('i'))), li(p(t('j'), br(), t('k'))))), md: '9. i\n10. j\\\n    k\n' },
  { name: 'adjacent lists', doc: doc(ul(li(p(t('a')))), ul(li(p(t('b')))), ol(1, li(p(t('c')))), ol(1, li(p(t('d'))))),
    md: '- a\n\n* b\n\n1. c\n\n1) d\n' },
  { name: 'nested list not starting at 1', doc: doc(ul(li(p(t('a')), ol(2, li(p(t('b'))))))), md: '- a\n\n  2. b\n' },
  { name: 'quote with break and empty paragraph', doc: doc(quote(p(t('a'), br(), t('b')), p())), md: '> a\\\n> b\n>\n> &nbsp;\n' },
  { name: 'flanking close', doc: doc(p(t('word,', 'bold'), t('next'))), md: '**word,**&#110;ext\n' },
  { name: 'flanking open', doc: doc(p(t('a'), t(',b', 'bold'))), md: '&#97;**,b**\n' },
  { name: 'subscript escapes', doc: doc(p(t('x'), t('a-b', 'subscript'))), md: 'x~a\\-b~\n' },
  { name: 'raw block', doc: doc(p(t('Intro.')), raw('| a | b |\n|---|---|'), p(t('End.'))), md: 'Intro.\n\n| a | b |\n|---|---|\n\nEnd.\n' },
  { name: 'non-Latin text', doc: doc(h(1, t('Capítulo 1 — A Chegada')), p(t('Ελληνικά, 中文, العربية, 😀👍🏽'))),
    md: '# Capítulo 1 — A Chegada\n\nΕλληνικά, 中文, العربية, 😀👍🏽\n' },
];
```

```ts
// serialize.test.ts (after the SPDX line)
import { describe, expect, test } from 'vitest';
import { serializeBlocks, serializeBody } from './serialize';
import { CANONICAL_CASES } from './cases';
import { doc, p, t } from './test-builders';

describe('serializeBody', () => {
  test.each(CANONICAL_CASES)('$name', ({ doc: d, md }) => {
    expect(serializeBody(d)).toBe(md);
  });
  test('normalises before writing', () => {
    expect(serializeBody(doc(p(t('a'), t(' b', 'bold'))))).toBe('a **b**\n');
  });
  test('blocks align with the document', () => {
    expect(serializeBlocks(doc(p(t('a')), p()))).toEqual(['a', '&nbsp;']);
    expect(serializeBlocks(doc(p()))).toEqual([]);
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `npx vitest run src/markdown/serialize.test.ts`
Expected: FAIL, `Cannot find module './serialize'`.

- [ ] **Step 3: Implement `serializeBlocks` and `serializeBody` in `serialize.ts`**

- [ ] **Step 4: Run the tests**

Run: `npm test && npm run typecheck`
Expected: PASS; 26 canonical cases plus 2 tests.

- [ ] **Step 5: Commit**

```bash
git add src/markdown/serialize.ts src/markdown/cases.ts src/markdown/serialize.test.ts
git commit -s -m "feat(markdown): canonical serializer"
```

---

### Task 4: Parser for editable content

**Files:**
- Create: `src/markdown/spans.ts`, `src/markdown/vendor.d.ts`, `src/markdown/parse.ts`
- Test: `src/markdown/parse.test.ts`

**Interfaces:**
- Consumes:
  - `CANONICAL_CASES` (Task 3)
  - `checkDoc` (Task 2)
  - `PulpDoc` and builders (Task 2)
- Produces:
  - `parseBody(markdown: string): PulpDoc`, always a valid normalised document
  - `pulpSpans(md: MarkdownIt): void`

**Input and setup:**
- First replace `\r\n` and `\r` with `\n`, then apply NFC.
- The parser is `new MarkdownIt({ html: true, linkify: false, typographer: false }).use(sub).use(sup).use(pulpSpans)`.

**`pulpSpans` rule:**
- It is an inline rule registered before `'link'`.
- At `[`, find the label end with `state.md.helpers.parseLinkLabel(state, state.pos, false)`.
- It matches only when the label is followed by exactly `{.underline}` or `{.smallcaps}`.
- On a match, it pushes `span_open` with attribute `class`, then tokenizes the label (set `pos` / `posMax`, call `state.md.inline.tokenize(state)`, restore), then pushes `span_close`. It honours `silent`.
- `vendor.d.ts` declares `markdown-it-sub` and `markdown-it-sup` as default-exported `PluginSimple`.

**Converting blocks:**
- Each top-level block (a level-0 opening token with a `map`) is converted to a node.
- These tokens are supported:
  - Blocks: `paragraph`, ATX `heading` (`markup` starting `#`), `blockquote`, `bullet_list`, `ordered_list` (`start` attribute), `list_item`.
  - Inline: `text`, `hardbreak`, `strong`, `em`, `s`, `sub`, `sup`, `span` (class `underline` → `underline`, `smallcaps` → `smallCaps`).
- A paragraph whose inline token's `content` is exactly `&nbsp;` becomes an empty paragraph.
- Any other token, a `softbreak`, or a result that fails `checkDoc` makes that block a `rawBlock`. Its `markdown` is the block's source lines `[map[0], map[1])`, joined with `\n`, with trailing whitespace-only lines removed.
- The final result goes through `normalizeDoc`.

- [ ] **Step 1: Write the failing test** in `src/markdown/parse.test.ts`

```ts
import { describe, expect, test } from 'vitest';
import { parseBody } from './parse';
import { CANONICAL_CASES } from './cases';
import { doc, p, h, t } from './test-builders';

describe('parseBody', () => {
  test.each(CANONICAL_CASES)('$name', ({ doc: d, md }) => {
    expect(parseBody(md)).toEqual(d);
  });
  test('tolerated input', () => {
    expect(parseBody('\n\nA')).toEqual(doc(p(t('A'))));
    expect(parseBody('A\r\n\r\nB\r\n')).toEqual(doc(p(t('A')), p(t('B'))));
    expect(parseBody('   \n')).toEqual(doc(p()));
  });
  test('NFD input', () => {
    expect(parseBody('# Cafe\u0301\n')).toEqual(doc(h(1, t('Café'))));
  });
  test('a real no-break space is text, not an empty paragraph', () => {
    expect(parseBody('\u00a0\n')).toEqual(doc(p(t('\u00a0'))));
  });
  test('Pandoc bracketed spans with other classes are not spans', () => {
    expect(JSON.stringify(parseBody('[x]{.mark}\n'))).not.toMatch(/underline|smallCaps/);
  });
});
```

The last test only pins that no span mark appears. Task 5 turns that block into a raw block.

- [ ] **Step 2: Run to see it fail**

Run: `npx vitest run src/markdown/parse.test.ts`
Expected: FAIL, `Cannot find module './parse'`.

- [ ] **Step 3: Implement `pulpSpans`, `vendor.d.ts` and `parseBody`**

- [ ] **Step 4: Run all tests**

Run: `npm test && npm run typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/markdown/spans.ts src/markdown/vendor.d.ts src/markdown/parse.ts src/markdown/parse.test.ts
git commit -s -m "feat(markdown): parse canonical Pulp Markdown into editor documents"
```

---

### Task 5: Preserving content Pulp doesn't understand

**Files:**
- Modify: `src/markdown/parse.ts`
- Test: `src/markdown/preserve.test.ts`

**Interfaces:**
- Consumes: `serializeBlocks` (Task 3), `parseBody` (Task 4).
- Produces: no new exports. `parseBody` now follows Task 1's definition §8.

This adds three rules to `parseBody`:

1. **Uncovered lines.** Non-blank source lines that are not inside any top-level token's `map` are grouped into runs of consecutive non-blank lines. Each run becomes a `rawBlock` at its position.
2. **Exact-or-raw, repeated until nothing changes.** Build the tentative document and compute `serializeBlocks(tentative)`.
   - If the result is `[]` but the document has a block (the single `&nbsp;` case), that block becomes raw.
   - Otherwise, every non-raw block whose canonical string differs from its source slice (trailing blank lines removed) becomes raw.
   - Repeat while any block changed. A raw block never changes back, so this ends.
3. **Raw blocks never pass through `normalizeDoc`'s text rules.** Their `markdown` is the source slice exactly, which is already NFC and LF from the input step.

- [ ] **Step 1: Write the failing test** in `src/markdown/preserve.test.ts`

```ts
import { describe, expect, test } from 'vitest';
import { parseBody } from './parse';
import { serializeBody } from './serialize';
import { doc, p, t, raw } from './test-builders';

const kept = (md: string) => expect(serializeBody(parseBody(md))).toBe(md);

describe('content Pulp does not understand', () => {
  test.each([
    ['table', '| a | b |\n|---|---|\n| 1 | 2 |\n'],
    ['fenced code with a blank line inside', '```js\nx\n\ny\n```\n'],
    ['link', 'See [the site](https://example.org).\n'],
    ['html', '<div>x</div>\n'],
    ['thematic break', '---\n'],
    ['other italic style', '_italic_\n'],
    ['other bullet style', '* a\n'],
    ['hard-wrapped paragraph', 'line one\nline two\n'],
    ['trailing spaces', 'a  \n'],
    ['setext heading', 'Title\n=====\n'],
    ['Pandoc line block', '| line one\n| line two\n'],
    ['citation', 'As shown [@doe99].\n'],
    ['maths', 'Energy $E=mc^2$.\n'],
    ['other span class', '[x]{.mark}\n'],
    ['heading attributes', '# Title {#intro}\n'],
    ['list inside a quote', '> - a\n'],
    ['heading inside a list', '- # h\n'],
    ['body that is only &nbsp;', '&nbsp;\n'],
  ])('%s is kept as written', (_name, md) => {
    const d = parseBody(md);
    expect(d.content!.every((b) => b.type === 'rawBlock')).toBe(true);
    kept(md);
  });

  test('editable and protected blocks side by side', () => {
    const md = 'Intro.\n\n```\ncode\n```\n\nOutro.\n';
    expect(parseBody(md)).toEqual(doc(p(t('Intro.')), raw('```\ncode\n```'), p(t('Outro.'))));
    kept(md);
  });

  test('lines outside any block are kept: footnotes and references', () => {
    kept('Text.[^1]\n\n[^1]: The note.\n');
    kept('Text [a].\n\n[a]: https://example.org\n');
  });

  test('a block that becomes raw can change its neighbour (repeat until stable)', () => {
    expect(parseBody('- _x_\n\n* b\n')).toEqual(doc(raw('- _x_'), raw('* b')));
    kept('- _x_\n\n* b\n');
  });

  test('only blank lines between blocks change', () => {
    expect(serializeBody(parseBody('\n\nA\n\n\n\n| t |\n|---|\n\n\nB'))).toBe('A\n\n| t |\n|---|\n\nB\n');
  });
});
```

- [ ] **Step 2: Run to see it fail**

Run: `npx vitest run src/markdown/preserve.test.ts`
Expected: FAIL. For example, `_italic_`, the hard-wrapped paragraph and the footnote definition are changed or dropped.

- [ ] **Step 3: Implement the three rules in `parse.ts`**

- [ ] **Step 4: Run all tests**

Run: `npm test && npm run typecheck`
Expected: PASS, including every Task 4 canonical case, which stays editable.

- [ ] **Step 5: Commit**

```bash
git add src/markdown/parse.ts src/markdown/preserve.test.ts
git commit -s -m "feat(markdown): keep content Pulp does not understand exactly as written"
```

---

### Task 6: Round-trip suite, fixtures, performance and public API

**Files:**
- Create: `src/markdown/index.ts`, `src/markdown/arbitrary.ts`
- Create the fixtures in `src/markdown/fixtures/`. Each is a canonical body ending in `\n`:
  - `chapter.md`, a realistic chapter that uses every element in definition §4
  - `scripts.md`: Portuguese, Greek, CJK, Arabic, and emoji with skin tones and joiners
  - `escapes.md`, every rule in §5
  - `lists.md`, every rule in §7
  - `foreign.md`, a body written in another app: tables, code, links, footnotes, maths, citations, HTML, line blocks, fenced divs, `_italic_`, `*` bullets and hard-wrapped lines, separated by single blank lines
- Test: `src/markdown/roundtrip.test.ts`, `src/markdown/perf.test.ts`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `index.ts` re-exports `parseBody`, `serializeBody`, `normalizeDoc`, `UnsupportedContentError`, `pulpSchema`, `checkDoc`, `MARK_ORDER` and the types `PulpDoc`, `PulpNode`, `PulpMark`, `PulpMarkName`. This is the only module Plan 3 imports.
  - `arbitrary.ts` exports `arbDoc: fc.Arbitrary<PulpDoc>` and `arbMarkdown: fc.Arbitrary<string>`.

**`arbDoc`:**
- Builds any valid tree from the Task 2 schema, up to depth 3, then applies `normalizeDoc`.
- Text comes from a weighted alphabet:
  - ASCII letters, digits, space and tab
  - all ASCII punctuation
  - U+00A0, `é`, `e\u0301`, `ß`, `中`, `😀`, `👍🏽`, U+200D, `\n`
  - the strings `&amp;`, `&#32;`, `&nbsp;`
- Ordered list `start` is 0–1000.
- Random mark sets from `MARK_ORDER`.
- `rawBlock` content is drawn from the `foreign.md` blocks, and every `rawBlock` is followed by a paragraph. This keeps out the two merges that definition §8 and §10 accept.
- `foreign.md` uses fenced code only, never indented code, because two indented code blocks in a row merge.

**`arbMarkdown`:**
- Joins 1–12 random lines with `\n`.
- Each line is built from fragments:
  - prefixes: `- `, `* `, `1. `, `3) `, `> `, `# `, ` `, `    `
  - fences: ` ``` `, `~~~`, `---`, `***`, `|`, `: `
  - inline syntax: `_x_`, `**`, `*`, `~~`, `~`, `^`, `[a](b)`, `[a]{.underline}`, `[a]{.mark}`, `$`, `@k`, `\`, `&nbsp;`, `&#32;`, `<b>`
  - text: words from the `arbDoc` alphabet, and empty strings (which make blank lines)

- [ ] **Step 1: Write the failing round-trip tests** in `src/markdown/roundtrip.test.ts`

```ts
import { describe, expect, test } from 'vitest';
import fc from 'fast-check';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { parseBody, serializeBody, normalizeDoc } from './index';
import { arbDoc, arbMarkdown } from './arbitrary';

const dir = join(import.meta.dirname, 'fixtures');
const read = (f: string) => readFileSync(join(dir, f), 'utf8').replace(/\r\n/g, '\n');
const nonBlankLines = (s: string) => s.replace(/\r\n?/g, '\n').normalize('NFC').split('\n').filter((l) => !/^[ \t]*$/.test(l));
const RUNS = { numRuns: 1000 };

describe('fixtures', () => {
  test.each(readdirSync(dir).filter((f) => f.endsWith('.md')))('%s reads and writes back byte for byte', (f) => {
    const md = read(f);
    expect(serializeBody(parseBody(md))).toBe(md);
    const blocks = parseBody(md).content!;
    const raws = blocks.filter((b) => b.type === 'rawBlock').length;
    if (f === 'foreign.md') { expect(raws).toBe(blocks.length); expect(raws).toBeGreaterThanOrEqual(10); }
    else expect(raws).toBe(0);
  });
});

describe('properties', () => {
  test('P1 lossless: editor → file → editor', () => {
    fc.assert(fc.property(arbDoc, (d) => { expect(parseBody(serializeBody(d))).toEqual(normalizeDoc(d)); }), RUNS);
  });
  test('P2 normalising twice changes nothing', () => {
    fc.assert(fc.property(arbDoc, (d) => { expect(normalizeDoc(normalizeDoc(d))).toEqual(normalizeDoc(d)); }), RUNS);
  });
  test('P3 file → editor → file → editor is stable', () => {
    fc.assert(fc.property(arbMarkdown, (s) => { const d = parseBody(s); expect(parseBody(serializeBody(d))).toEqual(d); }), RUNS);
  });
  test('P4 never changes what it does not understand: only blank lines differ', () => {
    fc.assert(fc.property(arbMarkdown, (s) => { expect(nonBlankLines(serializeBody(parseBody(s)))).toEqual(nonBlankLines(s)); }), RUNS);
  });
});
```

- [ ] **Step 2: Write the performance test** in `src/markdown/perf.test.ts`

The test builds a deterministic document of 20,000 words: 2,000 paragraphs of 10 words each. Every 7th word is bold, every 50th paragraph is a level-3 heading, and every 100th paragraph is followed by a 5-item bullet list.

After one warm-up run, it asserts that `serializeBody` takes under 1000 ms and that `parseBody` of the result takes under 1000 ms. It also logs both times with `console.info`, so the report can quote them.

- [ ] **Step 3: Run to see it fail**

Run: `npm test`
Expected: FAIL, with `./index`, `./arbitrary` and the fixtures missing.

- [ ] **Step 4: Write `index.ts`, `arbitrary.ts` and the five fixtures**

Any property failure is a converter bug. Fix it in `serialize.ts`, `parse.ts` or `normalize.ts`, and add the shrunk counterexample as a named case. Use `CANONICAL_CASES` for editable content, or `preserve.test.ts` for raw content.

If the failure shows a combination with no representation at all, rule on a new normalisation. Add it to `normalizeDoc`, its test and definition §9, ledger it, and list it in the report for the owner.

- [ ] **Step 5: Run everything**

Run: `npm test && npm run typecheck`
Expected: PASS. The perf log shows both times under 1000 ms.

- [ ] **Step 6: Commit**

```bash
git add src/markdown
git commit -s -m "test(markdown): round-trip suite, fixtures and performance check"
```

---

## Handoff to Plan 3

- **Opening and saving:** Plan 3 imports only `src/markdown/index.ts`.
  - On open: `editor.commands.setContent(parseBody(itemFile.body))`.
  - On save: `serializeBody(editor.getJSON())`, which is then passed to the core with the header.
- **Matching schema:** TipTap must be configured to produce exactly `pulpSchema`: the same node and mark names, content expressions, attributes and excludes. Plan 3 adds a test that compares TipTap's generated schema with `pulpSchema`.
- **Protected blocks:** `rawBlock` is shown as a protected, non-editable block. Plan 3 decides how it looks.
- **Unsupported content:** `normalizeDoc` (and so `serializeBody`) throws `UnsupportedContentError` if the editor produces a node or mark type this plan doesn't define, and a `RangeError` or `TypeError` from `checkDoc` if the document is structurally invalid. The errors are not wrapped. Plan 3 treats either as a bug and never saves.
