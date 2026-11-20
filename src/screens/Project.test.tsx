// SPDX-License-Identifier: MPL-2.0
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, expect, it, vi } from 'vitest';
import { useState } from 'react';
import Library from './Library';
import { handleShortcut, registerCommand } from '../app/commands';
import Project from './Project';
import { t } from '../i18n/en';
import type { OpenedProject, TreeNode } from '../app/core';

afterEach(() => { cleanup(); clearMocks(); });

const doc = (id: string, title: string): TreeNode => ({ id, title, kind: 'document', children: [] });
const baseTree = (): TreeNode[] => [
  { id: 'f1', title: 'Manuscript', kind: 'folder', children: [doc('d1', 'Chapter 1'), doc('d2', 'Chapter 2'), { id: 'm1', title: '', kind: 'missing', children: [] }] },
  { id: 'f2', title: 'Part Two', kind: 'folder', children: [doc('d3', 'Scene')] },
];
const noReport = { found: [], missing: [], duplicates: [], copies: [], conflicts: [], recreated: [] };
const opened = (over: Partial<OpenedProject> = {}): OpenedProject => ({
  id: 'p1', name: 'The Harbour', path: 'D:/Pulp/Projects/The Harbour', sync: null,
  tree: baseTree(), report: noReport, firstDocument: 'd1', ...over,
});

function mock(over: Record<string, (args: any) => unknown> = {}, tree = baseTree) {
  const calls: { cmd: string; args: any }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in over) return over[cmd](args);
    if (cmd === 'project_tree') return tree();
    if (cmd === 'read_document') return { title: `Title ${(args as any).id}`, body: `Body of ${(args as any).id}` };
    if (cmd === 'create_document') return 'new1';
    return null;
  });
  return calls;
}
const item = (name: string) => screen.getByRole('treeitem', { name });
const sent = (calls: { cmd: string; args: any }[], cmd: string) => calls.filter((c) => c.cmd === cmd);

it('opens on first document', async () => {
  mock();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  expect(await screen.findByRole('heading', { name: 'Title d1' })).toBeTruthy();
  expect(screen.getByText('Body of d1')).toBeTruthy();
  expect(item('Chapter 1').getAttribute('aria-selected')).toBe('true');
  expect(screen.getByText(t('project.preview'))).toBeTruthy();
});

it('rename keeps the typed title', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.click(item('Chapter 2'));
  await user.keyboard('{F2}');
  await user.keyboard('Capítulo 1: A Chegada? 🌊{Enter}');
  await waitFor(() => expect(sent(calls, 'rename_node')).toHaveLength(1));
  expect(sent(calls, 'rename_node')[0].args).toEqual({ project: 'p1', id: 'd2', title: 'Capítulo 1: A Chegada? 🌊' });
});

it('blank rename restores the old title', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.click(item('Chapter 2'));
  await user.keyboard('{F2}{Backspace}{Enter}');
  expect(sent(calls, 'rename_node')).toHaveLength(0);
  expect(await screen.findByRole('treeitem', { name: 'Chapter 2' })).toBeTruthy();
  expect(screen.queryByRole('textbox')).toBeNull();
});

it('move up is disabled for the first item; reorder sends index-1', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.pointer({ keys: '[MouseRight]', target: item('Chapter 1') });
  expect((screen.getByRole('menuitem', { name: 'Move up' }) as HTMLButtonElement).disabled).toBe(true);
  await user.keyboard('{Escape}');
  await user.pointer({ keys: '[MouseRight]', target: item('Chapter 2') });
  await user.click(screen.getByRole('menuitem', { name: 'Move up' }));
  await waitFor(() => expect(sent(calls, 'reorder_node')).toHaveLength(1));
  expect(sent(calls, 'reorder_node')[0].args).toEqual({ project: 'p1', id: 'd2', index: 0 });
});

it('move to folder lists folders and calls move_document; folders do not offer it', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.pointer({ keys: '[MouseRight]', target: item('Part Two') });
  expect(screen.queryByRole('menuitem', { name: 'Move to folder…' })).toBeNull();
  await user.keyboard('{Escape}');
  await user.pointer({ keys: '[MouseRight]', target: item('Chapter 1') });
  await user.click(screen.getByRole('menuitem', { name: 'Move to folder…' }));
  const dialog = await screen.findByRole('dialog');
  await user.click(within(dialog).getByRole('button', { name: 'Part Two' }));
  await waitFor(() => expect(sent(calls, 'move_document')).toHaveLength(1));
  expect(sent(calls, 'move_document')[0].args).toEqual({ project: 'p1', id: 'd1', folder: 'f2', index: 1 });
});

