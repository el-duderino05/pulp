// SPDX-License-Identifier: MPL-2.0
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { open } from '@tauri-apps/plugin-dialog';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, expect, it, vi } from 'vitest';
import { t } from '../i18n/en';
import LocateLibrary from './LocateLibrary';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

afterEach(() => { cleanup(); clearMocks(); });

it('missing library offers Locate only', () => {
  render(<LocateLibrary path="E:/Pulp" onDone={vi.fn()} />);
  expect(screen.getByText(t('library.missing', { path: 'E:/Pulp' }))).toBeTruthy();
  const buttons = screen.getAllByRole('button');
  expect(buttons.map((b) => b.textContent)).toEqual(['Locate…']);
});

it('locating a library calls open_library and finishes', async () => {
  const info = { root: 'F:/Pulp', author: 'A', sync: null, recreated: [] };
  vi.mocked(open).mockResolvedValue('F:/Pulp');
  mockIPC((cmd, args) => (cmd === 'open_library' && (args as { path: string }).path === 'F:/Pulp' ? info : Promise.reject({ code: 'Other' })));
  const onDone = vi.fn();
  render(<LocateLibrary path="E:/Pulp" onDone={onDone} />);
  await userEvent.click(screen.getByRole('button', { name: 'Locate…' }));
  await waitFor(() => expect(onDone).toHaveBeenCalledWith(info));
});

it('locating a non-library shows the friendly message', async () => {
  vi.mocked(open).mockResolvedValue('F:/Nope');
  mockIPC(() => Promise.reject({ code: 'NotALibrary', detail: '' }));
  const onDone = vi.fn();
  render(<LocateLibrary path="E:/Pulp" onDone={onDone} />);
  await userEvent.click(screen.getByRole('button', { name: 'Locate…' }));
  expect((await screen.findByRole('alert')).textContent).toBe(t('error.NotALibrary'));
  expect(onDone).not.toHaveBeenCalled();
});
