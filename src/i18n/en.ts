// SPDX-License-Identifier: MPL-2.0
// All interface text lives here. Tasks 8-11 add their screen copy.
export const en = {
  'error.NotALibrary': "This folder isn't a Pulp Library. Choose the folder that has your Projects folder in it.",
  'error.TooNew': 'This was made with a newer version of Pulp. Update Pulp to open it. Nothing in it has been changed.',
  'error.PermissionDenied': "Pulp isn't allowed to change this file right now. Another program, often antivirus or a sync app, may be using it. Try again in a moment.",
  'error.Locked': 'Another program is using this file. Close it, or wait for syncing to finish, then try again.',
  'error.DiskFull': 'The disk is full. Free some space, then try again. Nothing was lost.',
  'error.NotFound': "Pulp couldn't find this item. It may have been moved or deleted outside Pulp.",
  'error.Exists': 'Something with that name is already there, so nothing was changed.',
  'error.Other': 'Something went wrong: {detail}',
  'library.missing': "Pulp can't find your Library at {path}. If you moved it or unplugged a drive, show Pulp where it is now.",
  'report.found': "Pulp found {n} document(s) that weren't listed and added them at the end of their folder.",
  'report.missing': '{n} item(s) have lost their file. They stay in the tree, greyed, until the file comes back.',
  'report.duplicates': '{n} exact duplicate(s) were moved to the trash.',
  'report.copies': '{n} copied document(s) were given their own identity and marked "(copy)".',
  'report.conflicts': '{n} document(s) have two versions from a sync service. Both are kept.',
  'report.recreated': 'Pulp recreated missing folders: {list}.',
  'trash.confirmDelete': 'Delete "{title}" from Pulp? It goes to your computer\'s trash.',
  'trash.confirmEmpty': "Delete all {n} item(s) in this trash? They go to your computer's trash.",
  'command.rename': 'Rename',
  'command.trash': 'Move to trash',
} as const;

export type StringKey = keyof typeof en;

export function t(key: StringKey, vars: Record<string, string | number> = {}): string {
  return en[key].replace(/\{(\w+)\}/g, (m, name: string) => (name in vars ? String(vars[name]) : m));
}
