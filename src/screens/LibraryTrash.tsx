// SPDX-License-Identifier: MPL-2.0
import { useCallback, useEffect, useState } from 'react';
import {
  deleteLibraryTrashEntry, emptyLibraryTrash, listLibraryTrash, restoreProject, type LibraryTrashEntry,
} from '../app/core';
import { friendlyMessage } from '../app/errors';
import { t } from '../i18n/en';
import ConfirmDialog from '../components/ConfirmDialog';
import { FolderIcon } from '../components/Icons';

type Pending = { kind: 'one'; entry: LibraryTrashEntry } | { kind: 'all' };

export default function LibraryTrash({ onBack }: { onBack: () => void }) {
  const [entries, setEntries] = useState<LibraryTrashEntry[]>([]);
  const [pending, setPending] = useState<Pending | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => setEntries(await listLibraryTrash()), []);
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
  }, []);

  const confirm = () => {
    const p = pending!;
    setPending(null);
    void act(async () => {
      if (p.kind === 'one') await deleteLibraryTrashEntry(p.entry.entryId);
      else await emptyLibraryTrash();
      await refresh();
    });
  };

  return (
    <div className="page">
      <main className="page-body">
        <button type="button" className="back" onClick={onBack}>{t('trash.back')}</button>
        <div className="trash-head">
          <div className="field">
            <h1>{t('trash.title')}</h1>
            <p className="note">{t('trash.intro')}</p>
          </div>
          <button type="button" className="btn btn-danger-outline" disabled={busy || entries.length === 0} onClick={() => setPending({ kind: 'all' })}>
            {t('trash.emptyButton')}
          </button>
        </div>
        {error && <p role="alert" className="error">{error}</p>}
        {entries.length === 0 ? (
          <p className="note">{t('trash.empty')}</p>
        ) : (
          <ul className="list">
            {entries.map((e) => (
              <li key={e.entryId}>
                <FolderIcon size={20} />
                <div className="text">
                  <span className="name">{e.name}</span>
                  <span className="meta">{t('trash.deleted', { when: new Date(e.deletedAt).toLocaleString(undefined, { dateStyle: 'medium', timeStyle: 'short' }) })}</span>
                </div>
                <button type="button" className="btn btn-primary trash-btn" disabled={busy} onClick={() => void act(async () => { await restoreProject(e.entryId); await refresh(); })}>
                  {t('trash.restore')}
                </button>
                <button type="button" className="btn btn-danger-outline trash-btn" disabled={busy} onClick={() => setPending({ kind: 'one', entry: e })}>
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
          message={pending.kind === 'one' ? t('trash.confirmDelete', { title: pending.entry.name }) : t('trash.confirmEmpty', { n: entries.length })}
          confirmLabel={t('common.delete')}
          onConfirm={confirm}
          onCancel={() => setPending(null)}
        />
      )}
    </div>
  );
}
