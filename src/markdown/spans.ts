// SPDX-License-Identifier: MPL-2.0
import type { MarkdownIt, StateInline } from 'markdown-it';

/** The only bracketed-span attribute blocks Pulp reads as marks. */
const SPAN_SUFFIXES: readonly { suffix: string; cls: string }[] = [
  { suffix: '{.underline}', cls: 'underline' },
  { suffix: '{.smallcaps}', cls: 'smallcaps' },
];

function span(state: StateInline, silent: boolean): boolean {
  const start = state.pos;
  if (state.src.charCodeAt(start) !== 0x5b /* [ */) return false;

  const labelEnd = state.md.helpers.parseLinkLabel(state, start, false);
  if (labelEnd < 0) return false;

  const after = labelEnd + 1;
  const match = SPAN_SUFFIXES.find(
    ({ suffix }) => after + suffix.length <= state.posMax && state.src.startsWith(suffix, after),
  );
  if (!match) return false;

  if (!silent) {
    const oldPosMax = state.posMax;
    const open = state.push('span_open', 'span', 1);
    open.attrs = [['class', match.cls]];
    open.markup = '[';
    state.pos = start + 1;
    state.posMax = labelEnd;
    state.md.inline.tokenize(state);
    state.posMax = oldPosMax;
    const close = state.push('span_close', 'span', -1);
    close.markup = ']' + match.suffix;
  }
  state.pos = after + match.suffix.length;
  return true;
}

/** Reads `[x]{.underline}` and `[x]{.smallcaps}` as `span` tokens with a `class` attribute. */
export function pulpSpans(md: MarkdownIt): void {
  md.inline.ruler.before('link', 'pulp_span', span);
}
