// SPDX-License-Identifier: MPL-2.0
import { useEffect, useRef } from 'react';

export type MenuItem = { label: string; onSelect: () => void; danger?: boolean };

// Rendered inside a `position: relative` parent; Escape or an outside click closes it.
export default function ContextMenu({ items, onClose }: { items: MenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLUListElement>(null);
  useEffect(() => {
    ref.current?.querySelector('button')?.focus();
    const away = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) onClose();
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [onClose]);
  return (
    <ul
      ref={ref}
      role="menu"
      className="menu"
      onKeyDown={(e) => {
        if (e.key === 'Escape') onClose();
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          const all = [...ref.current!.querySelectorAll('button')];
          const i = all.indexOf(document.activeElement as HTMLButtonElement);
          all[(i + (e.key === 'ArrowDown' ? 1 : all.length - 1)) % all.length]?.focus();
          e.preventDefault();
        }
      }}
    >
      {items.map((it) => (
        <li key={it.label} role="none">
          <button
            type="button"
            role="menuitem"
            className={it.danger ? 'danger' : undefined}
            onClick={() => {
              onClose();
              it.onSelect();
            }}
          >
            {it.label}
          </button>
        </li>
      ))}
    </ul>
  );
}
