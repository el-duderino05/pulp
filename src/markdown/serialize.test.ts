// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { serializeBlocks, serializeBody } from './serialize';
import { CANONICAL_CASES } from './cases';
import { doc, p, t } from './test-builders';

describe('serializeBody', () => {
  test.each(CANONICAL_CASES)('$name', ({ doc: d, md }) => {
    expect(serializeBody(d)).toBe(md);
  });
  test('normalises before writing', () => {
    expect(serializeBody(doc(p(t('a'), t(' b', 'bold'))))).toBe('a **b**\n');
  });
  test('blocks align with the document', () => {
    expect(serializeBlocks(doc(p(t('a')), p()))).toEqual(['a', '&nbsp;']);
    expect(serializeBlocks(doc(p()))).toEqual([]);
  });
});

describe('serializeBody, extra', () => {
  test('bold, italic and underline nest in MARK_ORDER', () => {
    expect(serializeBody(doc(p(t('x', 'bold', 'italic', 'underline'))))).toBe('***[x]{.underline}***\n');
  });
  test('an underscore closer before a letter repairs its flanking', () => {
    expect(serializeBody(doc(p(t('a', 'bold'), t('b', 'bold', 'italic'), t('c', 'italic'), t('d'))))).toBe('**a*b***_c_&#100;\n');
  });
});
