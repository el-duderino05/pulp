// SPDX-License-Identifier: MPL-2.0
import { afterEach, describe, expect, it, vi } from 'vitest';
import { handleShortcut, listCommands, registerCommand, runCommand } from './commands';

// Node environment has no KeyboardEvent; handleShortcut only reads these fields.
const ev = (key: string, mods: Partial<KeyboardEvent> = {}) => ({ key, ...mods }) as KeyboardEvent;
const offs:Array<() => void> = [];
const reg = (c: Parameters<typeof registerCommand>[0]) => {
  const off = registerCommand(c);
  offs.push(off);
  return off;
};
afterEach(() => {
  offs.splice(0).forEach((o) => o());
});

describe('command registry', () => {
  it('runs a registered command', async () => {
    const run = vi.fn();
    reg({ id: 'a', label: 'command.rename', run });
    await runCommand('a');
    expect(run).toHaveBeenCalledOnce();
  });
  it('does not run a disabled command', async () => {
    const run = vi.fn();
    reg({ id: 'a', label: 'command.rename', run, enabled: () => false });
    await runCommand('a');
    expect(run).not.toHaveBeenCalled();
  });
  it('throws on an unknown id', async () => {
    await expect(runCommand('nope')).rejects.toThrow();
  });
  it('runs the command bound to F2', () => {
    const run = vi.fn();
    reg({ id: 'r', label: 'command.rename', shortcut: 'F2', run });
    expect(handleShortcut(ev('F2'))).toBe(true);
    expect(run).toHaveBeenCalledOnce();
    expect(handleShortcut(ev('F3'))).toBe(false);
  });
  it('matches modifiers', () => {
    const run = vi.fn();
    reg({ id: 'n', label: 'command.rename', shortcut: 'Ctrl+N', run });
    expect(handleShortcut(ev('n'))).toBe(false);
    expect(handleShortcut(ev('n', { ctrlKey: true }))).toBe(true);
  });
  it('unregister removes the command', async () => {
    const off = reg({ id: 'a', label: 'command.rename', run: () => {} });
    off();
    expect(listCommands().some((c) => c.id === 'a')).toBe(false);
    await expect(runCommand('a')).rejects.toThrow();
  });
});
