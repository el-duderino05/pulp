// SPDX-License-Identifier: MPL-2.0

export type PulpMarkName =
  | 'bold'
  | 'italic'
  | 'strike'
  | 'underline'
  | 'smallCaps'
  | 'subscript'
  | 'superscript';

/** The marks in their canonical order. */
export const MARK_ORDER: readonly PulpMarkName[] = [
  'bold',
  'italic',
  'strike',
  'underline',
  'smallCaps',
  'subscript',
  'superscript',
];

export interface PulpMark {
  type: PulpMarkName;
}

/** ProseMirror JSON, the shape TipTap's getJSON() produces. */
export interface PulpNode {
  type: string;
  attrs?: Record<string, unknown>;
  content?: PulpNode[];
  marks?: PulpMark[];
  text?: string;
}

export type PulpDoc = PulpNode & { type: 'doc' };
