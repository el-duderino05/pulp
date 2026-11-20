// SPDX-License-Identifier: MPL-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import type { TreeNode } from '../app/core';
import { t } from '../i18n/en';
import ContextMenu from './ContextMenu';
import { DocumentIcon, FolderIcon } from './Icons';

export type TreeAction = 'open' | 'rename' | 'moveUp' | 'moveDown' | 'moveToFolder' | 'trash';
type Row = { node: TreeNode; level: number; index: number; count: number; parent: string | null };

function walk(nodes: TreeNode[], collapsed: Set<string>, level = 1, parent: string | null = null, out: Row[] = []): Row[] {
  nodes.forEach((node, index) => {
    out.push({ node, level, index, count: nodes.length, parent });
    if (node.kind === 'folder' && !collapsed.has(node.id)) walk(node.children, collapsed, level + 1, node.id, out);
  });
  return out;
}

// WAI-ARIA tree: roving tabindex, arrows move, Enter opens, F2 renames, Delete trashes, Shift+F10 / ContextMenu opens the menu.
export default function Tree(p: {
  nodes: TreeNode[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAction: (action: TreeAction, id: string) => void;
  renamingId?: string | null;
  onRename?: (id: string, title: string | null) => void;
  readOnly?: boolean; // no menu, rename, trash or open; navigation and expand/collapse stay
}) {
  const { nodes, selectedId, onSelect, onAction, renamingId = null, onRename, readOnly = false } = p;
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [menuId, setMenuId] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const rows = walk(nodes, collapsed);
  const el = (id: string) => root.current?.querySelector<HTMLElement>(`[data-id="${id}"]`) ?? null;
  const toggle = (id: string) =>
    setCollapsed((c) => {
      const n = new Set(c);
      if (!n.delete(id)) n.add(id);
      return n;
    });
  const closeMenu = useCallback(() => {
    setMenuId(null);
    if (menuId) el(menuId)?.focus();
  }, [menuId]);

  // A new item can sit under a collapsed folder: open its ancestors so the rename input shows.
  useEffect(() => {
    if (!renamingId) return;
    const path = (ns: TreeNode[]): string[] | null => {
      for (const n of ns) {
        if (n.id === renamingId) return [];
        const sub = path(n.children);
        if (sub) return [n.id, ...sub];
      }
      return null;
    };
    const anc = path(nodes) ?? [];
    if (anc.some((a) => collapsed.has(a))) setCollapsed((c) => new Set([...c].filter((x) => !anc.includes(x))));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [renamingId, nodes]);

  // After a rename ends, put focus back on the item.
  const wasRenaming = useRef<string | null>(null);
  useEffect(() => {
    if (wasRenaming.current && !renamingId) el(wasRenaming.current)?.focus();
    wasRenaming.current = renamingId;
  });

  const go = (id: string) => {
    onSelect(id);
    el(id)?.focus();
  };

  function onKeyDown(e: React.KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target.getAttribute('role') !== 'treeitem') return;
    const i = rows.findIndex((r) => r.node.id === target.dataset.id);
    if (i < 0) return;
    const { node, parent } = rows[i];
    const act = (a: TreeAction) => onAction(a, node.id);
    let handled = true;
    if (e.key === 'ArrowDown') go(rows[Math.min(i + 1, rows.length - 1)].node.id);
    else if (e.key === 'ArrowUp') go(rows[Math.max(i - 1, 0)].node.id);
    else if (e.key === 'Home') go(rows[0].node.id);
    else if (e.key === 'End') go(rows[rows.length - 1].node.id);
    else if (e.key === 'ArrowRight' && node.kind === 'folder') {
      if (collapsed.has(node.id)) toggle(node.id);
      else if (node.children.length) go(node.children[0].id);
    } else if (e.key === 'ArrowLeft') {
      if (node.kind === 'folder' && !collapsed.has(node.id)) toggle(node.id);
      else if (parent) go(parent);
    } else if (e.key === 'Enter') {
      if (node.kind === 'folder') toggle(node.id);
      else if (!readOnly) act('open');
    } else if (readOnly || node.kind === 'missing') handled = false; // missing items stay until the file returns (D7)
    else if (e.key === 'F2') act('rename');
    else if (e.key === 'Delete' || e.key === 'Backspace') act('trash'); // Backspace is the Mac delete key
    else if ((e.key === 'F10' && e.shiftKey) || e.key === 'ContextMenu') setMenuId(node.id);
    else handled = false;
    if (handled) e.preventDefault();
  }

  const menuItems = (r: Row) => {
    const act = (a: TreeAction) => () => onAction(a, r.node.id);
    const trash = { label: t('command.trash'), danger: true, onSelect: act('trash') };
    return [
      { label: t('command.rename'), onSelect: act('rename') },
      { label: t('command.moveUp'), onSelect: act('moveUp'), disabled: r.index === 0 },
      { label: t('command.moveDown'), onSelect: act('moveDown'), disabled: r.index === r.count - 1 },
      ...(r.node.kind === 'document' ? [{ label: t('command.moveToFolder'), onSelect: act('moveToFolder') }] : []),
      trash,
    ];
  };

  const tabbable = rows.some((r) => r.node.id === selectedId) ? selectedId : rows[0]?.node.id;
  return (
    <div ref={root} role="tree" aria-label={t('project.tree')} className="tree" onKeyDown={onKeyDown}>
      {rows.map((r) => {
        const { node, level } = r;
        const missing = node.kind === 'missing';
        const folder = node.kind === 'folder';
        const selected = node.id === selectedId;
        return (
          <div key={node.id} role="none" className="tree-wrap">
            <div
              role="treeitem"
              data-id={node.id}
              aria-level={level}
              aria-setsize={r.count}
              aria-posinset={r.index + 1}
              aria-selected={selected}
              aria-expanded={folder ? !collapsed.has(node.id) : undefined}
              aria-disabled={missing || undefined}
              aria-description={missing ? t('project.missingWhy') : undefined}
              title={missing ? t('project.missingWhy') : undefined}
              tabIndex={node.id === tabbable ? 0 : -1}
              className={`tree-item${selected ? ' selected' : ''}${missing ? ' missing' : ''}${folder ? ' folder' : ''}`}
              style={{ paddingLeft: 8 + (level - 1) * 28 }}
              onClick={() => {
                onSelect(node.id);
                if (folder) toggle(node.id);
                else if (!readOnly) onAction('open', node.id);
              }}
              onContextMenu={(e) => {
                e.preventDefault();
                if (readOnly || missing) return;
                onSelect(node.id);
                setMenuId(node.id);
              }}
            >
              {folder && <span aria-hidden="true" className="chev">{collapsed.has(node.id) ? '▸' : '▾'}</span>}
              {folder ? <FolderIcon /> : <DocumentIcon color={selected ? 'var(--accent)' : 'var(--muted)'} />}
              {renamingId === node.id ? (
                <RenameInput title={node.title} onDone={(v) => onRename?.(node.id, v)} />
              ) : (
                <span className="tree-title">{missing ? t('project.missing') : node.title}</span>
              )}
            </div>
            {menuId === node.id && (
              <div className="tree-menu">
                <ContextMenu
                  items={menuItems(r)}
                  onClose={closeMenu}
                />
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
}

// Enter commits, Esc cancels; the parent decides what a blank or unchanged title means.
function RenameInput({ title, onDone }: { title: string; onDone: (v: string | null) => void }) {
  const done = useRef(false);
  const finish = (v: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(v);
  };
  return (
    <input
      className="tree-rename"
      aria-label={t('project.renameLabel', { title })}
      defaultValue={title}
      autoFocus
      onFocus={(e) => e.currentTarget.select()}
      onClick={(e) => e.stopPropagation()}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === 'Enter') finish(e.currentTarget.value);
        if (e.key === 'Escape') finish(null);
      }}
      onBlur={(e) => finish(e.currentTarget.value)}
    />
  );
}
