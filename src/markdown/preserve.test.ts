// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { parseBody } from './parse';
import { serializeBody } from './serialize';
import { doc, h, p, t, raw, ul, li } from './test-builders';

const kept = (md: string) => expect(serializeBody(parseBody(md))).toBe(md);

describe('content Pulp does not understand', () => {
  test.each([
    ['table', '| a | b |\n|---|---|\n| 1 | 2 |\n'],
    ['fenced code with a blank line inside', '```js\nx\n\ny\n```\n'],
    ['link', 'See [the site](https://example.org).\n'],
    ['html', '<div>x</div>\n'],
    ['thematic break', '---\n'],
    ['other italic style', '_italic_\n'],
    ['other bullet style', '* a\n'],
    ['hard-wrapped paragraph', 'line one\nline two\n'],
    ['trailing spaces', 'a  \n'],
    ['setext heading', 'Title\n=====\n'],
    ['Pandoc line block', '| line one\n| line two\n'],
    ['citation', 'As shown [@doe99].\n'],
    ['maths', 'Energy $E=mc^2$.\n'],
    ['other span class', '[x]{.mark}\n'],
    ['heading attributes', '# Title {#intro}\n'],
    ['list inside a quote', '> - a\n'],
    ['heading inside a list', '- # h\n'],
    ['body that is only &nbsp;', '&nbsp;\n'],
  ])('%s is kept as written', (_name, md) => {
    const d = parseBody(md);
    expect(d.content!.every((b) => b.type === 'rawBlock')).toBe(true);
    kept(md);
  });

  test('editable and protected blocks side by side', () => {
    const md = 'Intro.\n\n```\ncode\n```\n\nOutro.\n';
    expect(parseBody(md)).toEqual(doc(p(t('Intro.')), raw('```\ncode\n```'), p(t('Outro.'))));
    kept(md);
  });

  test('lines outside any block are kept: footnotes and references', () => {
    kept('Text.[^1]\n\n[^1]: The note.\n');
    kept('Text [a].\n\n[a]: https://example.org\n');
  });

  test('lines outside any block stay together with a block they touch', () => {
    kept('[^1]: The note.\n    continued here.\n');
    expect(parseBody('[^1]: The note.\n    continued here.\n').content!.length).toBe(1);
    expect(parseBody('[a]: https://example.org\nText.\n')).toEqual(doc(raw('[a]: https://example.org\nText.')));
    kept('[a]: https://example.org\nText.\n');
    expect(parseBody('Text.\n[a]: https://example.org\n')).toEqual(doc(raw('Text.\n[a]: https://example.org')));
    kept('Text.\n[a]: https://example.org\n');
  });

  test('a block that becomes raw can change its neighbour (repeat until stable)', () => {
    expect(parseBody('- _x_\n\n* b\n')).toEqual(doc(raw('- _x_'), raw('* b')));
    kept('- _x_\n\n* b\n');
  });

  test('only blank lines between blocks change', () => {
    expect(serializeBody(parseBody('\n\nA\n\n\n\n| t |\n|---|\n\n\nB'))).toBe('A\n\n| t |\n|---|\n\nB\n');
  });

  test.each([
    ['empty item, then two blank lines, then a list', '-\n\n\n- a\n'],
    ['ordered list ending in an empty item', '1. a\n2.\n\n\n3. c\n'],
    ['star list with an empty item', '*\n\n\n* a\n'],
    ['fusing lists between paragraphs', 'x\n\n-\n\n\n- a\n\nz\n'],
  ])('blocks that would run together are kept as one: %s', (_name, md) => {
    kept(md);
    const d = parseBody(md);
    expect(parseBody(serializeBody(d))).toEqual(d);
  });

  test('blocks that would run together: the merged block holds the blank lines', () => {
    expect(parseBody('-\n\n\n- a\n')).toEqual(doc(raw('-\n\n\n- a')));
    expect(parseBody('x\n\n-\n\n\n- a\n\nz\n')).toEqual(doc(p(t('x')), raw('-\n\n\n- a'), p(t('z'))));
  });

  test('blocks that would run together: CRLF random find reads back the same', () => {
    const s = '\tcode\r\n-\r\n\r\r\n-\r\n-\r\n_i_\r\n~~~';
    const d = parseBody(s);
    expect(parseBody(serializeBody(d))).toEqual(d);
  });

  test('a chain of uncovered run, block and uncovered run is one raw block', () => {
    const md = '[a]: /x\n```\nc\n```\n[b]: /y\n\nP\n';
    expect(parseBody(md)).toEqual(doc(raw('[a]: /x\n```\nc\n```\n[b]: /y'), p(t('P'))));
    kept(md);
  });

  test('an uncovered run at the end of the body merges with the block it touches', () => {
    expect(parseBody('A\n\nB\n[a]: https://example.org')).toEqual(
      doc(p(t('A')), raw('B\n[a]: https://example.org')),
    );
    kept('A\n\nB\n[a]: https://example.org\n');
  });

  test('a protected star list right after an editable bullet list keeps its text (§10, R13)', () => {
    // §10: the protected list is exactly how Pulp writes a second list, so it may read back
    // as editable or merge with the list before it. Only its text is promised.
    const md = serializeBody(doc(ul(li(p())), raw('* bowls\n* jars\n* a single lamp'), p()));
    expect(md).toBe('- &nbsp;\n\n* bowls\n* jars\n* a single lamp\n\n&nbsp;\n');
    expect(serializeBody(parseBody(md))).toBe(md);
  });

  test('a line holding only U+00A0 is not blank', () => {
    expect(parseBody('A\n \nB\n')).toEqual(doc(raw('A\n \nB')));
    kept('A\n \nB\n');
  });

  test('CRLF input is read as LF', () => {
    expect(serializeBody(parseBody('A\r\n\r\nB\r\n'))).toBe('A\n\nB\n');
    expect(parseBody('line one\r\nline two\r\n')).toEqual(doc(raw('line one\nline two')));
  });

  test('two touching editable blocks get one blank line between them', () => {
    expect(parseBody('# H\nText\n')).toEqual(doc(h(1, t('H')), p(t('Text'))));
    expect(serializeBody(parseBody('# H\nText\n'))).toBe('# H\n\nText\n');
  });
});
