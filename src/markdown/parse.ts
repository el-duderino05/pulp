// SPDX-License-Identifier: MPL-2.0
import MarkdownIt, { type Token } from 'markdown-it';
import sub from 'markdown-it-sub';
import sup from 'markdown-it-sup';
import { pulpSpans } from './spans';
import { checkDoc } from './schema';
import { normalizeDoc } from './normalize';
import { serializeBlocks } from './serialize';
import type { PulpDoc, PulpMarkName, PulpNode } from './model';

const md = new MarkdownIt({ html: true, linkify: false, typographer: false })
  .use(sub)
  .use(sup)
  .use(pulpSpans);

/** Thrown while converting a block that holds something Pulp cannot edit. */
class Unsupported extends Error {}

/** Line endings to `\n`, then Unicode NFC. */
export function prepareSource(markdown: string): string {
  return markdown.replace(/\r\n?/g, '\n').normalize('NFC');
}

/** A top-level block: its tokens and its source lines `[map[0], map[1])`. */
export interface TopBlock {
  tokens: Token[];
  map: [number, number];
}

/** Splits the token stream into top-level blocks (level-0 tokens with a `map`). */
export function topLevelBlocks(tokens: Token[]): TopBlock[] {
  const blocks: TopBlock[] = [];
  let i = 0;
  while (i < tokens.length) {
    const tok = tokens[i];
    if (tok.level !== 0 || !tok.map || tok.nesting < 0) {
      i++;
      continue;
    }
    let end = i + 1;
    if (tok.nesting > 0) {
      let depth = 1;
      while (end < tokens.length && depth > 0) {
        depth += tokens[end].nesting;
        end++;
      }
    }
    blocks.push({ tokens: tokens.slice(i, end), map: [tok.map[0], tok.map[1]] });
    i = end;
  }
  return blocks;
}

/** A line is blank iff it holds only spaces and tabs. */
const isBlank = (line: string): boolean => /^[ \t]*$/.test(line);

/** The block's source lines, with trailing blank lines removed. */
export function sourceSlice(lines: readonly string[], map: [number, number]): string {
  const slice = lines.slice(map[0], map[1]);
  while (slice.length > 0 && isBlank(slice[slice.length - 1])) slice.pop();
  return slice.join('\n');
}

const MARK_TOKENS = new Map<string, PulpMarkName>([
  ['strong', 'bold'],
  ['em', 'italic'],
  ['s', 'strike'],
  ['sub', 'subscript'],
  ['sup', 'superscript'],
]);

const SPAN_CLASSES = new Map<string, PulpMarkName>([
  ['underline', 'underline'],
  ['smallcaps', 'smallCaps'],
]);

/** The mark of an opening token. Mapped by token type, never by markup (`*` and `_` alike). */
function markOf(tok: Token, base: string): PulpMarkName {
  const mark = base === 'span' ? SPAN_CLASSES.get(String(tok.attrGet('class') ?? '')) : MARK_TOKENS.get(base);
  if (!mark) throw new Unsupported(tok.type);
  return mark;
}

/** Converts an `inline` token's children. Marks are a set; `normalizeDoc` fixes their order. */
function convertInline(children: readonly Token[]): PulpNode[] {
  const open = new Map<PulpMarkName, number>();
  const stack: { base: string; mark: PulpMarkName }[] = [];
  const out: PulpNode[] = [];
  for (const tok of children) {
    switch (tok.type) {
      case 'text':
      case 'text_special': {
        if (tok.content === '') break;
        const node: PulpNode = { type: 'text', text: tok.content };
        const marks = [...open.keys()].map((type) => ({ type }));
        if (marks.length > 0) node.marks = marks;
        out.push(node);
        break;
      }
      case 'hardbreak':
        out.push({ type: 'hardBreak' });
        break;
      default:
        if (tok.nesting === 1) {
          const base = tok.type.replace(/_open$/, '');
          const mark = markOf(tok, base);
          stack.push({ base, mark });
          open.set(mark, (open.get(mark) ?? 0) + 1);
        } else if (tok.nesting === -1) {
          // A close token carries no class: it closes the latest open token of its type.
          const base = tok.type.replace(/_close$/, '');
          let at = stack.length - 1;
          while (at >= 0 && stack[at].base !== base) at--;
          if (at < 0) throw new Unsupported(tok.type);
          const mark = stack.splice(at, 1)[0].mark;
          const n = (open.get(mark) ?? 0) - 1;
          if (n > 0) open.set(mark, n);
          else open.delete(mark);
        } else {
          // softbreak, code_inline, html_inline, image and anything else.
          throw new Unsupported(tok.type);
        }
    }
  }
  return out;
}

