// TypeList (parity-EXT-01..05): extension stats with swatches, counts, sizes,
// % bars; multi-select filters all views via dimming.
import { useEffect, useState } from 'react';
import { typesList } from '../lib/prism';
import type { TypeRow } from '@prism/shared/generated';
import { formatBytes } from '@prism/shared/client';
import { resolvePaletteIndex, dataPalette } from '../lib/palette';
import { useScanStore } from '../stores/scan';

export function TypeList({ onFilter }: { onFilter?: (categories: string[]) => void }): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const [rows, setRows] = useState<TypeRow[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (scanId === null) {
      return;
    }
    let cancelled = false;
    void typesList(scanId)
      .then((page) => {
        if (!cancelled) {
          setRows(page.items.slice(0, 60));
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [scanId]);

  const toggle = (key: string): void => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      onFilter?.([...next]);
      return next;
    });
  };

  const palette = dataPalette((document.documentElement.dataset.theme as string | undefined) ?? 'nocturne');

  return (
    <section className="border-b border-hairline">
      <div className="micro-label border-b border-hairline px-3 py-2">Types</div>
      <ul className="max-h-[360px] overflow-y-auto px-1.5 py-1">
        {rows.map((row) => {
          const color = resolvePaletteIndex(row.category | 0x10000, palette, false, false);
          const isSelected = selected.has(row.key);
          return (
            <li key={row.key}>
              <button
                onClick={() => toggle(row.key)}
                className={`flex w-full items-center gap-2 rounded-xs px-1.5 py-1 text-left text-xs hover:bg-surface-hover ${isSelected ? 'bg-accent-subtle' : ''}`}
              >
                <span className="h-2.5 w-2.5 shrink-0 rounded-[2px]" style={{ background: color }} />
                <span className="mono w-16 shrink-0 truncate text-text-secondary">{row.key || '—'}</span>
                <span className="mono w-14 shrink-0 text-right text-text-faint">{row.files.toLocaleString('en-US')}</span>
                <span className="mono w-20 shrink-0 text-right text-text-secondary">{formatBytes(row.allocated)}</span>
                <span className="h-[3px] flex-1 rounded-[2px] bg-surface-hover">
                  <span className="block h-full rounded-[2px]" style={{ width: `${Math.min(row.share * 100 / (rows[0]?.share || 1), 100)}%`, background: color }} />
                </span>
              </button>
            </li>
          );
        })}
        {rows.length === 0 ? <li className="px-2 py-2 text-xs text-text-faint">No types yet</li> : null}
      </ul>
    </section>
  );
}
