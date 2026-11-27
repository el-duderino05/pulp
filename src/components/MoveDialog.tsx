// SPDX-License-Identifier: MPL-2.0
import type { TreeNode } from '../app/core';
import { t } from '../i18n/en';

export function foldersOf(nodes: TreeNode[], depth = 0): { id: string; title: string; depth: number }[] {
  return nodes.filter((n) => n.kind === 'folder').flatMap((n) => [{ id: n.id, title: n.title, depth }, ...foldersOf(n.children, depth + 1)]);
}

export default function MoveDialog(p: {
  folders: { id: string; title: string; depth: number }[];
  currentId: string | null;
  onPick: (folderId: string) => void;
  onCancel: () => void;
}) {
  return (
    <div className="scrim" onKeyDown={(e) => { if (e.key === 'Escape') p.onCancel(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby="move-title" className="card dialog">
        <h2 id="move-title">{t('move.title')}</h2>
        <ul className="move-list">
          {p.folders.map((f, i) => (
            <li key={f.id} style={{ paddingLeft: f.depth * 16 }}>
              <button type="button" className="btn" autoFocus={i === 0} disabled={f.id === p.currentId} onClick={() => p.onPick(f.id)}>{f.title}</button>
            </li>
          ))}
        </ul>
        <div className="actions">
          <button type="button" className="btn" onClick={p.onCancel}>{t('common.cancel')}</button>
        </div>
      </div>
    </div>
  );
}
