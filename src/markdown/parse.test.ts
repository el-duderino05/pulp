// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { parseBody } from './parse';
import { CANONICAL_CASES } from './cases';
import { doc, p, h, t } from './test-builders';

describe('parseBody', () => {
  test.each(CANONICAL_CASES)('$name', ({ doc: d, md }) => {
    expect(parseBody(md)).toEqual(d);
  });
  test('tolerated input', () => {
    expect(parseBody('\n\nA')).toEqual(doc(p(t('A'))));
    expect(parseBody('A\r\n\r\nB\r\n')).toEqual(doc(p(t('A')), p(t('B'))));
    expect(parseBody('   \n')).toEqual(doc(p()));
  });
  test('NFD input', () => {
    expect(parseBody('# Café\n')).toEqual(doc(h(1, t('Café'))));
  });
  test('a real no-break space is text, not an empty paragraph', () => {
    expect(parseBody(' \n')).toEqual(doc(p(t(' '))));
  });
  test('Pandoc bracketed spans with other classes are not spans', () => {
    expect(JSON.stringify(parseBody('[x]{.mark}\n'))).not.toMatch(/underline|smallCaps/);
  });
});