/** A cursor over one block's tokens. */
class Cursor {
  i = 0;
  constructor(private readonly tokens: readonly Token[]) {}
  peek(): Token | undefined {
    return this.tokens[this.i];
  }
  next(): Token {
    const tok = this.tokens[this.i++];
    if (!tok) throw new Unsupported('unexpected end of block');
    return tok;
  }
  expect(type: string): Token {
    const tok = this.next();
    if (tok.type !== type) throw new Unsupported(`expected ${type}, got ${tok.type}`);
    return tok;
  }
}

function withContent(node: PulpNode, content: PulpNode[]): PulpNode {
  if (content.length > 0) node.content = content;
  return node;
}

function inlineContent(cur: Cursor): { children: PulpNode[]; source: string } {
  const tok = cur.expect('inline');
  return { children: convertInline(tok.children ?? []), source: tok.content };
}

/** Converts blocks up to and including the close token `closeType`. */
function convertBlocksUntil(cur: Cursor, closeType: string): PulpNode[] {
  const out: PulpNode[] = [];
  for (;;) {
    const tok = cur.peek();
    if (!tok) throw new Unsupported(`missing ${closeType}`);
    if (tok.type === closeType) {
      cur.next();
      return out;
    }
    out.push(convertBlock(cur));
  }
}

function convertBlock(cur: Cursor): PulpNode {
  const tok = cur.next();
  switch (tok.type) {
    case 'paragraph_open': {
      const { children, source } = inlineContent(cur);
      cur.expect('paragraph_close');
      if (source === '&nbsp;') return { type: 'paragraph' };
      return withContent({ type: 'paragraph' }, children);
    }
    case 'heading_open': {
      if (!tok.markup.startsWith('#')) throw new Unsupported('setext heading');
      const level = Number(tok.tag.slice(1));
      const { children } = inlineContent(cur);
      cur.expect('heading_close');
      return withContent({ type: 'heading', attrs: { level } }, children);
    }
    case 'blockquote_open':
      return withContent({ type: 'blockquote' }, convertBlocksUntil(cur, 'blockquote_close'));
    case 'bullet_list_open':
      return withContent({ type: 'bulletList' }, convertBlocksUntil(cur, 'bullet_list_close'));
    case 'ordered_list_open': {
      const start = Number(tok.attrGet('start') ?? 1);
      return withContent({ type: 'orderedList', attrs: { start } }, convertBlocksUntil(cur, 'ordered_list_close'));
    }
    case 'list_item_open':
      return withContent({ type: 'listItem' }, convertBlocksUntil(cur, 'list_item_close'));
    default:
      throw new Unsupported(tok.type);
  }
}

/**
 * Converts one top-level block to an editor node. Returns `undefined` when the block
 * holds anything Pulp cannot edit or the result is not valid for the schema.
 */
export function convertTopBlock(block: TopBlock): PulpNode | undefined {
  try {
    const cur = new Cursor(block.tokens);
    const node = convertBlock(cur);
    if (cur.peek()) return undefined;
    checkDoc({ type: 'doc', content: [node] });
    return node;
  } catch (e) {
    if (e instanceof Unsupported || e instanceof RangeError || e instanceof TypeError) return undefined;
    throw e;
  }
}

const rawBlock = (markdown: string): PulpNode => ({ type: 'rawBlock', attrs: { markdown } });

/** One top-level piece of the body: where it starts, its source text and its tentative node. */
interface Piece {
  /** Source lines `[start, end)`; the last one is non-blank. */
  start: number;
  end: number;
  slice: string;
  node: PulpNode;
  /** Whether the piece starts / ends with lines that no top-level token covers. */
  startsUncovered: boolean;
  endsUncovered: boolean;
}

/**
 * The body's top-level pieces in source order: each top-level token, plus each run of
 * consecutive non-blank lines that no top-level token covers (for example link reference
 * definitions), which is raw. A run with no blank line between it and a neighbouring piece
 * is merged with it into one raw piece, so no blank line is ever added inside text that
 * was contiguous in the file.
 */
