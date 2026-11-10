// SPDX-License-Identifier: MPL-2.0
import { Schema, type MarkSpec, type NodeSpec } from 'prosemirror-model';
import { MARK_ORDER, type PulpDoc, type PulpMarkName, type PulpNode } from './model';

const nodes: Record<string, NodeSpec> = {
  doc: { content: 'block+' },
  paragraph: { group: 'block', content: 'inline*' },
  heading: {
    group: 'block',
    content: 'text*',
    marks: '_',
    attrs: { level: { validate: 'number' } },
  },
  blockquote: { group: 'block', content: 'paragraph+' },
  bulletList: { group: 'block', content: 'listItem+' },
  orderedList: {
    group: 'block',
    content: 'listItem+',
    attrs: { start: { default: 1, validate: 'number' } },
  },
  listItem: { content: 'paragraph (paragraph | bulletList | orderedList)*' },
  rawBlock: {
    group: 'block',
    atom: true,
    attrs: { markdown: { validate: 'string' } },
  },
  hardBreak: { group: 'inline', inline: true },
  text: { group: 'inline' },
};

const markSpecs: Record<PulpMarkName, MarkSpec> = {
  bold: {},
  italic: {},
  strike: { excludes: 'subscript strike' },
  underline: {},
  smallCaps: {},
  subscript: { excludes: 'superscript strike subscript' },
  superscript: { excludes: 'subscript superscript' },
};

const marks: Record<string, MarkSpec> = {};
for (const name of MARK_ORDER) marks[name] = markSpecs[name];

export const pulpSchema: Schema = new Schema({ nodes, marks });

const MAX_START = 999_999_999;

function isIntIn(v: unknown, min: number, max: number): boolean {
  return typeof v === 'number' && Number.isInteger(v) && v >= min && v <= max;
}

function checkNode(node: PulpNode): void {
  switch (node.type) {
    case 'heading':
      if (!isIntIn(node.attrs?.level, 1, 6)) throw new RangeError('heading level must be an integer 1-6');
      break;
    case 'orderedList':
      if (node.attrs?.start !== undefined && !isIntIn(node.attrs.start, 0, MAX_START)) {
        throw new RangeError('list start must be an integer 0-999999999');
      }
      break;
    case 'rawBlock':
      if (typeof node.attrs?.markdown !== 'string') throw new TypeError('rawBlock markdown must be a string');
      break;
    case 'text': {
      const names = (node.marks ?? []).map((m) => m.type);
      for (const a of names) {
        for (const b of names) {
          const markA = pulpSchema.marks[a];
          const markB = pulpSchema.marks[b];
          if (a !== b && markA && markB && markA.excludes(markB)) {
            throw new RangeError(`marks ${a} and ${b} exclude each other`);
          }
        }
      }
      break;
    }
  }
  for (const child of node.content ?? []) checkNode(child);
}

/** Throws if the document is not valid for the Pulp schema. */
export function checkDoc(doc: PulpDoc): void {
  checkNode(doc);
  pulpSchema.nodeFromJSON(doc).check();
}
