// SPDX-License-Identifier: MPL-2.0
/// <reference types="node" />
import { describe, expect, test } from 'vitest';
import fc from 'fast-check';
import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { parseBody, serializeBody, normalizeDoc } from './index';
import { arbDoc, arbMarkdown } from './arbitrary';

const dir = join(import.meta.dirname, 'fixtures');
const read = (f: string) => readFileSync(join(dir, f), 'utf8').replace(/\r\n/g, '\n');
const nonBlankLines = (s: string) => s.replace(/\r\n?/g, '\n').normalize('NFC').split('\n').filter((l) => !/^[ \t]*$/.test(l));
const RUNS = { numRuns: 1000 };
/** Property tests can be slow when the suite runs in parallel. */
const SLOW = { timeout: 120_000 };

describe('fixtures', () => {
  test('the five fixtures are all there', () => {
    expect(readdirSync(dir).filter((f) => f.endsWith('.md')).sort()).toEqual(
      ['chapter.md', 'escapes.md', 'foreign.md', 'lists.md', 'scripts.md'],
    );
  });
  test.each(readdirSync(dir).filter((f) => f.endsWith('.md')))('%s reads and writes back byte for byte', (f) => {
    const md = read(f);
    expect(serializeBody(parseBody(md))).toBe(md);
    const blocks = parseBody(md).content!;
    const raws = blocks.filter((b) => b.type === 'rawBlock').length;
    if (f === 'foreign.md') { expect(raws).toBe(blocks.length); expect(raws).toBeGreaterThanOrEqual(10); }
    else expect(raws).toBe(0);
  });
});

describe('properties', () => {
  test('P1 lossless: editor → file → editor', SLOW, () => {
    fc.assert(fc.property(arbDoc, (d) => { expect(parseBody(serializeBody(d))).toEqual(normalizeDoc(d)); }), RUNS);
  });
  test('P2 normalising twice changes nothing', SLOW, () => {
    fc.assert(fc.property(arbDoc, (d) => { expect(normalizeDoc(normalizeDoc(d))).toEqual(normalizeDoc(d)); }), RUNS);
  });
  test('P3 file → editor → file → editor is stable', SLOW, () => {
    fc.assert(fc.property(arbMarkdown, (s) => { const d = parseBody(s); expect(parseBody(serializeBody(d))).toEqual(d); }), RUNS);
  });
  test('P4 never changes what it does not understand: only blank lines differ', SLOW, () => {
    fc.assert(fc.property(arbMarkdown, (s) => { expect(nonBlankLines(serializeBody(parseBody(s)))).toEqual(nonBlankLines(s)); }), RUNS);
  });
});