function pieces(src: string, lines: readonly string[]): Piece[] {
  const found: Piece[] = [];
  const covered = new Array<boolean>(lines.length).fill(false);
  for (const block of topLevelBlocks(md.parse(src, {}))) {
    for (let l = block.map[0]; l < block.map[1]; l++) covered[l] = true;
    const slice = sourceSlice(lines, block.map);
    if (slice === '') continue;
    found.push({
      start: block.map[0],
      end: block.map[0] + slice.split('\n').length,
      slice,
      node: convertTopBlock(block) ?? rawBlock(slice),
      startsUncovered: false,
      endsUncovered: false,
    });
  }
  let l = 0;
  while (l < lines.length) {
    if (covered[l] || isBlank(lines[l])) {
      l++;
      continue;
    }
    const start = l;
    while (l < lines.length && !covered[l] && !isBlank(lines[l])) l++;
    const slice = lines.slice(start, l).join('\n');
    found.push({ start, end: l, slice, node: rawBlock(slice), startsUncovered: true, endsUncovered: true });
  }
  found.sort((a, b) => a.start - b.start);
  const out: Piece[] = [];
  for (const piece of found) {
    const prev = out[out.length - 1];
    if (prev && prev.end === piece.start && (prev.endsUncovered || piece.startsUncovered)) {
      const slice = lines.slice(prev.start, piece.end).join('\n');
      out[out.length - 1] = { ...prev, end: piece.end, slice, node: rawBlock(slice), endsUncovered: piece.endsUncovered };
    } else {
      out.push(piece);
    }
  }
  return out;
}

/**
 * Parses a Pulp Markdown body into a normalised editor document. A top-level block is
 * editable only if writing the document back reproduces its source text exactly; otherwise
 * it is a raw block holding that text, byte for byte. Making one block raw can change how a
 * neighbour is written (adjacent lists alternate markers), so this repeats until nothing
 * changes. A raw block never turns back, so it ends.
 */
export function parseBody(markdown: string): PulpDoc {
  const src = prepareSource(markdown);
  const lines = src.split('\n');
  const items = pieces(src, lines);
  exactOrRaw(items);
  // Every piece is now written exactly as its slice, so whether two neighbours run together
  // depends only on their slices. Merging never changes another piece's slice, so one pass
  // (re-checking the pairs around each merge) finds every fusing pair.
  if (fuseNeighbours(items, lines)) exactOrRaw(items);
  // Raw blocks keep their source slice: normalizeDoc copies a raw block's text unchanged.
  return normalizeDoc({ type: 'doc', content: items.map((it) => it.node) });
}

/** Makes raw every editable piece not written back exactly as its slice, until nothing changes. */
function exactOrRaw(items: Piece[]): void {
  for (let changed = true; changed; ) {
    changed = false;
    const written = serializeBlocks({ type: 'doc', content: items.map((it) => it.node) });
    items.forEach((it, idx) => {
      // `written` is empty when the document reads as one empty paragraph (an `&nbsp;` body).
      if (it.node.type !== 'rawBlock' && (written.length === 0 || written[idx] !== it.slice)) {
        it.node = rawBlock(it.slice);
        changed = true;
      }
    });
  }
}

/** Whether `a` and `b`, written with one blank line between, are read as one top-level block. */
function runTogether(a: string, b: string): boolean {
  const bStart = a.split('\n').length + 1;
  return topLevelBlocks(md.parse(`${a}\n\n${b}`, {})).some((blk) => blk.map[0] < bStart && blk.map[1] > bStart);
}

/**
 * Merges neighbouring pieces that would run together once the blank lines between them are
 * tidied into one raw piece holding the exact source lines, blank lines included.
 * Returns whether anything merged.
 */
function fuseNeighbours(items: Piece[], lines: readonly string[]): boolean {
  let merged = false;
  let i = 0;
  while (i + 1 < items.length) {
    const a = items[i];
    const b = items[i + 1];
    if (!runTogether(a.slice, b.slice)) {
      i++;
      continue;
    }
    const slice = lines.slice(a.start, b.end).join('\n');
    items.splice(i, 2, { ...a, end: b.end, slice, node: rawBlock(slice), endsUncovered: b.endsUncovered });
    merged = true;
    // The merged piece may now run together with its left neighbour too.
    if (i > 0) i--;
  }
  return merged;
}
