// SPDX-License-Identifier: MPL-2.0
// The public API of the Pulp Markdown converter. Nothing outside src/markdown imports any other module here.
// Errors: serializeBody and normalizeDoc throw UnsupportedContentError for an unknown node or
// mark type, and checkDoc (called by both) throws a RangeError or TypeError for a structurally
// invalid document. They are not wrapped.
export { parseBody } from './parse';
export { serializeBody } from './serialize';
export { normalizeDoc, UnsupportedContentError } from './normalize';
export { pulpSchema, checkDoc } from './schema';
export { MARK_ORDER } from './model';
export type { PulpDoc, PulpNode, PulpMark, PulpMarkName } from './model';
