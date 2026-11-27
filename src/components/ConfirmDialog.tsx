// SPDX-License-Identifier: MPL-2.0
import { t } from '../i18n/en';

export default function ConfirmDialog(p: {
  title: string;
  message: string;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="scrim" onKeyDown={(e) => { if (e.key === 'Escape') p.onCancel(); }}>
      <div role="dialog" aria-modal="true" aria-labelledby="confirm-title" className="card dialog">
        <h2 id="confirm-title">{p.title}</h2>
        <p>{p.message}</p>
        <div className="actions">
          <button type="button" className="btn" autoFocus onClick={p.onCancel}>{t('common.cancel')}</button>
          <button type="button" className={`btn btn-primary${p.danger ? ' btn-danger' : ''}`} onClick={p.onConfirm}>{p.confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}
