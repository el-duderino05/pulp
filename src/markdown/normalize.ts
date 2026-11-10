// SPDX-License-Identifier: MPL-2.0
import { checkDoc } from './schema';
import { MARK_ORDER, type PulpDoc, type PulpMarkName, type PulpNode } from './model';

export class UnsupportedContentError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'UnsupportedContentError';
  }
}

const NODE_TYPES = new Set([
  'doc', 'paragraph', 'heading', 'blockquote', 'bulletList', 'orderedList',
  'listItem', 'rawBlock', 'hardBreak', 'text',
]);

type Marks = Set<PulpMarkName>;
type CharUnit = { kind: 'char'; ch: string; marks: Marks };
type Unit = CharUnit | { kind: 'break' } | { kind: 'other'; node: PulpNode };

const WS = /\s/u;

function clampInt(v: unknown, min: number, max: number, fallback: number): number {
  if (typeof v !== 'number' || !Number.isFinite(v)) return fallback;
  return Math.min(max, Math.max(min, Math.trunc(v)));
}

function readMarks(node: PulpNode): Marks {
  const out: Marks = new Set();
  for (const m of node.marks ?? []) {
    if (!(MARK_ORDER as readonly string[]).includes(m.type)) {
      throw new UnsupportedContentError(`unsupported mark type: ${String(m.type)}`);
    }
    out.add(m.type);
  }
  return out;
}

function hasMark(u: Unit | undefined, mark: PulpMarkName): u is CharUnit {
  return u !== undefined && u.kind === 'char' && u.marks.has(mark);
}

/** Removes `mark` from whitespace at the start and end of each of its ranges. */
function trimMark(units: Unit[], mark: PulpMarkName): void {
  let i = 0;
  while (i < units.length) {
    if (!hasMark(units[i], mark)) {
      i++;
      continue;
    }
    let end = i;
    while (hasMark(units[end + 1], mark)) end++;
    let s = i;
    let e = end;
    while (s <= e && WS.test((units[s] as CharUnit).ch)) {
      (units[s] as CharUnit).marks.delete(mark);
      s++;
    }
    while (e >= s && WS.test((units[e] as CharUnit).ch)) {
      (units[e] as CharUnit).marks.delete(mark);
      e--;
    }
    i = end + 1;
  }
}

function markList(marks: Marks): { type: PulpMarkName }[] {
  return MARK_ORDER.filter((m) => marks.has(m)).map((type) => ({ type }));
}

function normalizeInline(children: PulpNode[], heading: boolean): PulpNode[] {
  const units: Unit[] = [];
  for (const child of children) {
    if (child.type === 'text') {
      const marks = readMarks(child);
      const text = (child.text ?? '').normalize('NFC').replace(/\u0000/g, '�');
      text.split(/\r\n|\r|\n/).forEach((segment, idx) => {
        if (idx > 0) {
          units.push(heading ? { kind: 'char', ch: ' ', marks: new Set(marks) } : { kind: 'break' });
        }
        for (const ch of segment) units.push({ kind: 'char', ch, marks: new Set(marks) });
      });
    } else if (child.type === 'hardBreak') {
      readMarks(child); // validates mark types; a line break carries none
      units.push(heading ? { kind: 'char', ch: ' ', marks: new Set() } : { kind: 'break' });
    } else {
      units.push({ kind: 'other', node: normalizeNode(child) });
    }
  }

  for (const u of units) {
    if (u.kind !== 'char') continue;
    if (u.marks.has('superscript')) u.marks.delete('subscript');
    if (u.marks.has('subscript')) u.marks.delete('strike');
    if (WS.test(u.ch)) {
      u.marks.delete('subscript');
      u.marks.delete('superscript');
    }
  }
  for (const m of MARK_ORDER) trimMark(units, m);

  // Strike must not touch subscript: drop it from the touching character.
  units.forEach((u, i) => {
    if (!hasMark(u, 'subscript')) return;
    for (const j of [i - 1, i + 1]) {
      const nb = units[j];
      if (nb && nb.kind === 'char') nb.marks.delete('strike');
    }
  });
  trimMark(units, 'strike');

  while (units.length > 0 && units[units.length - 1].kind === 'break') units.pop();

  const out: PulpNode[] = [];
  let curKey: string | undefined;
  let cur: PulpNode | undefined;
  for (const u of units) {
    if (u.kind === 'char') {
      const marks = markList(u.marks);
      const key = marks.map((m) => m.type).join(',');
      if (cur && curKey === key) {
        cur.text += u.ch;
      } else {
        cur = { type: 'text', text: u.ch };
        if (marks.length > 0) cur.marks = marks;
        curKey = key;
        out.push(cur);
      }
    } else {
      cur = undefined;
      out.push(u.kind === 'break' ? { type: 'hardBreak' } : u.node);
    }
  }
  // Merging can bring a base character next to a combining mark: recompose.
  for (const n of out) if (n.type === 'text') n.text = (n.text ?? '').normalize('NFC');
  return out;
}

function normalizeNode(node: PulpNode): PulpNode {
  if (!NODE_TYPES.has(node.type)) {
    throw new UnsupportedContentError(`unsupported node type: ${String(node.type)}`);
  }
  const out: PulpNode = { type: node.type };
  switch (node.type) {
    case 'heading':
      out.attrs = { level: clampInt(node.attrs?.level, 1, 6, 1) };
      break;
    case 'orderedList':
      out.attrs = { start: clampInt(node.attrs?.start, 0, 999_999_999, 1) };
      break;
    case 'rawBlock':
      out.attrs = { markdown: node.attrs?.markdown };
      break;
  }
  if (node.type === 'text' || node.type === 'hardBreak') {
    // Only reachable outside a paragraph or heading; keep the shape, checkDoc rejects it.
    const marks = markList(readMarks(node));
    if (node.type === 'text') out.text = node.text ?? '';
    if (node.type === 'text' && marks.length > 0) out.marks = marks;
    return out;
  }
  const children = node.content ?? [];
  let content: PulpNode[];
  if (node.type === 'paragraph' || node.type === 'heading') {
    content = normalizeInline(children, node.type === 'heading');
  } else {
    content = children.map(normalizeNode);
    if ((node.type === 'doc' || node.type === 'listItem') && content.length === 0) {
      content = [{ type: 'paragraph' }];
    }
  }
  if (content.length > 0) out.content = content;
  return out;
}

/** Applies the editor's normalisation rules. Does not mutate its input. */
export function normalizeDoc(doc: PulpDoc): PulpDoc {
  const result = normalizeNode(doc) as PulpDoc;
  checkDoc(result);
  return result;
}
