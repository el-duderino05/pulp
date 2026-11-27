// SPDX-License-Identifier: MPL-2.0
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, expect, it, vi } from 'vitest';
import ProjectTrash from './ProjectTrash';
import { t } from '../i18n/en';
import type { TrashRow } from '../app/core';

afterEach(() => { cleanup(); clearMocks(); });

const rows = (): TrashRow[] => [
  { entryId: 'e1', title: 'Chapter 9', isFolder: false, itemCount: 1, from: 'Manuscript', deletedAt: '2026-11-14T10:41:00Z' },
  { entryId: 'e2', title: 'Old outline', isFolder: true, itemCount: 4, from: null, deletedAt: '2026-11-13T09:00:00Z' },
];

function mock() {
  let current = rows();
  const calls: { cmd: string; args: any }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === 'list_project_trash') return current;
    if (cmd === 'project_tree') return [{ id: 'f1', title: 'Manuscript', kind: 'folder', children: [] }];
    if (cmd === 'restore_trash_entry' || cmd === 'delete_trash_entry') {
      current = current.filter((r) => r.entryId !== (args as any).entry);
    }
    if (cmd === 'empty_project_trash') { current = []; return 2; }
    return null;
  });
  return calls;
}

it('rows show title, origin and date', async () => {
  mock();
  render(<ProjectTrash project="p1" name="The Harbour" onBack={vi.fn()} />);
  expect(await screen.findByText('Chapter 9')).toBeTruthy();
  expect(screen.getByText(/^From Manuscript · deleted /)).toBeTruthy();
  expect(screen.getByText(/^From the top level · deleted /)).toBeTruthy();
  expect(screen.getByText('(folder, 4 documents)')).toBeTruthy();
  expect(screen.getByText(t('project.trashLink', { n: 2 }))).toBeTruthy();
});

it('restore calls the command and removes the row', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<ProjectTrash project="p1" name="The Harbour" onBack={vi.fn()} />);
  await screen.findByText('Chapter 9');
  await user.click(screen.getAllByRole('button', { name: t('trash.restore') })[0]);
  await waitFor(() => expect(screen.queryByText('Chapter 9')).toBeNull());
  expect(calls.find((c) => c.cmd === 'restore_trash_entry')!.args).toEqual({ project: 'p1', entry: 'e1' });
});

it('delete forever and empty trash confirm first', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<ProjectTrash project="p1" name="The Harbour" onBack={vi.fn()} />);
  await screen.findByText('Chapter 9');
  await user.click(screen.getAllByRole('button', { name: t('trash.deleteForever') })[0]);
  expect(screen.getByText(t('trash.confirmDelete', { title: 'Chapter 9' }))).toBeTruthy();
  expect(calls.some((c) => c.cmd === 'delete_trash_entry')).toBe(false);
  await user.click(screen.getByRole('button', { name: t('common.cancel') }));
  await user.click(screen.getByRole('button', { name: t('trash.emptyButton') }));
  expect(screen.getByText(t('trash.confirmEmpty', { n: 2 }))).toBeTruthy();
  expect(calls.some((c) => c.cmd === 'empty_project_trash')).toBe(false);
  await user.click(screen.getByRole('button', { name: t('common.delete') }));
  await waitFor(() => expect(calls.some((c) => c.cmd === 'empty_project_trash')).toBe(true));
});

it('the sidebar tree is read-only: no menu, Delete does nothing', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<ProjectTrash project="p1" name="The Harbour" onBack={vi.fn()} />);
  const item = await screen.findByRole('treeitem', { name: /Manuscript/ });
  item.focus();
  await user.keyboard('{Shift>}{F10}{/Shift}');
  expect(screen.queryByRole('menu')).toBeNull();
  await user.pointer({ keys: '[MouseRight]', target: item });
  expect(screen.queryByRole('menu')).toBeNull();
  await user.keyboard('{F2}{Delete}');
  expect(screen.queryByRole('textbox')).toBeNull();
  expect(calls.some((c) => c.cmd === 'trash_node' || c.cmd === 'rename_node')).toBe(false);
});

it('a failed load shows the error, not the empty message', async () => {
  mockIPC((cmd) => {
    if (cmd === 'list_project_trash') throw { code: 'Other', detail: 'boom' };
    return [];
  });
  render(<ProjectTrash project="p1" name="The Harbour" onBack={vi.fn()} />);
  expect((await screen.findByRole('alert')).textContent).toContain('boom');
  expect(screen.queryByText(t('trash.empty'))).toBeNull();
});
