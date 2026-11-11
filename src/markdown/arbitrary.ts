// SPDX-License-Identifier: MPL-2.0
/// <reference types="node" />
// fast-check generators for the round-trip property tests.
import fc from 'fast-check';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { MARK_ORDER, type PulpDoc, type PulpMark, type PulpNode } from './model';
import { normalizeDoc } from './normalize';

/** Every ASCII punctuation character. */
const ASCII_PUNCT = [...'!"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~'];
const ASCII_ALNUM = [...'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789'];
const SPECIAL = [' ', 'é', 'é', 'ß', '中', '😀', '👍🏽', '‍', '\n'];
const ENTITY_STRINGS = ['&amp;', '&#32;', '&nbsp;'];

/** One piece of text from the weighted alphabet. */
const arbChar: fc.Arbitrary<string> = fc.oneof(
  { weight: 10, arbitrary: fc.constantFrom(...ASCII_ALNUM) },
  { weight: 4, arbitrary: fc.constantFrom(' ', '\t') },
  { weight: 5, arbitrary: fc.constantFrom(...ASCII_PUNCT) },
  { weight: 2, arbitrary: fc.constantFrom(...SPECIAL) },
  { weight: 1, arbitrary: fc.constantFrom(...ENTITY_STRINGS) },
);

const arbText: fc.Arbitrary<string> = fc.array(arbChar, { minLength: 1, maxLength: 8 }).map((cs) => cs.join(''));

const arbMarks: fc.Arbitrary<PulpMark[]> = fc
  .subarray([...MARK_ORDER])
  .map((names) => names.map((type) => ({ type })));

const arbTextNode: fc.Arbitrary<PulpNode> = fc.tuple(arbText, arbMarks).map(([text, marks]) => {
  const node: PulpNode = { type: 'text', text };
  if (marks.length > 0) node.marks = marks;
  return node;
});

const arbInline: fc.Arbitrary<PulpNode[]> = fc.array(
  fc.oneof({ weight: 6, arbitrary: arbTextNode }, { weight: 1, arbitrary: fc.constant<PulpNode>({ type: 'hardBreak' }) }),
  { maxLength: 6 },
);

const withContent = (node: PulpNode, content: PulpNode[]): PulpNode =>
  content.length > 0 ? { ...node, content } : node;

const arbParagraph: fc.Arbitrary<PulpNode> = arbInline.map((c) => withContent({ type: 'paragraph' }, c));

const arbHeading: fc.Arbitrary<PulpNode> = fc
  .tuple(fc.integer({ min: 1, max: 6 }), fc.array(arbTextNode, { maxLength: 4 }))
  .map(([level, c]) => withContent({ type: 'heading', attrs: { level } }, c));

const arbQuote: fc.Arbitrary<PulpNode> = fc
  .array(arbParagraph, { minLength: 1, maxLength: 3 })
  .map((c) => ({ type: 'blockquote', content: c }));

/** A bullet or numbered list whose items nest lists down to `depth` levels. */
function arbList(depth: number): fc.Arbitrary<PulpNode> {
  const rest: fc.Arbitrary<PulpNode> =
    depth > 1 ? fc.oneof({ weight: 2, arbitrary: arbParagraph }, { weight: 1, arbitrary: arbList(depth - 1) }) : arbParagraph;
  const item = fc
    .tuple(arbParagraph, fc.array(rest, { maxLength: depth > 1 ? 2 : 1 }))
    .map(([first, more]): PulpNode => ({ type: 'listItem', content: [first, ...more] }));
  const items = fc.array(item, { minLength: 1, maxLength: 3 });
  return fc.oneof(
    items.map((content): PulpNode => ({ type: 'bulletList', content })),
    fc.tuple(fc.integer({ min: 0, max: 1000 }), items).map(([start, content]): PulpNode => ({
      type: 'orderedList',
      attrs: { start },
      content,
    })),
  );
}

/** The blocks of `fixtures/foreign.md`, which has no blank line inside a block. */
export const FOREIGN_BLOCKS: readonly string[] = readFileSync(
  join(import.meta.dirname, 'fixtures', 'foreign.md'),
  'utf8',
)
  .replace(/\r\n/g, '\n')
  .trimEnd()
  .split(/\n\n+/);

/**
 * A raw block with the paragraph that always follows it, and a paragraph that goes before it
 * only when the block before would be a list (see arbDoc).
 */
const arbRawGroup: fc.Arbitrary<PulpNode[]> = fc
  .tuple(arbParagraph, fc.constantFrom(...FOREIGN_BLOCKS), arbParagraph)
  .map(([before, markdown, after]) => [before, { type: 'rawBlock', attrs: { markdown } }, after]);

const isList = (n: PulpNode | undefined): boolean => n?.type === 'bulletList' || n?.type === 'orderedList';

const arbTopLevel: fc.Arbitrary<PulpNode[]> = fc.oneof(
  { weight: 4, arbitrary: arbParagraph.map((b) => [b]) },
  { weight: 2, arbitrary: arbHeading.map((b) => [b]) },
  { weight: 2, arbitrary: arbQuote.map((b) => [b]) },
  { weight: 3, arbitrary: arbList(3).map((b) => [b]) },
  { weight: 1, arbitrary: arbRawGroup },
);

/** Any valid editor document (lists nest up to depth 3), normalised. */
export const arbDoc: fc.Arbitrary<PulpDoc> = fc.array(arbTopLevel, { maxLength: 6 }).map((groups) => {
  const content: PulpNode[] = [];
  for (const group of groups) {
    // Definition §10: a raw block never directly follows a list and is always followed by a paragraph.
    const [first, ...rest] = group;
    if (rest[0]?.type === 'rawBlock' && !isList(content[content.length - 1])) content.push(...rest);
    else content.push(first, ...rest);
  }
  return normalizeDoc({ type: 'doc', content });
});

const PREFIXES = ['- ', '* ', '1. ', '3) ', '> ', '# ', ' ', '    '];
const FENCES = ['```', '~~~', '---', '***', '|', ': '];
const INLINE = ['_x_', '**', '*', '~~', '~', '^', '[a](b)', '[a]{.underline}', '[a]{.mark}', '$', '@k', '\\', '&nbsp;', '&#32;', '<b>'];

const arbFragment: fc.Arbitrary<string> = fc.oneof(
  { weight: 2, arbitrary: fc.constantFrom(...FENCES) },
  { weight: 4, arbitrary: fc.constantFrom(...INLINE) },
  { weight: 4, arbitrary: arbText },
  { weight: 1, arbitrary: fc.constant(' ') },
);

const arbLine: fc.Arbitrary<string> = fc.oneof(
  { weight: 1, arbitrary: fc.constant('') },
  {
    weight: 5,
    arbitrary: fc
      .tuple(fc.array(fc.constantFrom(...PREFIXES), { maxLength: 2 }), fc.array(arbFragment, { maxLength: 6 }))
      .map(([prefixes, rest]) => prefixes.join('') + rest.join('')),
  },
);

/** Any Markdown-ish text: 1–12 lines of prefixes, fences, inline syntax and words. */
export const arbMarkdown: fc.Arbitrary<string> = fc
  .array(arbLine, { minLength: 1, maxLength: 12 })
  .map((lines) => lines.join('\n'));
