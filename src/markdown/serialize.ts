// SPDX-License-Identifier: MPL-2.0
import MarkdownIt from 'markdown-it';
import { MARK_ORDER, type PulpDoc, type PulpMarkName, type PulpNode } from './model';
import { normalizeDoc } from './normalize';

const { isPunctChar, isMdAsciiPunct, isWhiteSpace } = new MarkdownIt().utils;

/**
 * One piece of a rendered line of inline text.
 * - `lit`: a literal character, which rule §5(e) may rewrite as `&#N;`.
 * - `fixed`: text that is never changed (escapes, entities, `[`, `~`, `^`...).
 * - `delim`: a `*`/`_`-family or `~~` delimiter, checked against the flanking rules.
 *   An opener links to its closer (`partner`) so both can switch from `*` to `_`.
 */
type Family = 'star' | 'under' | 'tilde';
type Delim = { kind: 'delim'; text: string; family: Family; opens: boolean; partner?: Delim; swapped?: boolean };
type Piece = { kind: 'lit'; ch: string; ent: boolean } | { kind: 'fixed'; text: string } | Delim;

const OPEN: Record<PulpMarkName, string> = {
  bold: '**', italic: '*', strike: '~~', underline: '[', smallCaps: '[', subscript: '~', superscript: '^',
};
const CLOSE: Record<PulpMarkName, string> = {
  bold: '**', italic: '*', strike: '~~', underline: ']{.underline}', smallCaps: ']{.smallcaps}',
  subscript: '~', superscript: '^',
};
const FAMILY: Partial<Record<PulpMarkName, Family>> = { bold: 'star', italic: 'star', strike: 'tilde' };

/** §5(a): always backslash-escaped. */
const ALWAYS_ESCAPED = new Set('\\`*_[]{}<>~^$@|&#');
/** §5(b): backslash-escaped at the start of a line. */
const LINE_START_ESCAPED = new Set('-+=:');

const entity = (ch: string): string => `&#${ch.codePointAt(0)};`;

function delimPiece(mark: PulpMarkName, opens: boolean): Piece {
  const text = opens ? OPEN[mark] : CLOSE[mark];
  const family = FAMILY[mark];
  return family ? { kind: 'delim', text, family, opens } : { kind: 'fixed', text };
}

function pieceText(p: Piece): string {
  if (p.kind === 'lit') return p.ent ? entity(p.ch) : p.ch;
  return p.text;
}

/** One line of a paragraph or heading: its pieces, and whether a hard break `\` ends it. */
interface Line {
  pieces: Piece[];
  hardBreak: boolean;
}

/** Walks the inline children with a stack of open marks (MARK_ORDER nesting). */
function inlinePieces(children: PulpNode[]): Line[] {
  const lines: Line[] = [{ pieces: [], hardBreak: false }];
  const stack: PulpMarkName[] = [];
  const openers: Piece[] = [];
  let cur = lines[0].pieces;
  const closeWhile = (cond: () => boolean): void => {
    while (stack.length > 0 && cond()) {
      const closer = delimPiece(stack.pop()!, false);
      const opener = openers.pop()!;
      if (opener.kind === 'delim' && closer.kind === 'delim') opener.partner = closer;
      cur.push(closer);
    }
  };
  for (const child of children) {
    if (child.type === 'hardBreak') {
      closeWhile(() => true);
      lines[lines.length - 1].hardBreak = true;
      lines.push({ pieces: [], hardBreak: false });
      cur = lines[lines.length - 1].pieces;
      continue;
    }
    const marks = new Set((child.marks ?? []).map((m) => m.type));
    // Close until every open mark is kept and nothing to open sorts before the top (§4 nesting).
    const rank = (m: PulpMarkName): number => MARK_ORDER.indexOf(m);
    closeWhile(() => {
      const top = stack[stack.length - 1];
      return !stack.every((m) => marks.has(m)) || [...marks].some((m) => !stack.includes(m) && rank(m) < rank(top));
    });
    for (const m of MARK_ORDER) {
      if (marks.has(m) && !stack.includes(m)) {
        const opener = delimPiece(m, true);
        stack.push(m);
        openers.push(opener);
        cur.push(opener);
      }
    }
    const subSup = marks.has('subscript') || marks.has('superscript');
    for (const ch of child.text ?? '') {
      if (subSup) {
        // §5(d): every ASCII punctuation character is escaped, nothing else.
        cur.push({ kind: 'fixed', text: isMdAsciiPunct(ch.codePointAt(0)!) ? `\\${ch}` : ch });
      } else if (ALWAYS_ESCAPED.has(ch)) {
        cur.push({ kind: 'fixed', text: `\\${ch}` });
      } else {
        cur.push({ kind: 'lit', ch, ent: false });
      }
    }
  }
  closeWhile(() => true);
  return lines;
}

