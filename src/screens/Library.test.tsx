// SPDX-License-Identifier: MPL-2.0
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, expect, it, vi } from 'vitest';
import Library from './Library';
import LibraryTrash from './LibraryTrash';
import { open } from '@tauri-apps/plugin-dialog';
import { t } from '../i18n/en';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

afterEach(() => { cleanup(); clearMocks(); });

const rows = [
  { id: 'a', name: 'The Harbour', path: 'D:/Pulp/Projects/The Harbour', tooNew: false, lastOpened: '2026-11-12T09:12:00Z', external: false, missing: false, unreadable: false },
  { id: 'b', name: 'Elsewhere', path: 'E:/Elsewhere', tooNew: false, lastOpened: null, external: true, missing: false, unreadable: false },
  { id: 'c', name: 'Essay drafts', path: 'D:/Pulp/Projects/Essay drafts', tooNew: true, lastOpened: null, external: false, missing: false, unreadable: false },
];
const opened = { id: 'n', name: 'Novel', path: 'D:/Pulp/Projects/Novel', sync: null, tree: [], report: {}, firstDocument: null };

function mock(extra: Record<string, unknown> = {}) {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in extra) return extra[cmd];
    if (cmd === 'list_projects') return rows;
    if (cmd === 'create_project') return opened;
    return null;
  });
  return calls;
}

it('lists projects with last opened', async () => {
  mock();
  render(<Library onOpen={vi.fn()} />);
  const harbour = (await screen.findByText('The Harbour')).closest('li')!;
  expect(within(harbour).getByText(/^Opened /)).toBeTruthy();
  const elsewhere = screen.getByText('Elsewhere').closest('li')!;
  expect(within(elsewhere).queryByText(/Opened/)).toBeNull();
  const essay = screen.getByText('Essay drafts').closest('li')!;
  expect(within(essay).getByText('Read-only')).toBeTruthy();
});

it.each([
  ['/home/x/Pulp', '/home/x/Pulp/Projects'],
  ['D:\\Pulp', 'D:\\Pulp\\Projects'],
])('footer path for root %s', async (root, shown) => {
  mock();
  render(<Library onOpen={vi.fn()} info={{ root, author: 'a', sync: null, recreated: [] }} />);
  const note = await screen.findByText(/^Projects are plain folders in/);
  expect(note.textContent).toBe(t('library.footer', { path: shown }));
});

it('create is disabled for a blank name', async () => {
  mock();
  render(<Library onOpen={vi.fn()} />);
  await userEvent.click(await screen.findByRole('button', { name: 'New project' }));
  const create = screen.getByRole('button', { name: 'Create' }) as HTMLButtonElement;
  expect(create.disabled).toBe(true);
  await userEvent.type(screen.getByLabelText('Project name'), '   ');
  expect(create.disabled).toBe(true);
});

it('new project opens it', async () => {
  const calls = mock();
  const onOpen = vi.fn();
  render(<Library onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole('button', { name: 'New project' }));
  await userEvent.type(screen.getByLabelText('Project name'), 'Novel');
  await userEvent.click(screen.getByRole('button', { name: 'Create' }));
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith(opened));
  expect(calls.find((c) => c.cmd === 'create_project')?.args).toEqual({ name: 'Novel' });
});

it('opening a project calls open_project then onOpen', async () => {
  mock({ open_project: opened });
  const onOpen = vi.fn();
  render(<Library onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole('button', { name: 'The Harbour' }));
  await waitFor(() => expect(onOpen).toHaveBeenCalledWith(opened));
});

it('too-new project shows the refusal', async () => {
  const calls = mock();
  const onOpen = vi.fn();
  render(<Library onOpen={onOpen} />);
  await userEvent.click(await screen.findByRole('button', { name: 'Read-only' }));
  expect((await screen.findByRole('alert')).textContent).toBe(t('error.TooNew'));
  expect(onOpen).not.toHaveBeenCalled();
  expect(calls.some((c) => c.cmd === 'open_project')).toBe(false);
});

