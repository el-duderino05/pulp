// SPDX-License-Identifier: MPL-2.0
export const formatWhen = (iso: string) =>
  new Date(iso).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' });
