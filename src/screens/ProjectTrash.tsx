// SPDX-License-Identifier: MPL-2.0
import { useCallback, useEffect, useState } from 'react';
import {
  deleteTrashEntry, emptyProjectTrash, listProjectTrash, projectTree, restoreTrashEntry,
  type TrashRow, type TreeNode,
} from '../app/core';
import { friendlyMessage } from '../app/errors';
import { formatWhen } from '../app/format';
import { t } from '../i18n/en';
import ConfirmDialog from '../components/ConfirmDialog';
import { FolderIcon, TrashIcon } from '../components/Icons';
import Tree from '../components/Tree';

type Pending = { kind: 'one'; row: TrashRow } | { kind: 'all' };
const noop = () => {};

export default function ProjectTrash({ project, name, onBack }: { project: string; name: string; onBack: () => void }) {
  const [rows, setRows] = useState<TrashRow[]>([]);
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // A restore changes the tree too, so both come from the core every time.
  const refresh = useCallback(async () => {
    setRows((await listProjectTrash(project)) ?? []);
    setTree((await projectTree(project)) ?? []);
  }, [project]);
  const act = async (fn: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(friendlyMessage(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void act(refresh);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [project]);

  const confirm = () => {
    const p = pending!;
    setPending(null);
    void act(async () => {
      if (p.kind === 'one') await deleteTrashEntry(project, p.row.entryId);
      else await emptyProjectTrash(project);
      await refresh();
    });
  };

  return (
    <div className="project">
      <nav aria-label={t('project.nav')} className="project-nav">
        <div className="project-head">
          <button type="button" className="back" onClick={onBack}>{t('trash.backToProject')}</button>
          <span className="project-name">{name}</span>
        </div>
        <div className="project-tree">
          <Tree nodes={tree} selectedId={null} onSelect={noop} onAction={noop} renamingId={null} onRename={noop} />
        </div>
        <div className="project-foot">
          <button type="button" className="trash-link trash-link-current" aria-current="page">
            <TrashIcon />{t('project.trashLink', { n: rows.length })}
          </button>
        </div>
      </nav>
      <main className="project-main trash-main">
        <div className="trash-head">
          <div className="field">
            <h1>{t('trash.title')}</h1>
            <p className="note">{t('trash.intro')}</p>
          </div>
          <button type="button" className="btn btn-danger-outline" disabled={busy || rows.length === 0} onClick={() => setPending({ kind: 'all' })}>
            {t('trash.emptyButton')}
          </button>
        </div>
        {error && <p role="alert" className="error">{error}</p>}
        {!error && rows.length === 0 && <p className="note">{t('trash.empty')}</p>}
        {rows.length > 0 && (
          <ul className="list">
            {rows.map((r) => (
              <li key={r.entryId}>
                <FolderIcon size={20} />
                <div className="text">
                  <span className="name">
                    {r.title}
                    {r.isFolder && <> <span className="meta">{t('trash.folderNote', { n: r.itemCount })}</span></>}
                  </span>
                  <span className="meta">
                    {t('trash.rowMeta', { from: r.from ? t('trash.from', { from: r.from }) : t('trash.fromTop'), when: formatWhen(r.deletedAt) })}
                  </span>
                </div>
                <button type="button" className="btn btn-primary trash-btn" disabled={busy} onClick={() => void act(async () => { await restoreTrashEntry(project, r.entryId); await refresh(); })}>
                  {t('trash.restore')}
                </button>
                <button type="button" className="btn btn-danger-outline trash-btn" disabled={busy} onClick={() => setPending({ kind: 'one', row: r })}>
                  {t('trash.deleteForever')}
                </button>
              </li>
            ))}
          </ul>
        )}
      </main>
      {pending && (
        <ConfirmDialog
          danger
          title={pending.kind === 'one' ? t('trash.deleteForever') : t('trash.emptyButton')}
          message={pending.kind === 'one' ? t('trash.confirmDelete', { title: pending.row.title }) : t('trash.confirmEmpty', { n: rows.length })}
          confirmLabel={t('common.delete')}
          onConfirm={confirm}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
