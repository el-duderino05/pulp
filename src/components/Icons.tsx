// SPDX-License-Identifier: MPL-2.0
import type { ReactNode } from 'react';

const Svg = ({ size, color, children, width = 1.3 }: { size: number; color: string; children: ReactNode; width?: number }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="none" stroke={color} strokeWidth={width} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
    {children}
  </svg>
);

type P = { size?: number; color?: string };

export const FolderIcon = ({ size = 16, color = 'var(--accent)' }: P) => (
  <Svg size={size} color={color}><path d="M2 3.5h4l1.5 1.5H14v8H2z" /></Svg>
);
export const DocumentIcon = ({ size = 16, color = 'var(--muted)' }: P) => (
  <Svg size={size} color={color}><path d="M4 1.5h5.5L12 4v10.5H4z M6 7h4M6 9.5h4" /></Svg>
);
export const LockIcon = ({ size = 16, color = 'var(--muted)' }: P) => (
  <Svg size={size} color={color}><rect x="3" y="7" width="10" height="7" rx="1" /><path d="M5.5 7V5a2.5 2.5 0 0 1 5 0v2" /></Svg>
);
export const TrashIcon = ({ size = 16, color = 'currentColor' }: P) => (
  <Svg size={size} color={color}><path d="M3 4h10M6 4V2.5h4V4M4.5 4l.7 9.5h5.6l.7-9.5" /></Svg>
);
export const PlusIcon = ({ size = 16, color = 'currentColor' }: P) => (
  <Svg size={size} color={color} width={1.8}><path d="M8 3v10M3 8h10" /></Svg>
);
