// SPDX-License-Identifier: MPL-2.0
import { useCallback, useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  addExternalProject, createProject, forgetExternalProject, listProjects, openProject, trashProject,
  type LibraryInfo, type OpenedProject, type ProjectRow,
} from '../app/core';
import { registerCommand, runCommand, setErrorSink } from '../app/commands';
import { friendlyMessage } from '../app/errors';
import { formatWhen } from '../app/format';
import ErrorBanner from '../components/ErrorBanner';
import { t } from '../i18n/en';
import Notice from '../components/Notice';
import ContextMenu from '../components/ContextMenu';
import { FolderIcon, LockIcon, PlusIcon, TrashIcon } from '../components/Icons';
import LibraryTrash from './LibraryTrash';

// Join with the separator the root already uses, so POSIX roots don't get a backslash.
export const projectsPath = (root: string) => {
  const sep = root.includes('\\') && !root.includes('/') ? '\\' : '/';
  return root.replace(/[\\/]+$/, '') + sep + 'Projects';
};

export default function Library({ onOpen, info }: { onOpen: (p: OpenedProject) => void; info?: LibraryInfo }) {
  const [rows, setRows] = useState<ProjectRow[]>([]);
  const [view, setView] = useState<'list' | 'trash'>('list');
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState('');
  const [menu, setMenu] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Runs one action at a time; a failure shows a friendly message and leaves the list as it was.
  const act = useCallback(async (fn: () => Promise<void>) => {
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
  }, [busy]);

  const run = (id: string) => runCommand(id).catch((e) => setError(friendlyMessage(e)));
  const refresh = useCallback(async () => setRows(await listProjects()), []);

  useEffect(() => {
    if (view === 'list') void act(refresh);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view]);

  const folder = () => act(async () => {
    const dir = await open({ directory: true });
    if (typeof dir !== 'string') return;
    await addExternalProject(dir);
    await refresh();
  });

  useEffect(() => {
    if (view !== 'list') return;
    return setErrorSink((e) => setError(friendlyMessage(e)));
  }, [view]);

  useEffect(() => {
    const offs = [
      registerCommand({ id: 'library.newProject', label: 'library.newProject', run: () => setNaming(true) }),
      registerCommand({ id: 'library.openFolder', label: 'library.openFolder', run: folder }),
      registerCommand({ id: 'library.showTrash', label: 'library.trash', run: () => setView('trash') }),
    ];
    return () => offs.forEach((off) => off());
  });

  if (view === 'trash') return <LibraryTrash onBack={() => setView('list')} />;

  const create = () => act(async () => {
    const p = await createProject(name);
    onOpen(p);
  });
  const openRow = (r: ProjectRow) => {
    if (r.tooNew) return setError(t('error.TooNew'));
    void act(async () => onOpen(await openProject(r.path)));
  };

  return (
    <div className="page">
      <header className="page-head">
        <div>
          <span className="brand">Pulp</span>
          <span className="sub">
            {t('library.heading')}{info && ` · ${info.root}${info.sync ? ` · ${t('library.inSync', { sync: info.sync })}` : ''}`}
          </span>
        </div>
        <div className="row">
          <button type="button" className="btn btn-white" onClick={() => run('library.showTrash')}><TrashIcon />{t('library.trash')}</button>
          <button type="button" className="btn btn-white" disabled={busy} onClick={() => run('library.openFolder')}>{t('library.openFolder')}</button>
          <button type="button" className="btn btn-primary" onClick={() => run('library.newProject')}><PlusIcon />{t('library.newProject')}</button>
        </div>
      </header>
      <main className="page-body">
        <h1>{t('library.title')}</h1>
        {naming && (
          <form className="row" onSubmit={(e) => { e.preventDefault(); if (name.trim()) void create(); }}>
            <label htmlFor="project-name" className="sr-only">{t('library.projectName')}</label>
            <input id="project-name" type="text" autoFocus placeholder={t('library.projectName')} value={name} onChange={(e) => setName(e.target.value)} />
            <button type="submit" className="btn btn-primary" disabled={busy || !name.trim()}>{t('library.create')}</button>
            <button type="button" className="btn" onClick={() => { setNaming(false); setName(''); }}>{t('common.cancel')}</button>
          </form>
        )}
        {info && info.recreated.length > 0 && (
          <Notice report={{ found: [], missing: [], duplicates: [], copies: [], conflicts: [], recreated: info.recreated }} />
        )}
        {error && <ErrorBanner message={error} onDismiss={() => setError(null)} />}
        <ul className="list">
          {rows.map((r) => (
            <li key={r.path} className={r.missing || r.unreadable ? 'unavailable' : undefined}>
              {r.tooNew ? <LockIcon size={22} /> : <FolderIcon size={22} />}
              <div className="text">
                {r.missing || r.unreadable ? (
                  <>
                    <span className="name">{r.name}</span>
                    <span className="meta">{t(r.missing ? 'library.cantFind' : 'library.cantRead', { path: r.path })}</span>
                  </>
                ) : (
                  <>
                    <button type="button" className="link" disabled={busy} onClick={() => openRow(r)}>{r.name}</button>
                    {r.lastOpened && <span className="meta">{t('library.opened', { when: formatWhen(r.lastOpened) })}</span>}
                  </>
                )}
              </div>
              {r.tooNew ? (
                <button type="button" className="badge" onClick={() => openRow(r)}>{t('library.readOnly')}</button>
              ) : (
                <button type="button" className="more" aria-label={t('library.moreActions', { name: r.name })} onClick={() => setMenu(r.path)}>⋯</button>
              )}
              {menu === r.path && (
                <ContextMenu
                  onClose={() => setMenu(null)}
                  items={[
                    r.external
                      ? { label: t('library.forget'), onSelect: () => void act(async () => { await forgetExternalProject(r.path); await refresh(); }) }
                      : { label: t('command.trash'), onSelect: () => void act(async () => { await trashProject(r.path); await refresh(); }) },
                  ]}
                />
              )}
            </li>
          ))}
        </ul>
        {info && <p className="note">{t('library.footer', { path: projectsPath(info.root) })}</p>}
      </main>
    </div>
  );
}