it('trash clears the pane when the open item is trashed', async () => {
  let gone = false;
  const calls = mock({ trash_node: () => { gone = true; return 'e1'; } }, () => {
    const t = baseTree();
    if (gone) t[0].children.shift();
    return t;
  });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Title d1' });
  item('Chapter 1').focus();
  await user.keyboard('{Delete}');
  await waitFor(() => expect(screen.queryByRole('heading', { name: 'Title d1' })).toBeNull());
  expect(sent(calls, 'trash_node')[0].args).toEqual({ project: 'p1', id: 'd1' });
  expect(screen.queryByRole('treeitem', { name: 'Chapter 1' })).toBeNull();
});

it("missing items are greyed and can't be opened", async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Title d1' });
  const m = item(t('project.missing'));
  expect(m.getAttribute('aria-disabled')).toBe('true');
  expect(m.getAttribute('title')).toBe(t('project.missingWhy'));
  await user.click(m);
  await user.keyboard('{Enter}');
  expect(sent(calls, 'read_document').some((c) => c.args.id === 'm1')).toBe(false);
});

it('notice shows the report counts and details', async () => {
  mock();
  const report = { ...noReport, found: ['Loose.md'], recreated: ['Notes', 'Extra'] };
  const user = userEvent.setup();
  render(<Project opened={opened({ report })} onBack={vi.fn()} />);
  const notice = await screen.findByRole('status');
  expect(notice.textContent).toContain(t('report.found', { n: 1 }));
  expect(notice.textContent).toContain(t('report.recreated', { list: 'Notes, Extra' }));
  expect(notice.textContent).not.toContain('lost their file');
  expect(screen.queryByText('Loose.md')).toBeNull();
  await user.click(within(notice).getByRole('button', { name: 'Details' }));
  expect(screen.getByText('Loose.md')).toBeTruthy();
  await user.click(within(notice).getByRole('button', { name: 'Dismiss' }));
  expect(screen.queryByRole('status')).toBeNull();
});

it('no notice when the report is empty', async () => {
  mock();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Title d1' });
  expect(screen.queryByRole('status')).toBeNull();
});

it('body text is shown as text', async () => {
  mock({ read_document: () => ({ title: 'T', body: '<b>x</b>\n\nSecond paragraph' }) });
  const { container } = render(<Project opened={opened()} onBack={vi.fn()} />);
  expect(await screen.findByText('<b>x</b>')).toBeTruthy();
  expect(container.querySelector('article b')).toBeNull();
  expect(container.querySelectorAll('article p').length).toBe(3); // label + 2 paragraphs
});

it('keyboard: arrows move selection, Enter opens, Shift+F10 opens the menu', async () => {
  const calls = mock();
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Title d1' });
  item('Chapter 1').focus();
  await user.keyboard('{ArrowDown}');
  expect(item('Chapter 2').getAttribute('aria-selected')).toBe('true');
  expect(document.activeElement).toBe(item('Chapter 2'));
  await user.keyboard('{Enter}');
  expect(await screen.findByRole('heading', { name: 'Title d2' })).toBeTruthy();
  expect(sent(calls, 'read_document').some((c) => c.args.id === 'd2')).toBe(true);
  await user.keyboard('{Shift>}{F10}{/Shift}');
  expect(screen.getByRole('menu')).toBeTruthy();
});

it('+ Document goes in the parent folder, titled Untitled, and opens rename', async () => {
  let made = false;
  const calls = mock({ create_document: () => { made = true; return 'new1'; } }, () => {
    const t = baseTree();
    if (made) t[1].children.push(doc('new1', 'Untitled'));
    return t;
  });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.click(item('Scene'));
  await user.click(screen.getByRole('button', { name: '+ Document' }));
  await waitFor(() => expect(sent(calls, 'create_document')).toHaveLength(1));
  expect(sent(calls, 'create_document')[0].args).toEqual({ project: 'p1', folder: 'f2', title: 'Untitled' });
  expect(await screen.findByRole('textbox')).toBeTruthy();
});

