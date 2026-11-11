// SPDX-License-Identifier: MPL-2.0
import { describe, expect, test } from 'vitest';
import { parseBody } from './parse';
import { serializeBody } from './serialize';
import { doc, p, t, raw } from './test-builders';

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
});
