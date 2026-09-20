// Inspector (docs/09 § Inspector): selection-bound context panel with
// metrics, attributes, Largest Inside, actions.
import { useEffect, useState } from 'react';
import { Copy, ExternalLink, FolderOpen, Trash2 } from 'lucide-react';
import type { NodeDetail, NodeRow } from '@prism/shared/generated';
import { formatBytes, formatPercent } from '@prism/shared/client';
import { nodeDetail, children } from '../lib/prism';
import { useScanStore } from '../stores/scan';

export function Inspector(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const selected = useScanStore((s) => s.selectedNode);
  const select = useScanStore((s) => s.select);
  const [detail, setDetail] = useState<NodeDetail | null>(null);
  const [largest, setLargest] = useState<NodeRow[]>([]);

  useEffect(() => {
    if (scanId === null || selected === null) {
      setDetail(null);
      setLargest([]);
      return;
    }
    let cancelled = false;
    void nodeDetail(scanId, selected)
      .then((d) => {
        if (!cancelled) {
          setDetail(d);
        }
      })
      .catch(() => undefined);
    void children(scanId, selected, 0, 10)
      .then((page) => {
        if (!cancelled) {
          setLargest(page.items);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [scanId, selected]);

  if (selected === null) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center">
        <div className="text-sm text-text-secondary">Select something to inspect it.</div>
        <div className="text-xs text-text-faint">Click any tile, row, or type.</div>
      </div>
    );
  }
  if (!detail) {
    return <div className="p-4 text-xs text-text-muted">Loading…</div>;
  }
  const parentShare = detail.parent !== detail.id && largest.length > 0 ? undefined : undefined;
  void parentShare;
  return (
    <div className="flex h-full flex-col overflow-y-auto">
      <header className="border-b border-hairline p-3">
        <div className="truncate text-sm font-medium text-text-primary">{detail.name}</div>
        <div className="mono mt-0.5 truncate text-xs text-text-muted" title={detail.path}>{detail.path}</div>
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          <span className="rounded-xs border border-hairline px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-text-secondary">{detail.kind}</span>
          {detail.extension ? <span className="mono rounded-xs bg-surface-raised px-1.5 py-0.5 text-[10px] text-text-secondary">{detail.extension}</span> : null}
          <span className="rounded-xs border border-hairline px-1.5 py-0.5 text-[10px] text-text-muted">{detail.categoryName}</span>
        </div>
      </header>

      <section className="grid grid-cols-3 gap-px border-b border-hairline bg-hairline">
        <Metric label="Logical" value={formatBytes(detail.logical)} />
        <Metric label="Allocated" value={formatBytes(detail.allocated)} />
        <Metric label="Unique" value={formatBytes(detail.unique)} />
      </section>

      <section className="border-b border-hairline px-3 py-2">
        <div className="micro-label mb-1.5">Attributes</div>
        <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
          <dt className="text-text-muted">Modified</dt>
          <dd className="mono text-text-secondary">{detail.modified ? new Date(Number(detail.modified)).toLocaleString() : '—'}</dd>
          <dt className="text-text-muted">Files</dt>
          <dd className="mono text-text-secondary">{detail.files.toLocaleString('en-US')}</dd>
          <dt className="text-text-muted">Folders</dt>
          <dd className="mono text-text-secondary">{detail.folders.toLocaleString('en-US')}</dd>
          <dt className="text-text-muted">Depth</dt>
          <dd className="mono text-text-secondary">{detail.depth}</dd>
        </dl>
      </section>

      <section className="border-b border-hairline px-3 py-2">
        <div className="micro-label mb-1.5">Largest inside</div>
        <ul className="flex flex-col gap-0.5">
          {largest.map((n) => (
            <li key={n.id}>
              <button
                onClick={() => select(n.id)}
                className="flex w-full items-center gap-2 rounded-xs px-1 py-0.5 text-left text-xs hover:bg-surface-hover"
              >
                <span className="truncate text-text-secondary">{n.name}</span>
                <span className="mono ml-auto shrink-0 text-text-faint">{formatBytes(n.allocated)}</span>
              </button>
            </li>
          ))}
          {largest.length === 0 ? <li className="text-xs text-text-faint">No children</li> : null}
        </ul>
      </section>

      <section className="flex flex-col gap-1 p-3">
        <button className="flex items-center gap-2 rounded-sm border border-hairline px-2.5 py-1.5 text-xs hover:border-border-strong">
          <FolderOpen size={13} /> Reveal in Explorer
        </button>
        <button
          className="flex items-center gap-2 rounded-sm border border-hairline px-2.5 py-1.5 text-xs hover:border-border-strong"
          onClick={() => void navigator.clipboard.writeText(detail.path)}
        >
          <Copy size={13} /> Copy path
        </button>
        <button className="flex items-center gap-2 rounded-sm border border-hairline px-2.5 py-1.5 text-xs text-danger hover:border-danger">
          <Trash2 size={13} /> Delete to recycle bin
        </button>
      </section>
      <div className="mt-auto px-3 pb-3 text-[10px] text-text-faint">
        {formatPercent(largest[0]?.parentShare ?? 0)} of parent
      </div>
    </div>
  );
}

function Metric({ label, value }: { label: string; value: string }): React.ReactElement {
  return (
    <div className="bg-surface-panel p-3">
      <div className="micro-label">{label}</div>
      <div className="mono mt-1 text-sm text-text-primary">{value}</div>
    </div>
  );
}

export { ExternalLink };
