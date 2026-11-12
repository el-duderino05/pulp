// SPDX-License-Identifier: MPL-2.0
import { expect, test } from 'vitest';
import { parseBody, serializeBody, type PulpDoc, type PulpNode } from './index';
import { doc, h, li, p, t, ul } from './test-builders';

/** 20,000 words: 2,000 paragraphs of 10 words, every 7th word bold, every 50th paragraph
 *  a level-3 heading, every 100th paragraph followed by a 5-item bullet list. */
function bigDocument(): PulpDoc {
  const blocks: PulpNode[] = [];
  let word = 0;
  for (let para = 1; para <= 2000; para++) {
    const inline: PulpNode[] = [];
    for (let w = 0; w < 10; w++) {
      word++;
      if (w > 0) inline.push(t(' '));
      const text = `word${word}`;
      inline.push(word % 7 === 0 ? t(text, 'bold') : t(text));
    }
    blocks.push(para % 50 === 0 ? h(3, ...inline) : p(...inline));
    if (para % 100 === 0) {
      blocks.push(ul(...[1, 2, 3, 4, 5].map((n) => li(p(t(`item ${n} after paragraph ${para}`))))));
    }
  }
  return doc(...blocks);
}

function time<T>(f: () => T): { ms: number; result: T } {
  const start = performance.now();
  const result = f();
  return { ms: performance.now() - start, result };
}

test('a 20,000-word document converts in under a second each way', { timeout: 120_000 }, () => {
  const d = bigDocument();
  parseBody(serializeBody(d)); // warm-up
  const write = time(() => serializeBody(d));
  const read = time(() => parseBody(write.result));
  console.info(`serializeBody: ${write.ms.toFixed(1)} ms, parseBody: ${read.ms.toFixed(1)} ms`);
  expect(read.result.content!.length).toBe(d.content!.length);
  expect(write.ms).toBeLessThan(1000);
  expect(read.ms).toBeLessThan(1000);
});
