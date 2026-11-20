// SPDX-License-Identifier: MPL-2.0
import { t, type StringKey } from '../i18n/en';
import type { AppError } from './core';

const KNOWN = ['NotALibrary', 'TooNew', 'PermissionDenied', 'Locked', 'DiskFull', 'NotFound', 'Exists'];

function isAppError(e: unknown): e is AppError {
  return typeof e === 'object' && e !== null && typeof (e as AppError).code === 'string';
}

export function friendlyMessage(e: unknown): string {
  if (isAppError(e)) {
    if (KNOWN.includes(e.code)) return t(`error.${e.code}` as StringKey);
    return t('error.Other', { detail: String(e.detail ?? e.code) });
  }
  return t('error.Other', { detail: String(e) });
}
