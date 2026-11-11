// SPDX-License-Identifier: MPL-2.0
import MarkdownIt from 'markdown-it';
import { MARK_ORDER, type PulpDoc, type PulpMarkName, type PulpNode } from './model';
import { normalizeDoc } from './normalize';

const { isPunctChar, isMdAsciiPunct, isWhiteSpace } = new MarkdownIt().utils;

/**
 * One piece of a rendered line of inline text.
 * - `lit`: a literal character, which rule §5(e) may rewrite as `&#N;`.
 * - `fixed`: text that is never changed (escapes, entities, `[`, `~`, `^`...).
 * - `delim`: a `*`-family or `~~` delimiter, checked against the flanking rules.
 */
type Piece =
  | { kind: 'lit'; ch: string; ent: boolean }
  | { kind: 'fixed'; text: string }
  | { kind: 'delim'; text: string; family: 'star' | 'tilde'; opens: boolean };

const OPEN: Record<PulpMarkName, string> = {
  bold: '**', italic: '*', strike: '~~', underline: '[', smallCaps: '[', subscript: '~', superscript: '^',
};
const CLOSE: Record<PulpMarkName, string> = {
  bold: '**', italic: '*', strike: '~~', underline: ']{.underline}', smallCaps: ']{.smallcaps}',
  subscript: '~', superscript: '^',
};
const FAMILY: Partial<Record<PulpMarkName, 'star' | 'tilde'>> = { bold: 'star', italic: 'star', strike: 'tilde' };

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
  let cur = lines[0].pieces;
  const closeWhile = (keep: (m: PulpMarkName) => boolean): void => {
    while (stack.length > 0 && !stack.every(keep)) cur.push(delimPiece(stack.pop()!, false));
  };
  for (const child of children) {
    if (child.type === 'hardBreak') {
      closeWhile(() => false);
      lines[lines.length - 1].hardBreak = true;
      lines.push({ pieces: [], hardBreak: false });
      cur = lines[lines.length - 1].pieces;
      continue;
    }
    const marks = new Set((child.marks ?? []).map((m) => m.type));
    closeWhile((m) => marks.has(m));
    for (const m of MARK_ORDER) {
      if (marks.has(m) && !stack.includes(m)) {
        stack.push(m);
        cur.push(delimPiece(m, true));
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
  closeWhile(() => false);
  return lines;
}

const isLit = (p: Piece | undefined, test: (ch: string) => boolean): p is Piece & { kind: 'lit' } =>
  p !== undefined && p.kind === 'lit' && !p.ent && test(p.ch);

/** §5(b) and §5(c): escapes at the start of a line and spaces or tabs at its edges. */
function applyLineEdges(pieces: Piece[]): void {
  const first = pieces[0];
  if (isLit(first, (ch) => LINE_START_ESCAPED.has(ch))) {
    pieces[0] = { kind: 'fixed', text: `\\${first.ch}` };
  } else {
    let n = 0;
    while (isLit(pieces[n], (ch) => ch >= '0' && ch <= '9')) n++;
    const after = pieces[n];
    if (n >= 1 && n <= 9 && isLit(after, (ch) => ch === '.' || ch === ')')) {
      pieces[n] = { kind: 'fixed', text: `\\${after.ch}` };
    }
  }
  const isBlank = (ch: string): boolean => ch === ' ' || ch === '\t';
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

/** §5(e): rewrites a literal neighbour as `&#N;` where a run would not open or close. */
function applyFlanking(line: Line): void {
  const ps = line.pieces;
  let changed = true;
  while (changed) {
    changed = false;
    let i = 0;
    while (i < ps.length) {
      const first = ps[i];
      if (first.kind !== 'delim') {
        i++;
        continue;
      }
      let end = i;
      let opens = first.opens;
      let closes = !first.opens;
      while (end + 1 < ps.length) {
        const nx = ps[end + 1];
        if (nx.kind !== 'delim' || nx.family !== first.family) break;
        end++;
        if (nx.opens) opens = true;
        else closes = true;
      }
      const prev = edgeChar(ps[i - 1], 'last');
      const next = end + 1 < ps.length ? edgeChar(ps[end + 1], 'first') : line.hardBreak ? 0x5c : null;
      const left = !ws(next) && (!punct(next) || ws(prev) || punct(prev));
      const right = !ws(prev) && (!punct(prev) || ws(next) || punct(next));
      const rewrite = (j: number): void => {
        const p = ps[j];
        if (p !== undefined && p.kind === 'lit' && !p.ent) {
          p.ent = true;
          changed = true;
        }
      };
      if (closes && !right) rewrite(end + 1);
      if (opens && !left) rewrite(i - 1);
      i = end + 1;
    }
  }
}

/** The lines of a paragraph or heading's text; all but the last end with `\`. */
function inlineLines(node: PulpNode): string[] {
  return inlinePieces(node.content ?? []).map((line) => {
    applyLineEdges(line.pieces);
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

/** Renders sibling blocks; the second, fourth... adjacent list of the same kind alternates its marker. */
function blockSequenceStrings(blocks: PulpNode[]): string[] {
  let run = 0;
  return blocks.map((b, i) => {
    run = i > 0 && blocks[i - 1].type === b.type ? run + 1 : 0;
    return blockLines(b, run % 2 === 1).join('\n');
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