const isLit = (p: Piece | undefined, test: (ch: string) => boolean): p is Piece & { kind: 'lit' } =>
  p !== undefined && p.kind === 'lit' && !p.ent && test(p.ch);

const isBlank = (ch: string): boolean => ch === ' ' || ch === '\t';
const isDigit = (ch: string): boolean => ch >= '0' && ch <= '9';
const isLetter = (ch: string): boolean => /^[A-Za-z]$/.test(ch);
const isRoman = (ch: string): boolean => /^[ivxlcdmIVXLCDM]$/.test(ch);

/** How many literal pieces from `at` make a Pandoc list label: a single ASCII letter or a roman-numeral run. */
function letterLabel(pieces: Piece[], at: number): number {
  let n = 0;
  while (isLit(pieces[at + n], isLetter)) n++;
  if (n === 1) return 1;
  for (let k = 0; k < n; k++) if (!isLit(pieces[at + k], isRoman)) return 0;
  return n;
}

/** Whether the piece after a list label is a space, a tab or the end of the line. */
const endsLabel = (p: Piece | undefined): boolean => p === undefined || isLit(p, isBlank);

/** §5(b) and §5(c): escapes at the start of a line and spaces or tabs at its edges. */
function applyLineEdges(pieces: Piece[]): void {
  const first = pieces[0];
  const escape = (i: number): void => {
    pieces[i] = { kind: 'fixed', text: `\\${pieceText(pieces[i])}` };
  };
  if (isLit(first, (ch) => LINE_START_ESCAPED.has(ch))) {
    escape(0);
  } else {
    let n = 0;
    while (isLit(pieces[n], isDigit)) n++;
    const isClose = (ch: string): boolean => ch === '.' || ch === ')';
    if (n >= 1 && n <= 9 && isLit(pieces[n], isClose)) {
      escape(n);
    } else if (n === 0) {
      // R16: Pandoc's fancy_lists read `a)`, `iv.` and `(1)` / `(a)` / `(iv)` as list markers.
      const letters = letterLabel(pieces, 0);
      if (letters > 0 && isLit(pieces[letters], isClose) && endsLabel(pieces[letters + 1])) {
        escape(letters);
      } else if (isLit(first, (ch) => ch === '(')) {
        let digits = 0;
        while (isLit(pieces[1 + digits], isDigit)) digits++;
        const label = digits >= 1 && digits <= 9 ? digits : digits === 0 ? letterLabel(pieces, 1) : 0;
        if (label > 0 && isLit(pieces[1 + label], (ch) => ch === ')') && endsLabel(pieces[2 + label])) escape(0);
      }
    }
  }
  for (const i of [0, pieces.length - 1]) {
    const p = pieces[i];
    if (isLit(p, isBlank)) pieces[i] = { kind: 'fixed', text: entity(p.ch) };
  }
}

/** The code point the parser sees next to a run; `null` means start or end of line (whitespace). */
function edgeChar(p: Piece | undefined, side: 'first' | 'last'): number | null {
  if (!p) return null;
  const chars = Array.from(pieceText(p));
  return chars[side === 'first' ? 0 : chars.length - 1].codePointAt(0)!;
}

const ws = (c: number | null): boolean => c === null || isWhiteSpace(c);
const punct = (c: number | null): boolean =>
  c !== null && (isMdAsciiPunct(c) || isPunctChar(String.fromCodePoint(c)));

