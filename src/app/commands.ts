// SPDX-License-Identifier: MPL-2.0
import type { StringKey } from '../i18n/en';

export type Command = {
  id: string;
  label: StringKey;
  shortcut?: string;
  run: () => void | Promise<void>;
  enabled?: () => boolean;
};

const commands = new Map<string, Command>();

export function registerCommand(c: Command): () => void {
  commands.set(c.id, c);
  return () => {
    if (commands.get(c.id) === c) commands.delete(c.id);
  };
}

export async function runCommand(id: string): Promise<void> {
  const c = commands.get(id);
  if (!c) throw new Error(`Unknown command: ${id}`);
  if (c.enabled && !c.enabled()) return;
  await c.run();
}

export function listCommands(): Command[] {
  return [...commands.values()];
}

// Canonical form: "Ctrl+Shift+Alt+<key lowercased>"; Cmd counts as Ctrl.
const canon = (ctrl: boolean, shift: boolean, alt: boolean, key: string) =>
  [ctrl && 'ctrl', shift && 'shift', alt && 'alt', key.toLowerCase()].filter(Boolean).join('+');

function canonShortcut(s: string): string {
  const parts = s.split('+').map((p) => p.trim().toLowerCase());
  const key = parts[parts.length - 1];
  return canon(parts.includes('ctrl') || parts.includes('cmd'), parts.includes('shift'), parts.includes('alt'), key);
}

export function handleShortcut(e: KeyboardEvent): boolean {
  const pressed = canon(e.ctrlKey || e.metaKey, e.shiftKey, e.altKey, e.key);
  for (const c of commands.values()) {
    if (!c.shortcut || canonShortcut(c.shortcut) !== pressed) continue;
    if (c.enabled && !c.enabled()) continue;
    void c.run();
    return true;
  }
  return false;
}
