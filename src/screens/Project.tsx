// SPDX-License-Identifier: MPL-2.0
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  closeProject, createDocument, createFolder, listProjectTrash, moveDocument, projectTree, readDocument,
  renameNode, reorderNode, trashNode,
  type DocumentView, type OpenedProject, type TreeNode,
} from '../app/core';
import { registerCommand, runCommand } from '../app/commands';
import { friendlyMessage } from '../app/errors';
import ErrorBanner from '../components/ErrorBanner';
import { t } from '../i18n/en';
import { TrashIcon } from '../components/Icons';
import MoveDialog, { foldersOf } from '../components/MoveDialog';
import Notice from '../components/Notice';
import Tree, { type TreeAction } from '../components/Tree';
import ProjectTrash from './ProjectTrash';

type Found = { node: TreeNode; parent: TreeNode | null; siblings: TreeNode[]; index: number };
function find(nodes: TreeNode[], id: string | null, parent: TreeNode | null = null): Found | null {
  if (!id) return null;
  for (let index = 0; index < nodes.length; index++) {
    const node = nodes[index];
    if (node.id === id) return { node, parent, siblings: nodes, index };
    const hit = find(node.children, id, node);
    if (hit) return hit;
  }
  return null;
}

export default function Project({ opened, onBack }: { opened: OpenedProject; onBack: () => void }) {
  const pid = opened.id;
  const [tree, setTree] = useState(opened.tree);
  const [selectedId, setSelectedId] = useState<string | null>(opened.firstDocument);
  const [doc, setDoc] = useState<DocumentView | null>(null);
  const [openId, setOpenId] = useState<string | null>(null);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [moving, setMoving] = useState<string | null>(null);
  const [trashCount, setTrashCount] = useState(0);
  const [view, setView] = useState<'project' | 'trash'>('project');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const busyRef = useRef(false);
  // Commands read the latest state through this, so they can be registered once per view.
  const live = useRef({ tree, selectedId, openId });
  live.current = { tree, selectedId, openId };

  // One action at a time; a failure shows a friendly message and leaves the tree as it was.
  const act = useCallback(async (fn: () => Promise<void>) => {
    if (busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(friendlyMessage(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }, []);

  const run = (id: string) => runCommand(id).catch((e) => setError(friendlyMessage(e)));

  // The tree is always replaced with what the core says, never patched locally.
  const refresh = useCallback(async () => {
    const next = await projectTree(pid);
    setTree(next);
    const { selectedId: sel, openId: open } = live.current;
    if (sel && !find(next, sel)) setSelectedId(null);
    if (open && !find(next, open)) {
      setOpenId(null);
      setDoc(null);
    } else if (open) setDoc(await readDocument(pid, open)); // titles may have changed
    setTrashCount(((await listProjectTrash(pid)) ?? []).length);
  }, [pid]);

  const open = useCallback(async (id: string) => {
    setDoc(await readDocument(pid, id));
    setOpenId(id);
  }, [pid]);

  useEffect(() => {
    void act(async () => {
      if (opened.firstDocument) await open(opened.firstDocument);
      setTrashCount(((await listProjectTrash(pid)) ?? []).length);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pid]);

  useEffect(() => {
    if (view !== 'project') return;
    const sel = () => find(live.current.tree, live.current.selectedId);
    const withSel = (fn: (f: Found) => Promise<void>) => () => act(async () => {
      const f = sel();
      if (f) await fn(f);
    });
    const create = (kind: 'doc' | 'folder') => () => act(async () => {
      const f = sel();
      const container = f ? (f.node.kind === 'folder' ? f.node : f.parent) : null;
      const title = t('project.untitled');
      let id: string;
      if (kind === 'doc') {
        const target = container ?? live.current.tree.find((n) => n.kind === 'folder');
        if (!target) return;
        id = await createDocument(pid, target.id, title);
      } else {
        id = await createFolder(pid, container?.id ?? null, title);
      }
      await refresh();
      setSelectedId(id);
      setRenamingId(id);
    });
    const move = (by: number) => withSel(async (f) => {
      await reorderNode(pid, f.node.id, f.index + by);
      await refresh();
    });
    const can = (pred: (f: Found) => boolean) => () => {
      const f = sel();
      return !!f && pred(f);
    };
    const offs = [
      registerCommand({ id: 'project.newDocument', label: 'project.newDocument', run: create('doc') }),
      registerCommand({ id: 'project.newFolder', label: 'project.newFolder', run: create('folder') }),
      registerCommand({
        id: 'project.rename', label: 'command.rename', enabled: can((f) => f.node.kind !== 'missing'),
        run: () => setRenamingId(live.current.selectedId),
      }),
      registerCommand({ id: 'project.moveUp', label: 'command.moveUp', enabled: can((f) => f.index > 0), run: move(-1) }),
      registerCommand({ id: 'project.moveDown', label: 'command.moveDown', enabled: can((f) => f.index < f.siblings.length - 1), run: move(1) }),
      registerCommand({
        id: 'project.moveToFolder', label: 'command.moveToFolder', enabled: can((f) => f.node.kind === 'document'),
        run: () => setMoving(live.current.selectedId),
      }),
      registerCommand({
        id: 'project.trash', label: 'command.trash', enabled: can(() => true),
        run: withSel(async (f) => {
          await trashNode(pid, f.node.id);
          await refresh();
        }),
      }),
      registerCommand({ id: 'project.showTrash', label: 'library.trash', run: () => setView('trash') }),
      registerCommand({
        id: 'nav.library', label: 'project.back',
        run: () => act(async () => {
          await closeProject(pid);
          onBack();
        }),
      }),
    ];
    return () => offs.forEach((off) => off());
  }, [view, pid, act, refresh, onBack]);

  const select = (id: string) => setSelectedId(id);
  const onAction = (action: TreeAction, id: string) => {
    setSelectedId(id);
    live.current.selectedId = id; // commands run before the next render
    if (action === 'open') {
      if (find(live.current.tree, id)?.node.kind === 'document') void act(() => open(id));
      return;
    }
    run(`project.${action}`);
  };
  const onRename = (id: string, title: string | null) => {
    setRenamingId(null);
    const f = find(live.current.tree, id);
    if (title === null || !title.trim() || !f || title === f.node.title) return;
    void act(async () => {
      await renameNode(pid, id, title);
      await refresh();
    });
  };
  const moveTo = (folderId: string) => {
    const id = moving;
    setMoving(null);
    if (!id) return;
    void act(async () => {
      const target = find(live.current.tree, folderId);
      await moveDocument(pid, id, folderId, target?.node.children.length ?? 0);
      await refresh();
    });
  };

  if (view === 'trash') {
    return <ProjectTrash project={pid} name={opened.name} onBack={() => { setView('project'); void act(refresh); }} />;
  }

  const paragraphs = doc ? doc.body.split(/\n\s*\n/).filter((x) => x.trim()) : [];
  const current = find(tree, moving);
  return (
    <div className="project">
      <nav aria-label={t('project.nav')} className="project-nav">
        <div className="project-head">
          <button type="button" className="back" onClick={() => run('nav.library')}>{t('project.back')}</button>
          <span className="project-name">{opened.name}</span>
        </div>
        <div className="project-add">
          <button type="button" className="btn-small" disabled={busy} onClick={() => run('project.newDocument')}>{t('project.newDocument')}</button>
          <button type="button" className="btn-small" disabled={busy} onClick={() => run('project.newFolder')}>{t('project.newFolder')}</button>
        </div>
        <div className="project-tree">
          <Tree nodes={tree} selectedId={selectedId} onSelect={select} onAction={onAction} renamingId={renamingId} onRename={onRename} />
        </div>
        <div className="project-foot">
          <button type="button" className="trash-link" onClick={() => run('project.showTrash')}>
            <TrashIcon />{t('project.trashLink', { n: trashCount })}
          </button>
          <span className="note">{t('project.saved', { path: opened.path })}{opened.sync && t('project.inside', { sync: opened.sync })}</span>
        </div>
      </nav>
      <main className="project-main">
        <Notice report={opened.report} />
        {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}
        <div className="project-scroll">
          <article>
            <p className="eyebrow">{t('project.preview')}</p>
            {doc && (
              <>
                <h1>{doc.title}</h1>
                {paragraphs.map((para, i) => <p key={i} className="doc-text">{para}</p>)}
              </>
            )}
          </article>
        </div>
      </main>
      {moving && current && (
        <MoveDialog folders={foldersOf(tree)} currentId={current.parent?.id ?? null} onPick={moveTo} onCancel={() => setMoving(null)} />
      )}
    </div>
  );
}
