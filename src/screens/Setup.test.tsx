// SPDX-License-Identifier: MPL-2.0
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, expect, it, vi } from 'vitest';
import { open } from '@tauri-apps/plugin-dialog';
import Setup from './Setup';
import { t } from '../i18n/en';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

afterEach(() => { cleanup(); clearMocks(); });

const info = { root: 'D:/Pulp', author: 'Gonçalo', sync: null, recreated: [] };

it('setup submits name and path', async () => {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    return info;
  });
  const onDone = vi.fn();
  render(<Setup defaultPath="D:/Pulp" onDone={onDone} />);
  await userEvent.type(screen.getByLabelText('Your name'), 'Gonçalo');
  await userEvent.click(screen.getByRole('button', { name: 'Create my Library' }));
  await waitFor(() => expect(onDone).toHaveBeenCalledWith(info));
  expect(calls).toEqual([{ cmd: 'create_library', args: { path: 'D:/Pulp', author: 'Gonçalo' } }]);
});

it('setup needs a name', async () => {
  mockIPC(() => info);
  render(<Setup defaultPath="D:/Pulp" onDone={vi.fn()} />);
  const button = screen.getByRole('button', { name: 'Create my Library' });
  expect((button as HTMLButtonElement).disabled).toBe(true);
  await userEvent.type(screen.getByLabelText('Your name'), '   ');
  expect((button as HTMLButtonElement).disabled).toBe(true);
});

it('offers to use an existing Library from a chosen folder', async () => {
  vi.mocked(open).mockResolvedValue('E:/Pulp');
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => { calls.push({ cmd, args }); return info; });
  const onDone = vi.fn();
  render(<Setup defaultPath="D:/Pulp" onDone={onDone} />);
  await userEvent.click(screen.getByRole('button', { name: t('setup.useExisting') }));
  await waitFor(() => expect(onDone).toHaveBeenCalledWith(info));
  expect(calls).toEqual([{ cmd: 'open_library', args: { path: 'E:/Pulp' } }]);
});

it('when a Library already exists at the path, offers to open it instead', async () => {
  const calls: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd === 'create_library') throw { code: 'LibraryExists', detail: 'library.json already exists in D:/Pulp' };
    return info;
  });
  const onDone = vi.fn();
  render(<Setup defaultPath="D:/Pulp" onDone={onDone} />);
  await userEvent.type(screen.getByLabelText('Your name'), 'Ana');
  await userEvent.click(screen.getByRole('button', { name: 'Create my Library' }));
  expect((await screen.findByRole('alert')).textContent).toContain(t('setup.exists'));
  expect(onDone).not.toHaveBeenCalled();
  await userEvent.click(screen.getByRole('button', { name: t('setup.openIt') }));
  await waitFor(() => expect(onDone).toHaveBeenCalledWith(info));
  expect(calls[calls.length - 1]).toEqual({ cmd: 'open_library', args: { path: 'D:/Pulp' } });
});
