// SPDX-License-Identifier: MPL-2.0
import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { openLibrary, type LibraryInfo } from '../app/core';
import { friendlyMessage } from '../app/errors';
import { t } from '../i18n/en';

// `message` replaces the default "library missing" text (used for startup failures).
export default function LocateLibrary({ path, message, onDone }: { path: string; message?: string; onDone: (lib: LibraryInfo) => void }) {
  const [error, setError] = useState<string | null>(null);

  const locate = async () => {
    try {
      setError(null);
      const dir = await open({ directory: true });
      if (typeof dir === 'string') onDone(await openLibrary(dir));
    } catch (e) {
      setError(friendlyMessage(e));
    }
  };

  return (
    <main className="center-screen">
      <div className="card">
        <h1>{t('locate.title')}</h1>
        <p>{message ?? t('library.missing', { path })}</p>
        {error && <p role="alert" className="error">{error}</p>}
        <div className="actions">
          <button type="button" className="btn btn-primary" onClick={() => void locate()}>{t('locate.button')}</button>
        </div>
      </div>
    </main>
  );
}
