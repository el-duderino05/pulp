// SPDX-License-Identifier: MPL-2.0
import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { createLibrary, openLibrary, type LibraryInfo } from '../app/core';
import { friendlyMessage } from '../app/errors';
import ErrorBanner from '../components/ErrorBanner';
import { t } from '../i18n/en';

export default function Setup({ defaultPath, onDone }: { defaultPath: string; onDone: (lib: LibraryInfo) => void }) {
  const [author, setAuthor] = useState('');
  const [path, setPath] = useState(defaultPath);
  const [error, setError] = useState<string | null>(null);
  const [exists, setExists] = useState(false);
  const [busy, setBusy] = useState(false);

  // One action at a time; a failure shows a friendly message.
  const act = async (fn: () => Promise<void>) => {
    if (busy) return;
    setBusy(true);
    setError(null);
    setExists(false);
    try {
      await fn();
    } catch (e) {
      const already = typeof e === 'object' && e !== null && (e as { code?: string }).code === 'LibraryExists';
      setExists(already);
      setError(already ? t('setup.exists') : friendlyMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const choose = () => act(async () => {
    const dir = await open({ directory: true });
    if (typeof dir === 'string') setPath(dir);
  });
  const create = () => act(async () => onDone(await createLibrary(path, author.trim())));
  const useExisting = (dir?: string) => act(async () => {
    const picked = dir ?? (await open({ directory: true }));
    if (typeof picked === 'string') onDone(await openLibrary(picked));
  });

  return (
    <main className="center-screen">
      <form className="card" onSubmit={(e) => { e.preventDefault(); void create(); }}>
        <div className="field">
          <h1>{t('setup.title')}</h1>
          <p>{t('setup.intro')}</p>
        </div>
        <div className="field">
          <label htmlFor="author">{t('setup.name')}</label>
          <input id="author" type="text" value={author} onChange={(e) => setAuthor(e.target.value)} />
          <p className="help">{t('setup.nameHelp')}</p>
        </div>
        <div className="field">
          <label htmlFor="where">{t('setup.where')}</label>
          <div className="row">
            <input id="where" type="text" value={path} onChange={(e) => setPath(e.target.value)} />
            <button type="button" className="btn" disabled={busy} onClick={() => void choose()}>{t('setup.choose')}</button>
          </div>
          <p className="help">{t('setup.whereHelp')}</p>
        </div>
        {error && <ErrorBanner message={error} onDismiss={() => { setError(null); setExists(false); }} />}
        {exists && (
          <div className="actions">
            <button type="button" className="btn" disabled={busy} onClick={() => void useExisting(path)}>{t('setup.openIt')}</button>
          </div>
        )}
        <div className="actions">
          <button type="button" className="btn" disabled={busy} onClick={() => void useExisting()}>{t('setup.useExisting')}</button>
          <button type="submit" className="btn btn-primary" disabled={busy || !author.trim() || !path.trim()}>{t('setup.create')}</button>
        </div>
      </form>
    </main>
  );
}
