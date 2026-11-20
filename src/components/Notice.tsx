// SPDX-License-Identifier: MPL-2.0
import { useState } from 'react';
import type { ReportView } from '../app/core';
import { t, type StringKey } from '../i18n/en';

const KEYS: [keyof ReportView, StringKey][] = [
  ['found', 'report.found'],
  ['missing', 'report.missing'],
  ['duplicates', 'report.duplicates'],
  ['copies', 'report.copies'],
  ['conflicts', 'report.conflicts'],
  ['recreated', 'report.recreated'],
];

// What Pulp tidied when it opened the project; shown only when something was.
export default function Notice({ report }: { report: ReportView }) {
  const [open, setOpen] = useState(false);
  const [hidden, setHidden] = useState(false);
  const lists = KEYS.filter(([k]) => report[k].length > 0);
  if (hidden || lists.length === 0) return null;
  return (
    <div role="status" className="notice">
      <div className="notice-text">
        <span>{lists.map(([k, key]) => t(key, { n: report[k].length, list: report[k].join(', ') })).join(' ')}</span>
        {open && (
          <ul>
            {lists.flatMap(([k]) => report[k].map((item, i) => <li key={`${k}${i}`}>{item}</li>))}
          </ul>
        )}
      </div>
      <button type="button" className="notice-btn" aria-expanded={open} onClick={() => setOpen(!open)}>{t('notice.details')}</button>
      <button type="button" className="notice-x" aria-label={t('notice.dismiss')} onClick={() => setHidden(true)}>×</button>
    </div>
  );
}