/** Maximal runs of adjacent delimiters of one family: [start, end, delimiters]. */
function delimRuns(ps: Piece[]): [number, number, Delim[]][] {
  const runs: [number, number, Delim[]][] = [];
  let i = 0;
  while (i < ps.length) {
    const first = ps[i];
    if (first.kind !== 'delim') {
      i++;
      continue;
    }
    const run: Delim[] = [first];
    let end = i;
    for (let nx = ps[end + 1]; nx?.kind === 'delim' && nx.family === first.family; nx = ps[end + 1]) {
      run.push(nx);
      end++;
    }
    runs.push([i, end, run]);
    i = end + 1;
  }
  return runs;
}

/** A `*` run that both closes and opens marks writes the opening part, and its closers, with `_`. */
function applyUnderscores(ps: Piece[]): void {
  for (const [, , run] of delimRuns(ps)) {
    if (run[0].family !== 'star' || !run.some((d) => d.opens) || !run.some((d) => !d.opens)) continue;
    for (const d of run) if (d.opens) swapFamily(d);
  }
}

/** Switches an emphasis opener and its closer between `*` and `_`. */
function swapFamily(d: Delim): void {
  for (const x of [d, d.partner!]) {
    x.swapped = true;
    x.family = x.family === 'star' ? 'under' : 'star';
    x.text = x.family === 'star' ? x.text.replace(/_/g, '*') : x.text.replace(/\*/g, '_');
  }
}

/** §5(e): rewrites a literal neighbour as `&#N;` where a run would not open or close. */
function applyFlanking(line: Line): void {
  const ps = line.pieces;
  let changed = true;
  while (changed) {
    changed = false;
    const runs = delimRuns(ps);
    const runLength = new Map<Delim, number>();
    for (const [, , run] of runs) {
      const len = run.reduce((n, d) => n + d.text.length, 0);
      for (const d of run) runLength.set(d, len);
    }
    const open: Delim[] = [];
    for (const [i, end, run] of runs) {
      const prev = edgeChar(ps[i - 1], 'last');
      const next = end + 1 < ps.length ? edgeChar(ps[end + 1], 'first') : line.hardBreak ? 0x5c : null;
      const left = !ws(next) && (!punct(next) || ws(prev) || punct(prev));
      const right = !ws(prev) && (!punct(prev) || ws(next) || punct(next));
      // `_` has CommonMark's stricter intraword rules (§6.2).
      const under = run[0].family === 'under';
      const canOpen = left && (!under || !right || punct(prev));
      const canClose = right && (!under || !left || punct(next));
      const rewrite = (j: number): void => {
        const p = ps[j];
        if (p !== undefined && p.kind === 'lit' && !p.ent) {
          p.ent = true;
          changed = true;
        }
      };
      for (const d of run) if (!d.opens) open.splice(open.findIndex((o) => o.partner === d), 1);
      // CommonMark §6.2: an opening run that can also close would close the other emphasis mark
      // still open with the same character, unless the rule of 3 stops it. Switch its character.
      const family = run[0].family;
      const outer = family === 'tilde' ? undefined : open.find((o) => o.family === family);
      if (outer && canClose && run.every((d) => d.opens && !d.swapped)) {
        const lo = runLength.get(outer)!;
        const lr = runLength.get(run[0])!;
        const ruleOf3 = (lo + lr) % 3 === 0 && !(lo % 3 === 0 && lr % 3 === 0);
        const other: Family = family === 'star' ? 'under' : 'star';
        const touches = [ps[i - 1], ps[end + 1]].some((p) => p?.kind === 'delim' && p.family === other);
        if (!ruleOf3 && !touches) {
          for (const d of run) swapFamily(d);
          changed = true;
          break;
        }
      }
      open.push(...run.filter((d) => d.opens));
      // A run that fails on whitespace inside it rewrites that whitespace; otherwise the character outside.
      if (run.some((d) => !d.opens) && !canClose) rewrite(ws(prev) ? i - 1 : end + 1);
      if (run.some((d) => d.opens) && !canOpen) rewrite(ws(next) ? end + 1 : i - 1);
    }
  }
}

