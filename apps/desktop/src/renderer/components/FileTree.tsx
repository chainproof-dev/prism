// FileTree (parity TRE rows): virtualized tree table via TanStack Virtual,
// engine-side paging, % bars, badges, selection sync.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { ChevronRight, Link2, AlertTriangle, Package } from 'lucide-react';
import type { NodeRow } from '@prism/shared/generated';
import { formatBytes, formatPercent } from '@prism/shared/client';
import { children as fetchChildren } from '../lib/prism';
import { useScanStore } from '../stores/scan';
import { resolvePaletteIndex, dataPalette } from '../lib/palette';

interface Expanded {
  [nodeId: number]: boolean;
}

interface FlatRow {
  node: NodeRow;
  depth: number;
}

export function FileTree(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const select = useScanStore((s) => s.select);
  const selected = useScanStore((s) => s.selectedNode);
  const [expanded, setExpanded] = useState<Expanded>({ 0: true });
  const [rowsByParent, setRowsByParent] = useState<Record<number, NodeRow[]>>({});
  const parentRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (scanId === null) {
      return;
    }
    let cancelled = false;
    const load = async (parent: number): Promise<void> => {
      try {
        const page = await fetchChildren(scanId, parent, 0, 500);
        if (!cancelled) {
          setRowsByParent((prev) => ({ ...prev, [parent]: page.items }));
        }
      } catch {
        // engine busy during walk → rows stream in via deltas (ADR-03)
      }
    };
    void load(0);
    return () => {
      cancelled = true;
    };
  }, [scanId]);

  const toggle = useCallback(
    (node: NodeRow) => {
      setExpanded((prev) => {
        const next = { ...prev, [node.id]: !prev[node.id] };
        if (next[node.id] && scanId !== null) {
          void fetchChildren(scanId, node.id, 0, 500)
            .then((page) => setRowsByParent((prev2) => ({ ...prev2, [node.id]: page.items })))
            .catch(() => undefined);
        }
        return next;
      });
    },
    [scanId],
  );

  // flatten visible rows
  const flat: FlatRow[] = useMemo(() => {
    const out: FlatRow[] = [];
    const walk = (parent: number, depth: number): void => {
      const kids = rowsByParent[parent] ?? [];
      for (const node of kids) {
        out.push({ node, depth });
        const isDir = node.kind === 'dir' || node.kind === 'reparse';
        if (isDir && expanded[node.id]) {
          walk(node.id, depth + 1);
        }
      }
    };
    walk(0, 0);
    return out;
  }, [rowsByParent, expanded]);

  const virtualizer = useVirtualizer({
    count: flat.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 28,
    overscan: 16,
  });

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="micro-label shrink-0 border-b border-hairline px-3 py-2">Directory tree</div>
      <div className="grid shrink-0 grid-cols-[1fr_56px_84px_84px] border-b border-hairline px-3 py-1 text-[10px] uppercase tracking-wider text-text-muted">
        <span>Name</span>
        <span className="text-right">%</span>
        <span className="text-right">Size</span>
        <span className="text-right">Items</span>
      </div>
      <div ref={parentRef} className="min-h-0 flex-1 overflow-y-auto">
        <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
          {virtualizer.getVirtualItems().map((vi) => {
            const { node, depth } = flat[vi.index]!;
            const isDir = node.kind === 'dir' || node.kind === 'reparse' || node.kind === 'mount';
            const isSelected = selected === node.id;
            const palette = dataPalette((document.documentElement.dataset.theme as string | undefined) ?? 'nocturne');
            const swatch = node.kind === 'file' ? resolvePaletteIndex(node.category | 0x10000, palette, false, false) : 'transparent';
            return (
              <div
                key={node.id}
                data-testid={`tree-row-${node.id}`}
                className={`absolute left-0 right-0 flex h-7 cursor-default items-center gap-2 border-l-2 pr-2 text-xs ${
                  isSelected ? 'border-accent bg-accent-subtle' : 'border-transparent hover:bg-surface-hover'
                }`}
                style={{
                  top: 0,
                  transform: `translateY(${vi.start}px)`,
                  paddingLeft: 12 + depth * 16,
                }}
                onClick={() => select(node.id)}
                onDoubleClick={() => (isDir ? toggle(node) : undefined)}
              >
                <span className="flex min-w-0 flex-1 items-center gap-1">
                  {isDir ? (
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        toggle(node);
                      }}
                      className="text-text-muted hover:text-text-primary"
                    >
                      <ChevronRight size={12} className={`transition-transform ${expanded[node.id] ? 'rotate-90' : ''}`} />
                    </button>
                  ) : (
                    <span className="inline-block w-3" />
                  )}
                  {node.kind === 'file' ? <span className="h-2 w-2 shrink-0 rounded-[2px]" style={{ background: swatch }} /> : null}
                  {node.badges.includes('junction') ? <Link2 size={11} className="shrink-0 text-text-muted" /> : null}
                  {node.badges.includes('denied') ? <AlertTriangle size={11} className="shrink-0 text-warning" /> : null}
                  {node.badges.includes('package') ? <Package size={11} className="shrink-0 text-text-muted" /> : null}
                  <span className={`truncate ${node.kind === 'file' ? 'text-text-secondary' : 'text-text-primary'}`}>{node.name}</span>
                </span>
                <span className="flex w-14 items-center gap-1.5">
                  <span className="h-[3px] flex-1 rounded-[2px] bg-surface-hover">
                    <span className="block h-full rounded-[2px]" style={{ width: `${Math.min(node.parentShare * 100, 100)}%`, background: swatch === 'transparent' ? 'var(--text-faint)' : swatch }} />
                  </span>
                  <span className="mono w-8 text-right text-text-faint">{formatPercent(node.parentShare)}</span>
                </span>
                <span className="mono w-20 text-right text-text-secondary">{formatBytes(node.allocated)}</span>
                <span className="mono w-20 text-right text-text-faint">{node.files > 0 ? `${node.files.toLocaleString('en-US')}` : ''}</span>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
