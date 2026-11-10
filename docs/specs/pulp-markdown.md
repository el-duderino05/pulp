# Pulp Markdown

How Pulp writes the text of a document into a Markdown file, and how it reads it back. This is the prose subset for Milestone 1a.

## 1. Scope

This defines the prose subset for Milestone 1a. Later milestones add the rest of the master design (§4.3):

- links, flags, footnotes, citations, maths, tables, images
- poetry line blocks, checklists, profile fields

Until then, those appear in files only as protected blocks (§8).

## 2. The file

The YAML header is defined in the M1a spec (§3.2) and handled by the core. This document defines the body, which starts on the line after the closing `---`.

## 3. How a body is read

- Reading follows CommonMark 0.31.2 plus these Pandoc extensions:
  - strikethrough `~~x~~`
  - subscript `~x~`
  - superscript `^x^`
  - bracketed spans with exactly the classes `.underline` or `.smallcaps`
- The reference implementation is markdown-it 15 with markdown-it-sub, markdown-it-sup and Pulp's span rule.
- Pulp always writes bodies that CommonMark and Pandoc read the same way. For files Pulp did not write, where the two disagree, Pulp follows CommonMark.

One example of each extension:

```markdown
~~struck~~ H~2~O x^2^ [underlined]{.underline} [small caps]{.smallcaps}
```

## 4. Elements

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

A quote with two paragraphs:

```markdown
> First paragraph.
>
> Second paragraph.
```

Bold, italic and underline on the same words:

```markdown
***[words]{.underline}***
```

## 5. Text and escaping

A literal character is written as follows.

**(a) Always escaped.** These characters are always backslash-escaped: `` \ ` * _ [ ] { } < > ~ ^ $ @ | & # ``.

```markdown
2 \* 3 \& 4 \#1
```

**(b) At the start of a line.** The start of a line means the start of a block's text, or right after a line break.

- `-`, `+`, `=` and `:` are backslash-escaped.
- If the line starts with 1–9 digits followed by `.` or `)`, that `.` or `)` is escaped.

```markdown
\- not a bullet

1\. not a list
```

**(c) Space or tab at the edge of a line.** A space or tab that is the first or last character of a line is written `&#32;` or `&#9;`.

```markdown
&#32;a paragraph that begins with a space
```

**(d) Inside subscript or superscript.** Every ASCII punctuation character is backslash-escaped, and nothing else is.

```markdown
x^a\.b^ and H~2\-3~O
```

**(e) Flanking.** If a run of `*` or `~~` delimiters would not be read as opening or closing because of CommonMark's flanking rules (§6.2), the neighbouring literal character outside the run is written as a decimal character reference (`&#110;` for `n`).

```markdown
**word,**&#110;ext
```

## 6. Paragraph details

- An empty paragraph is written `&nbsp;`. A paragraph whose source is exactly `&nbsp;` reads back as empty, while a real no-break space character stays a character.
- A document that is a single empty paragraph has the body `""`.
- A line break is `\` followed by a new line.
- Headings never contain line breaks.

```markdown
First line\
second line

&nbsp;

A paragraph after an empty one.
```

## 7. Lists

- **Markers:** bullet lists use `-`, ordered lists use `N.`. The second, fourth and further adjacent lists of the same kind use `*` or `N)` instead, so they stay separate lists.
- **Numbering** starts at the list's `start` and counts up.
- **Indentation:** an item's continuation lines and nested blocks are indented by the width of its own marker (`- ` = 2, `10. ` = 4).
- **Loose lists:** a list is loose (blank lines between items and between an item's blocks) if any item has two or more paragraphs, or a nested numbered list that doesn't start at 1. Otherwise it is tight.
- **Item content:** an item starts with a paragraph, followed by paragraphs or lists.

A list that starts at 3 (numbering counts up from there):

```markdown
3. three
4. four
```

An item with a paragraph, a second paragraph, then a nested list. Two paragraphs make the list loose:

```markdown
- First paragraph.

  Second paragraph.

  - nested bullet

- Next item
```

Two adjacent bullet lists, then a tight list with a nested list:

```markdown
- one
- two

* three

- four
  - nested
```

An ordered list that starts at 9. Item 10 has two paragraphs, so the list is loose, and item 10's second paragraph is indented 4:

```markdown
9. nine

10. ten

    More about ten.
```

## 8. Content Pulp doesn't understand

- A top-level block becomes editable only if Pulp would write it back exactly as it is in the file. Otherwise it is kept as a protected raw block, byte for byte.
- Lines that belong to no block become raw blocks too. Example: link reference definitions.
- Blank lines between blocks are written as exactly one.
- Example: a table, a code block, `_italic_` and a hard-wrapped paragraph each stay as written.
- Two protected blocks that end up next to each other (for example, after the paragraph between them is deleted) may be read back as one protected block. Their text is unchanged.

```markdown
| a | b |
|---|---|
| 1 | 2 |

This paragraph is
hard-wrapped, so it stays as written.
```

## 9. What the editor normalises

These are the exact `normalizeDoc` rules, in plain language:

- Text is put in Unicode NFC form.
- New lines inside text become line breaks.
- Formatting never starts or ends on whitespace.
- Subscript and superscript never cover whitespace.
- Superscript wins over subscript.
- Strikethrough is removed where it touches subscript.
- Trailing line breaks are dropped.
- An empty document is one empty paragraph.

For each rule, what the editor has and what is saved (or read back):

| Rule | The editor has | The file holds |
|---|---|---|
| NFC | `e` followed by a combining acute accent | the single character `é` |
| New lines become line breaks | the text `a`, a new line character, `b` | `a\` then a new line, then `b` |
| Formatting never starts or ends on whitespace | bold `bold ` (with a trailing space), then plain `text` | `**bold** text` |
| Subscript and superscript never cover whitespace | `x`, then superscript `2 ` (with a trailing space), then `y` | `x^2^ y` |
| Superscript wins over subscript | `2` marked both subscript and superscript | `^2^` |
| Strikethrough is removed where it touches subscript | `2` marked both strikethrough and subscript | `H~2~O` (no `~~`) |
| Trailing line breaks are dropped | `end` followed by a line break | `end` |
| An empty document is one empty paragraph | nothing at all | the body `""`, read back as one empty paragraph |

## 10. Limits in 1a

- Files written in another style are kept but not editable. This includes `_italic_`, `*` bullets and hard-wrapped lines; editing them arrives with "New project from folder" in 1b.
- Pandoc collapses runs of spaces when publishing; Pulp's own files keep them.
- A list placed directly after a protected list with the same marker is read back as part of that protected block. Its text is kept, but it is no longer editable.

One example for each limit, in the same order:

```markdown
_kept as written, not editable_

* a bullet written with a star

A hard-wrapped paragraph
that stays as it is.

two  spaces here (Pandoc publishes one; Pulp keeps both)

* an old list, protected
* with a star marker

* a new list Pulp writes with a star
```

The last two lists use the same marker and sit next to each other, so they read back as one protected block. All their text is kept.

## 11. Examples

See [`src/markdown/fixtures/`](../../src/markdown/fixtures/). Every fixture there is tested.
