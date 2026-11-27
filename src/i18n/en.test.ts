// SPDX-License-Identifier: MPL-2.0
import { describe, expect, it } from 'vitest';
import { en, t } from './en';

describe('t', () => {
  it('replaces placeholders', () => {
    expect(t('report.found', { n: 1 })).toBe(
      "Pulp found 1 document(s) that weren't listed and added them at the end of their folder.",
    );
    expect(t('trash.confirmDelete', { title: 'Draft' })).toBe(
      'Delete "Draft" from Pulp? It goes to your computer\'s trash.',
    );
  });

  it('has every fixed key', () => {
    const keys = [
      'error.NotALibrary', 'error.TooNew', 'error.PermissionDenied', 'error.Locked',
      'error.DiskFull', 'error.NotFound', 'error.Exists', 'error.Other', 'library.missing',
      'report.found', 'report.missing', 'report.duplicates', 'report.copies',
      'report.conflicts', 'report.recreated', 'trash.confirmDelete', 'trash.confirmEmpty',
    ];
    for (const k of keys) expect(Object.keys(en)).toContain(k);
  });
});
