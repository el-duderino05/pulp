// SPDX-License-Identifier: MPL-2.0
import { MARK_ORDER, type PulpDoc, type PulpMarkName, type PulpNode } from './model';

function node(type: string, attrs: Record<string, unknown> | undefined, content: PulpNode[]): PulpNode {
  const n: PulpNode = { type };
  if (attrs) n.attrs = attrs;
  if (content.length > 0) n.content = content;
  return n;
}

export function doc(...blocks: PulpNode[]): PulpDoc {
  return { ...node('doc', undefined, blocks), type: 'doc' };
}
export const p = (...inline: PulpNode[]): PulpNode => node('paragraph', undefined, inline);
export const h = (level: number, ...inline: PulpNode[]): PulpNode => node('heading', { level }, inline);
export const quote = (...paragraphs: PulpNode[]): PulpNode => node('blockquote', undefined, paragraphs);
export const ul = (...items: PulpNode[]): PulpNode => node('bulletList', undefined, items);
export const ol = (start: number, ...items: PulpNode[]): PulpNode => node('orderedList', { start }, items);
export const li = (...blocks: PulpNode[]): PulpNode => node('listItem', undefined, blocks);
export const br = (): PulpNode => ({ type: 'hardBreak' });
export const raw = (markdown: string): PulpNode => ({ type: 'rawBlock', attrs: { markdown } });

export function t(text: string, ...marks: PulpMarkName[]): PulpNode {
  const n: PulpNode = { type: 'text', text };
  const sorted = MARK_ORDER.filter((m) => marks.includes(m));
  if (sorted.length > 0) n.marks = sorted.map((type) => ({ type }));
  return n;
}
