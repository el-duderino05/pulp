// SPDX-License-Identifier: MPL-2.0
import { describe, expect, it } from 'vitest';
import { friendlyMessage } from './errors';

const texts: Record<string, string> = {
  NotALibrary: "This folder isn't a Pulp Library. Choose the folder that has your Projects folder in it.",
  TooNew: 'This was made with a newer version of Pulp. Update Pulp to open it. Nothing in it has been changed.',
  PermissionDenied: "Pulp isn't allowed to change this file right now. Another program, often antivirus or a sync app, may be using it. Try again in a moment.",
  Locked: 'Another program is using this file. Close it, or wait for syncing to finish, then try again.',
  DiskFull: 'The disk is full. Free some space, then try again. Nothing was lost.',
  NotFound: "Pulp couldn't find this item. It may have been moved or deleted outside Pulp.",
  Exists: 'Something with that name is already there, so nothing was changed.',
};

describe('friendlyMessage', () => {
  for (const [code, text] of Object.entries(texts)) {
    it(`maps ${code}`, () => {
      expect(friendlyMessage({ code, detail: 'x' })).toBe(text);
    });
  }
  it('includes the detail for Other and Invalid', () => {
    expect(friendlyMessage({ code: 'Other', detail: 'boom' })).toBe('Something went wrong: boom');
    expect(friendlyMessage({ code: 'Invalid', detail: 'bad' })).toBe('Something went wrong: bad');
  });
  it('handles anything else', () => {
    expect(friendlyMessage(new Error('oops'))).toBe('Something went wrong: Error: oops');
    expect(friendlyMessage('plain')).toBe('Something went wrong: plain');
    expect(friendlyMessage({ code: 'Weird', detail: 'd' })).toContain('Something went wrong');
  });
});
