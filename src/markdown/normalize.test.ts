// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { normalizeDoc, UnsupportedContentError } from './normalize';
import { doc, p, h, ul, ol, li, t, br, raw } from './test-builders';

const n = normalizeDoc;

describe('normalizeDoc', () => {
  test('NFC and NUL', () => {
    expect(n(doc(p(t('Café \u0000'))))).toEqual(doc(p(t('Café �'))));
  });
  test('new lines become line breaks in paragraphs and spaces in headings', () => {
    expect(n(doc(p(t('a\r\nb\rc\nd'))))).toEqual(doc(p(t('a'), br(), t('b'), br(), t('c'), br(), t('d'))));
    expect(n(doc(h(2, t('a\nb'))))).toEqual(doc(h(2, t('a b'))));
    expect(n(doc({ type: 'heading', attrs: { level: 2 }, content: [t('a'), br(), t('b')] }))).toEqual(doc(h(2, t('a b'))));
  });
  test('formatting never starts or ends on whitespace', () => {
    expect(n(doc(p(t('x'), t(' bold ', 'bold'), t('y'))))).toEqual(doc(p(t('x '), t('bold', 'bold'), t(' y'))));
  });
  test('subscript and superscript never cover whitespace', () => {
    expect(n(doc(p(t('a b', 'subscript'))))).toEqual(doc(p(t('a', 'subscript'), t(' '), t('b', 'subscript'))));
  });
  test('superscript wins over subscript; strike yields to subscript and its touching character', () => {
    const both = { type: 'text', text: 'x', marks: [{ type: 'subscript' }, { type: 'superscript' }] };
    expect(n(doc(p(both as never)))).toEqual(doc(p(t('x', 'superscript'))));
    expect(n(doc(p(t('ab', 'strike'), t('2', 'subscript'))))).toEqual(doc(p(t('a', 'strike'), t('b'), t('2', 'subscript'))));
    const strikeSub = { type: 'text', text: '2', marks: [{ type: 'strike' }, { type: 'subscript' }] };
    expect(n(doc(p(strikeSub as never)))).toEqual(doc(p(t('2', 'subscript'))));
  });
  test('line breaks carry no marks; trailing line breaks are dropped', () => {
    const markedBr = { type: 'hardBreak', marks: [{ type: 'bold' }] };
    expect(n(doc(p(t('a', 'bold'), markedBr as never, t('b', 'bold'), br(), br())))).toEqual(doc(p(t('a', 'bold'), br(), t('b', 'bold'))));
  });
  test('merges neighbours, drops empty text, sorts and dedupes marks', () => {
    const messy = { type: 'text', text: 'c', marks: [{ type: 'italic' }, { type: 'bold' }, { type: 'italic' }] };
    expect(n(doc(p(t('a'), t(''), t('b'), messy as never)))).toEqual(doc(p(t('ab'), t('c', 'bold', 'italic'))));
  });
  test('attributes: unknown dropped, start clamped, empty document gets a paragraph', () => {
    expect(n(doc(ol(-3, li(p(t('x'))))))).toEqual(doc(ol(0, li(p(t('x'))))));
    expect(n(doc({ type: 'orderedList', attrs: { start: 1, type: null }, content: [li(p(t('x')))] }))).toEqual(doc(ol(1, li(p(t('x'))))));
    expect(n({ type: 'doc' })).toEqual(doc(p()));
    expect(n(doc(ul({ type: 'listItem' })))).toEqual(doc(ul(li(p()))));
  });
  test('unknown node or mark types throw', () => {
    expect(() => n(doc({ type: 'image' }))).toThrow(UnsupportedContentError);
    expect(() => n(doc(p({ type: 'text', text: 'x', marks: [{ type: 'link' }] } as never)))).toThrow(UnsupportedContentError);
  });
  test('text stays NFC after neighbours merge', () => {
    expect(n(doc(p(t('e'), t('́'))))).toEqual(doc(p(t('é'))));
    expect(n(doc(p(t('e', 'bold'), t('́', 'bold'))))).toEqual(doc(p(t('é', 'bold'))));
    const d = doc(p(t('e', 'bold'), t('́')));
    expect(n(n(d))).toEqual(n(d));
  });
  test('lone surrogates become U+FFFD, like NUL', () => {
    expect(n(doc(p(t('a\uD800b'))))).toEqual(doc(p(t('a�b'))));
    expect(n(doc(p(t('a\uDC00'), t('😀'))))).toEqual(doc(p(t('a�😀'))));
  });
  test('raw blocks: empty ones are dropped, blank lines at the edges removed', () => {
    expect(n(doc(p(t('a')), raw(''), p(t('b'))))).toEqual(doc(p(t('a')), p(t('b'))));
    expect(n(doc(raw('\n \n\t\n')))).toEqual(doc(p()));
    expect(n(doc(raw('<div>x</div>\n')))).toEqual(doc(raw('<div>x</div>')));
    expect(n(doc(raw('\n\t\n  indented\n\nmore\n \n\n')))).toEqual(doc(raw('  indented\n\nmore')));
  });
  test('is idempotent', () => {
    const d = doc(p(t(' a ', 'bold'), br(), t('b\nc', 'subscript')), h(1, t('x\ny')));
    expect(n(n(d))).toEqual(n(d));
  });
});