it('shows storage line, trash count, and Library link closes the project', async () => {
  const calls = mock({ list_project_trash: () => [{}, {}] });
  const onBack = vi.fn();
  const user = userEvent.setup();
  render(<Project opened={opened({ sync: 'OneDrive' })} onBack={onBack} />);
  expect(screen.getByText('Saved in D:/Pulp/Projects/The Harbour · inside OneDrive')).toBeTruthy();
  expect(await screen.findByRole('button', { name: 'Trash · 2' })).toBeTruthy();
  await user.click(screen.getByRole('button', { name: '‹ Library' }));
  await waitFor(() => expect(onBack).toHaveBeenCalled());
  expect(sent(calls, 'close_project')[0].args).toEqual({ project: 'p1' });
});

it('renaming the open document updates the pane heading', async () => {
  let renamed = false;
  mock({ rename_node: () => { renamed = true; return null; }, read_document: () => ({ title: renamed ? 'Brand new' : 'Old title', body: 'x' }) },
    () => { const t = baseTree(); if (renamed) t[0].children[0].title = 'Brand new'; return t; });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Old title' });
  item('Chapter 1').focus();
  await user.keyboard('{F2}Brand new{Enter}');
  expect(await screen.findByRole('heading', { name: 'Brand new' })).toBeTruthy();
});

it('+ Document under a collapsed folder opens rename on the new item', async () => {
  let made = false;
  mock({ create_document: () => { made = true; return 'new1'; } }, () => {
    const t = baseTree();
    if (made) t[1].children.push(doc('new1', 'Untitled'));
    return t;
  });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.click(item('Part Two')); // collapses it
  expect(screen.queryByRole('treeitem', { name: 'Scene' })).toBeNull();
  await user.click(screen.getByRole('button', { name: '+ Document' }));
  const input = await screen.findByRole('textbox');
  expect(document.activeElement).toBe(input);
});

it('failed action keeps the tree and shows the message', async () => {
  const calls = mock({ rename_node: () => { throw { code: 'Locked', detail: '' }; } });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await user.click(item('Chapter 2'));
  await user.keyboard('{F2}New name{Enter}');
  const alert = await screen.findByRole('alert');
  expect(alert.textContent).toContain(t('error.Locked'));
  expect(sent(calls, 'rename_node')).toHaveLength(1);
  expect(screen.getByRole('treeitem', { name: 'Chapter 2' })).toBeTruthy();
  await user.click(screen.getByRole('button', { name: t('notice.dismiss') }));
  expect(screen.queryByRole('alert')).toBeNull();
});

it('a rejected command from a button shows in the banner', async () => {
  mock({ create_document: () => { throw { code: 'DiskFull', detail: '' }; } });
  const user = userEvent.setup();
  render(<Project opened={opened()} onBack={vi.fn()} />);
  await screen.findByRole('heading', { name: 'Title d1' });
  await user.click(screen.getByRole('button', { name: t('project.newDocument') }));
  expect((await screen.findByRole('alert')).textContent).toContain(t('error.DiskFull'));
});

function Shell() {
  const [p, setP] = useState(true);
  return p ? <Project opened={opened()} onBack={() => setP(false)} /> : <Library onOpen={vi.fn()} />;
}

it('a rejected shortcut shows in the active screen banner and is gone after navigating away', async () => {
  mock({ list_projects: () => [] });
  const user = userEvent.setup();
  const off = registerCommand({ id: 'x', label: 'command.rename', shortcut: 'Ctrl+Shift+9', run: async () => { throw { code: 'Locked', detail: '' }; } });
  render(<Shell />);
  await screen.findByRole('heading', { name: 'Title d1' });
  act(() => { handleShortcut({ key: '9', ctrlKey: true, shiftKey: true } as KeyboardEvent); });
  expect((await screen.findByRole('alert')).textContent).toContain(t('error.Locked'));
  await user.click(screen.getByRole('button', { name: t('project.back') }));
  expect(await screen.findByRole('heading', { name: t('library.title') })).toBeTruthy();
  expect(screen.queryByRole('alert')).toBeNull();
  off();
});