/** The lines of a paragraph or heading's text; all but the last end with `\`. */
function inlineLines(node: PulpNode): string[] {
  return inlinePieces(node.content ?? []).map((line) => {
    applyLineEdges(line.pieces);
    applyUnderscores(line.pieces);
    applyFlanking(line);
    return line.pieces.map(pieceText).join('') + (line.hardBreak ? '\\' : '');
  });
}

function paragraphLines(node: PulpNode): string[] {
  return (node.content ?? []).length === 0 ? ['&nbsp;'] : inlineLines(node);
}

function isLoose(list: PulpNode): boolean {
  return (list.content ?? []).some((item) => {
    const blocks = item.content ?? [];
    return (
      blocks.filter((b) => b.type === 'paragraph').length >= 2 ||
      blocks.some((b) => b.type === 'orderedList' && b.attrs?.start !== 1)
    );
  });
}

function listLines(list: PulpNode, alternate: boolean): string[] {
  const loose = isLoose(list);
  const ordered = list.type === 'orderedList';
  const start = ordered ? (list.attrs?.start as number) : 0;
  const out: string[] = [];
  (list.content ?? []).forEach((item, idx) => {
    if (idx > 0 && loose) out.push('');
    const marker = ordered ? `${start + idx}${alternate ? ')' : '.'} ` : alternate ? '* ' : '- ';
    const indent = ' '.repeat(marker.length);
    blockSequenceLines(item.content ?? [], loose ? '\n\n' : '\n').forEach((line, j) => {
      out.push(j === 0 ? marker + line : line === '' ? '' : indent + line);
    });
  });
  return out;
}

function blockLines(node: PulpNode, alternate: boolean): string[] {
  switch (node.type) {
    case 'paragraph':
      return paragraphLines(node);
    case 'heading': {
      const hashes = '#'.repeat(node.attrs?.level as number);
      const text = inlineLines(node).join('');
      return [text === '' ? hashes : `${hashes} ${text}`];
    }
    case 'blockquote': {
      const out: string[] = [];
      (node.content ?? []).forEach((para, idx) => {
        if (idx > 0) out.push('>');
        for (const line of paragraphLines(para)) out.push(`> ${line}`);
      });
      return out;
    }
    case 'bulletList':
    case 'orderedList':
      return listLines(node, alternate);
    case 'rawBlock':
      return [String(node.attrs?.markdown ?? '')];
    default:
      throw new Error(`cannot serialize node type: ${node.type}`);
  }
}

/**
 * The list-alternation counter: `run` for the block before `b`, plus one if `b` has the same
 * type, otherwise 0. A block's canonical text depends only on the blocks before it.
 */
export function nextRun(prev: PulpNode | undefined, run: number, b: PulpNode): number {
  return prev !== undefined && prev.type === b.type ? run + 1 : 0;
}

/**
 * The canonical text of one normalised top-level block whose counter is `run` (0 for the
 * first block of a run): the second, fourth... list of a run alternates its marker.
 */
export function writeBlock(node: PulpNode, run: number): string {
  return blockLines(node, run % 2 === 1).join('\n');
}

/** Renders sibling blocks; every even-numbered adjacent list of the same kind alternates its marker. */
function blockSequenceStrings(blocks: PulpNode[]): string[] {
  let run = 0;
  return blocks.map((b, i) => {
    run = nextRun(blocks[i - 1], run, b);
    return writeBlock(b, run);
  });
}

function blockSequenceLines(blocks: PulpNode[], separator: string): string[] {
  return blockSequenceStrings(blocks).join(separator).split('\n');
}

/** One canonical string per top-level block, without a trailing new line. */
export function serializeBlocks(doc: PulpDoc): string[] {
  const blocks = normalizeDoc(doc).content ?? [];
  if (blocks.length === 1 && blocks[0].type === 'paragraph' && (blocks[0].content ?? []).length === 0) {
    return [];
  }
  return blockSequenceStrings(blocks);
}

/** The Markdown body: blocks separated by one blank line, ending with one new line. */
export function serializeBody(doc: PulpDoc): string {
  const blocks = serializeBlocks(doc);
  return blocks.length === 0 ? '' : `${blocks.join('\n\n')}\n`;
}
