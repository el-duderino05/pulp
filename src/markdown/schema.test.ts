// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { checkDoc, pulpSchema } from './schema';
import { MARK_ORDER } from './model';
import { doc, p, h, quote, ul, ol, li, t, br, raw } from './test-builders';

describe('pulpSchema', () => {
  test('mark order', () => {
    expect(MARK_ORDER).toEqual(['bold', 'italic', 'strike', 'underline', 'smallCaps', 'subscript', 'superscript']);
    expect(Object.keys(pulpSchema.marks)).toEqual([...MARK_ORDER]);
  });

  test('accepts every element and keeps its JSON form', () => {
    const d = doc(
      h(1, t('Title', 'bold')), p(t('a'), br(), t('b', 'italic', 'underline')),
      quote(p(t('q')), p()), ul(li(p(t('x')), ol(2, li(p(t('y')))))), raw('| a |'),
    );
    expect(() => checkDoc(d)).not.toThrow();
    expect(pulpSchema.nodeFromJSON(d).toJSON()).toEqual(d);
  });

  test.each([
    ['heading level 7', doc({ type: 'heading', attrs: { level: 7 }, content: [t('x')] })],
    ['list inside a quote', doc({ type: 'blockquote', content: [ul(li(p(t('x'))))] })],
    ['list item starting with a list', doc(ul({ type: 'listItem', content: [ul(li(p(t('x'))))] }))],
    ['line break in a heading', doc({ type: 'heading', attrs: { level: 1 }, content: [t('a'), br()] })],
    ['subscript with superscript', doc(p({ type: 'text', text: 'x', marks: [{ type: 'subscript' }, { type: 'superscript' }] }))],
    ['empty document', { type: 'doc', content: [] }],
  ])('rejects %s', (_name, bad) => {
    expect(() => checkDoc(bad as never)).toThrow();
  });
});
