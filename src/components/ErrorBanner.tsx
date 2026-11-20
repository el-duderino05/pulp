// SPDX-License-Identifier: MPL-2.0
import { t } from '../i18n/en';

export default function ErrorBanner({ message, onDismiss }: { message: string; onDismiss: () => void }) {
  return (
    <div className="error-banner">
      <span role="alert" className="error-banner-text">{message}</span>
      <button type="button" className="notice-x" aria-label={t('notice.dismiss')} onClick={onDismiss}>×</button>
    </div>
  );
}
