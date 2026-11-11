// SPDX-License-Identifier: MPL-2.0
/// <reference types="node" />
// fast-check generators for the round-trip property tests.
import fc from 'fast-check';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { MARK_ORDER, type PulpDoc, type PulpMark, type PulpNode } from './model';
import { normalizeDoc } from './normalize';
import { convertTopBlock, readTopBlocks, runTogether } from './parse';
import { nextRun, writeBlock } from './serialize';

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

const arbRaw: fc.Arbitrary<PulpNode> = fc
  .constantFrom(...FOREIGN_BLOCKS)
  .map((markdown): PulpNode => ({ type: 'rawBlock', attrs: { markdown } }));

const arbTopLevel: fc.Arbitrary<PulpNode> = fc.oneof(
  { weight: 4, arbitrary: arbParagraph },
  { weight: 2, arbitrary: arbHeading },
  { weight: 2, arbitrary: arbQuote },
  { weight: 3, arbitrary: arbList(3) },
  { weight: 2, arbitrary: arbRaw },
);

const isList = (n: PulpNode | undefined): boolean => n?.type === 'bulletList' || n?.type === 'orderedList';
const isRaw = (n: PulpNode | undefined): boolean => n?.type === 'rawBlock';

/**
 * Whether the raw block at `i` sits where a ruled limit says it does not read back as itself.
 * `written` and `runs` are each block's canonical text and list-alternation counter.
 */
function inRuledLimit(blocks: PulpNode[], written: string[], runs: number[], i: number): boolean {
  const md = written[i];
  const prev = blocks[i - 1];
  const next = blocks[i + 1];
  // R15 (§10): a protected block that markdown-it reads as part of the editable list before it.
  if (isList(prev) && runTogether(written[i - 1], md)) return true;
  // §10: an editable list read as part of the protected list before it (same marker).
  if (isList(next) && runTogether(md, written[i + 1])) return true;
  // §8: two protected blocks next to each other may read back as one.
  if (isRaw(prev) && runTogether(written[i - 1], md)) return true;
  if (isRaw(next) && runTogether(md, written[i + 1])) return true;
  // R13 (§10): a protected list after an editable list of its kind, written exactly as Pulp
  // writes a list there, reads back as editable.
  if (isList(prev)) {
    const read = readTopBlocks(md);
    const node = read.length === 1 ? convertTopBlock(read[0]) : undefined;
    if (node?.type === prev!.type) {
      const normal = normalizeDoc({ type: 'doc', content: [node] }).content![0];
      if (writeBlock(normal, nextRun(prev, runs[i - 1], normal)) === md) return true;
    }
  }
  return false;
}

/**
 * Any valid editor document (lists nest up to depth 3), normalised. A raw block is left out
 * only where a ruled limit (§8, §10 / R13, R15) says it does not read back as itself.
 */
export const arbDoc: fc.Arbitrary<PulpDoc> = fc.array(arbTopLevel, { maxLength: 6 }).map((generated) => {
  let blocks = normalizeDoc({ type: 'doc', content: generated }).content!;
  for (;;) {
    const written: string[] = [];
    const runs: number[] = [];
    blocks.forEach((b, i) => {
      runs.push(nextRun(blocks[i - 1], runs[i - 1] ?? 0, b));
      written.push(writeBlock(b, runs[i]));
    });
    const bad = blocks.findIndex((b, i) => isRaw(b) && inRuledLimit(blocks, written, runs, i));
    if (bad < 0) break;
    blocks = blocks.filter((_, i) => i !== bad);
  }
  return normalizeDoc({ type: 'doc', content: blocks });
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