it('row menu trashes a Library project, forgets an external one', async () => {
  const calls = mock();
  render(<Library onOpen={vi.fn()} />);
  await userEvent.click(await screen.findByRole('button', { name: 'More actions for The Harbour' }));
  await userEvent.click(screen.getByRole('menuitem', { name: 'Move to trash' }));
  await waitFor(() => expect(calls.find((c) => c.cmd === 'trash_project')?.args).toEqual({ path: rows[0].path }));

  await userEvent.click(screen.getByRole('button', { name: 'More actions for Elsewhere' }));
  await userEvent.click(screen.getByRole('menuitem', { name: 'Remove from list' }));
  await waitFor(() => expect(calls.find((c) => c.cmd === 'forget_external_project')?.args).toEqual({ path: 'E:/Elsewhere' }));
});

it('library trash restore and delete-forever confirm first', async () => {
  const entry = { entryId: 'e1', projectId: 'p1', name: 'Old novel', deletedAt: '2026-11-14T10:00:00Z' };
  const calls = mock({ list_library_trash: [entry], restore_project: rows[0] });
  render(<LibraryTrash onBack={vi.fn()} />);
  await userEvent.click(await screen.findByRole('button', { name: 'Delete forever…' }));
  const dialog = screen.getByRole('dialog');
  expect(within(dialog).getByText(t('trash.confirmDelete', { title: 'Old novel' }))).toBeTruthy();
  expect(calls.some((c) => c.cmd === 'delete_library_trash_entry')).toBe(false);

  await userEvent.click(within(dialog).getByRole('button', { name: 'Cancel' }));
  expect(screen.queryByRole('dialog')).toBeNull();
  expect(calls.some((c) => c.cmd === 'delete_library_trash_entry')).toBe(false);

  await userEvent.click(screen.getByRole('button', { name: 'Delete forever…' }));
  await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Delete' }));
  await waitFor(() => expect(calls.find((c) => c.cmd === 'delete_library_trash_entry')?.args).toEqual({ entry: 'e1' }));

  await userEvent.click(screen.getByRole('button', { name: 'Restore' }));
  await waitFor(() => expect(calls.find((c) => c.cmd === 'restore_project')?.args).toEqual({ entry: 'e1' }));
});

it('a project that cannot be found or read is greyed, unopenable, and removable', async () => {
  const gone = { id: 'E:/USB/Gone', name: 'Gone', path: 'E:/USB/Gone', tooNew: false, lastOpened: null, external: true, missing: true, unreadable: false };
  const broken = { id: 'D:/Pulp/Projects/broken', name: 'broken', path: 'D:/Pulp/Projects/broken', tooNew: false, lastOpened: null, external: false, missing: false, unreadable: true };
  const calls = mock({ list_projects: [gone, broken] });
  render(<Library onOpen={vi.fn()} />);
  const goneLi = (await screen.findByText('Gone')).closest('li')!;
  expect(within(goneLi).getByText(t('library.cantFind', { path: gone.path }))).toBeTruthy();
  expect(within(goneLi).queryByRole('button', { name: 'Gone' })).toBeNull();
  const brokenLi = screen.getByText('broken').closest('li')!;
  expect(within(brokenLi).getByText(t('library.cantRead', { path: broken.path }))).toBeTruthy();
  expect(calls.some((c) => c.cmd === 'open_project')).toBe(false);

  await userEvent.click(screen.getByRole('button', { name: 'More actions for Gone' }));
  await userEvent.click(screen.getByRole('menuitem', { name: 'Remove from list' }));
  await waitFor(() => expect(calls.find((c) => c.cmd === 'forget_external_project')?.args).toEqual({ path: gone.path }));
});

it('says so when folders were recreated', async () => {
  mock();
  render(<Library onOpen={vi.fn()} info={{ root: 'D:/Pulp', author: 'a', sync: null, recreated: ['Projects'] }} />);
  expect((await screen.findByRole('status')).textContent).toContain(t('report.recreated', { list: 'Projects' }));
});

it('a non-project folder gets a friendly message', async () => {
  vi.mocked(open).mockResolvedValue('D:/plain');
  mockIPC((cmd) => {
    if (cmd === 'add_external_project') throw { code: 'NotAProject', detail: 'D:/plain is not a Pulp project' };
    return cmd === 'list_projects' ? rows : null;
  });
  render(<Library onOpen={vi.fn()} />);
  await screen.findByText('The Harbour');
  await userEvent.click(screen.getByRole('button', { name: 'Open a folder…' }));
  expect((await screen.findByRole('alert')).textContent).toBe(t('error.NotAProject'));
});
