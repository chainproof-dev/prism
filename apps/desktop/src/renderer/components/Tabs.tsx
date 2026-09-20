// Monitor (PRISM-HG-060, docs/10 § 8): 1 Hz samples via monitor:sample
// events; system strip + top processes. Informational only (no kill).
// Snapshots (PRISM-HG-050, docs/10 § 9): capture, list, guided diff.
// Applications + Leftovers (PRISM-HG-040, docs/10 § 7).
import { useEffect, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useScanStore } from '../stores/scan';
import { useLicenseStore } from '../stores/ui';
import { t } from '../lib/i18n';
import {
  monitorStart, monitorStop, onEngineEvents,
  snapshotsSave, snapshotsList, snapshotsDiff,
  appsList, appFootprint, leftovers,
} from '../lib/prism';
import type {
  MonitorSample, SnapshotInfo, AppRow, FootprintRoot,
} from '@prism/shared/generated';
import { formatBytes } from '@prism/shared/client';
import { Empty } from './Duplicates';

// ---------------------------------------------------------------- Monitor

export function MonitorTab(): React.ReactElement {
  const [samples, setSamples] = useState<MonitorSample[]>([]);
  const [sortBy, setSortBy] = useState<'cpu' | 'io' | 'mem'>('cpu');
  const phase = useLicenseStore((s) => s.phase);

  useEffect(() => {
    if (phase === 'unlicensed' || phase === 'expired') return;
    void monitorStart(1000);
    const off = onEngineEvents((ev) => {
      if (ev.ev === 'monitor-sample') {
        const s = (ev as unknown as { sample: MonitorSample }).sample;
        setSamples((prev) => [...prev.slice(-59), s]);
      }
    });
    return () => {
      off();
      void monitorStop();
    };
  }, [phase]);

  const latest = samples.at(-1);
  const procs = [...(latest?.processes ?? [])].sort((a, b) => {
    if (sortBy === 'io') return Number(b.readBps + b.writeBps - (a.readBps + a.writeBps));
    if (sortBy === 'mem') return Number(b.workingSet - a.workingSet);
    return b.cpu - a.cpu;
  }).slice(0, 20);

  return (
    <div className="flex h-full flex-col bg-surface-app">
      <div className="flex items-center gap-6 border-b border-hairline bg-surface-panel px-4 py-3 text-xs">
        <Stat label={t('monitor.cpu')} value={`${Math.round((latest?.cpuTotal ?? 0) * 100)}%`} spark={samples.map((s) => s.cpuTotal)} />
        <Stat
          label={t('monitor.memory')}
          value={`${formatBytes(latest?.memUsed ?? 0n)} / ${formatBytes(latest?.memTotal ?? 0n)}`}
          spark={samples.map((s) => (s.memTotal > 0n ? Number(s.memUsed / s.memTotal) : 0))}
        />
        <Stat
          label={t('monitor.disk')}
          value={`↓${formatBytes(latest?.diskReadBps ?? 0n)}/s ↑${formatBytes(latest?.diskWriteBps ?? 0n)}/s`}
          spark={samples.map((s) => Number(s.diskReadBps + s.diskWriteBps) / (100 * 1024 * 1024))}
        />
      </div>
      <div className="flex items-center gap-2 border-b border-hairline px-4 py-2 text-[11px]">
        <span className="text-text-faint">top by</span>
        {(['cpu', 'io', 'mem'] as const).map((k) => (
          <button
            key={k}
            type="button"
            onClick={() => setSortBy(k)}
            className={`rounded px-2 py-0.5 ${sortBy === k ? 'bg-accent/20 text-accent' : 'text-text-secondary hover:bg-surface-hover'}`}
          >
            {k}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-surface-panel text-left text-[11px] uppercase tracking-wide text-text-faint">
            <tr>
              <th className="px-4 py-2 font-medium">process</th>
              <th className="px-2 py-2 font-medium">pid</th>
              <th className="px-2 py-2 text-right font-medium">cpu</th>
              <th className="px-2 py-2 text-right font-medium">read/s</th>
              <th className="px-2 py-2 text-right font-medium">write/s</th>
              <th className="px-4 py-2 text-right font-medium">working set</th>
            </tr>
          </thead>
          <tbody>
            {procs.map((p) => (
              <tr key={p.pid} className="border-b border-hairline/50 hover:bg-surface-hover">
                <td className="px-4 py-1.5 text-text-primary">{p.name}</td>
                <td className="mono px-2 py-1.5 text-text-faint">{p.pid}</td>
                <td className="mono px-2 py-1.5 text-right text-text-secondary">{(p.cpu * 100).toFixed(1)}%</td>
                <td className="mono px-2 py-1.5 text-right text-text-secondary">{formatBytes(p.readBps)}</td>
                <td className="mono px-2 py-1.5 text-right text-text-secondary">{formatBytes(p.writeBps)}</td>
                <td className="mono px-4 py-1.5 text-right text-text-secondary">{formatBytes(p.workingSet)}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {samples.length === 0 && <Empty title="Sampling…" />}
      </div>
    </div>
  );
}

function Stat({ label, value, spark }: { label: string; value: string; spark: number[] }): React.ReactElement {
  const w = 120, h = 28;
  const pts = spark.length > 1
    ? spark.map((v, i) => `${(i / (spark.length - 1)) * w},${h - Math.min(v, 1) * h}`).join(' ')
    : '';
  return (
    <div className="flex items-center gap-2">
      <div>
        <div className="text-[10px] uppercase tracking-wide text-text-faint">{label}</div>
        <div className="mono text-text-primary">{value}</div>
      </div>
      {pts && (
        <svg width={w} height={h} className="overflow-visible">
          <polyline points={pts} fill="none" stroke="var(--data-1, #5aa9e6)" strokeWidth="1.5" />
        </svg>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- Snapshots

export function SnapshotsTab(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const [snaps, setSnaps] = useState<SnapshotInfo[]>([]);
  const [pick, setPick] = useState<{ before?: number; after?: number }>({});
  const [diff, setDiff] = useState<import('@prism/shared/generated').SnapshotDiffPage | null>(null);
  const [showSmall, setShowSmall] = useState(false);

  const reload = (): void => {
    void snapshotsList().then(setSnaps).catch(() => setSnaps([]));
  };
  useEffect(reload, []);

  useEffect(() => {
    if (pick.before !== undefined && pick.after !== undefined) {
      void snapshotsDiff(pick.before, pick.after, showSmall ? 1 : 10 * 1024 * 1024)
        .then(setDiff)
        .catch(() => setDiff(null));
    }
  }, [pick, showSmall]);

  const capture = (): void => {
    if (scanId === null) return;
    void snapshotsSave(scanId)
      .then(() => {
        toast.success(t('snapshots.capture'));
        reload();
      })
      .catch(() => toast.error('capture failed (premium feature — licensed?)'));
  };

  return (
    <div className="flex h-full flex-col bg-surface-app">
      <div className="flex items-center justify-between border-b border-hairline bg-surface-panel px-4 py-3">
        <div className="text-sm font-medium text-text-primary">{t('snapshots.title')}</div>
        <button
          type="button"
          disabled={scanId === null}
          onClick={capture}
          className="rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-on-accent disabled:opacity-40"
        >
          {t('snapshots.capture')}
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {snaps.length === 0 && <Empty title={t('snapshots.pickHint')} />}
        {snaps.map((s) => {
          const isBefore = pick.before === s.id;
          const isAfter = pick.after === s.id;
          return (
            <button
              key={s.id}
              type="button"
              onClick={() => {
                if (pick.before === undefined || (pick.before !== undefined && pick.after !== undefined)) {
                  setPick({ before: s.id });
                  setDiff(null);
                } else if (pick.before !== s.id) {
                  setPick({ ...pick, after: s.id });
                }
              }}
              className={`flex w-full items-center justify-between border-b border-hairline px-4 py-2 text-left text-xs hover:bg-surface-hover ${
                isBefore ? 'bg-accent/10' : isAfter ? 'bg-success/10' : ''
              }`}
            >
              <span className="mono truncate text-text-secondary">{s.rootPath}</span>
              <span className="ml-3 flex shrink-0 items-center gap-3 text-text-faint">
                {isBefore && <b className="text-accent">{t('snapshots.before')}</b>}
                {isAfter && <b className="text-success">{t('snapshots.after')}</b>}
                <span>{new Date(Number(s.createdAt)).toLocaleString()}</span>
                <span>{Number(s.files).toLocaleString('en-US')} files</span>
                <span>{formatBytes(s.bytes)}</span>
              </span>
            </button>
          );
        })}

        {diff && (
          <div className="p-4">
            <div className="mb-3 flex items-center justify-between">
              <div className="text-xs text-text-secondary">
                {t('snapshots.netChange')}: <b className={diff.net >= 0n ? 'text-danger' : 'text-success'}>{diff.net >= 0n ? '+' : '−'}{formatBytes(BigInt(Math.abs(Number(diff.net))))}</b>
              </div>
              <label className="flex items-center gap-1.5 text-[11px] text-text-faint">
                <input type="checkbox" checked={showSmall} onChange={(e) => setShowSmall(e.target.checked)} />
                {t('snapshots.showSmaller')}
              </label>
            </div>
            {diff.deltas.map((d) => (
              <div key={d.pathKey} className="flex items-center justify-between border-b border-hairline/50 py-1.5 text-xs">
                <span className="mono truncate text-text-secondary">{d.path}</span>
                <span className={`ml-3 shrink-0 font-medium ${d.deltaBytes >= 0 ? 'text-danger' : 'text-success'}`}>
                  {d.kind === 'added' ? t('snapshots.added') : d.kind === 'removed' ? t('snapshots.removed') : d.kind === 'grew' ? t('snapshots.grew') : t('snapshots.shrank')}
                  {' '}
                  {d.deltaBytes >= 0n ? '+' : '−'}
                  {formatBytes(BigInt(Math.abs(Number(d.deltaBytes))))}
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- Applications / Leftovers

export function ApplicationsTab(): React.ReactElement {
  const [apps, setApps] = useState<AppRow[]>([]);
  const [sel, setSel] = useState<string | null>(null);
  const [footprint, setFootprint] = useState<{ total: bigint; roots: FootprintRoot[] } | null>(null);
  const [includeSystem, setIncludeSystem] = useState(false);

  useEffect(() => {
    void appsList(includeSystem).then(setApps).catch(() => setApps([]));
  }, [includeSystem]);

  useEffect(() => {
    if (sel) {
      void appFootprint(sel).then(setFootprint).catch(() => setFootprint(null));
    }
  }, [sel]);

  return (
    <div className="flex h-full bg-surface-app">
      <div className="flex w-[320px] shrink-0 flex-col border-r border-hairline">
        <div className="flex items-center justify-between border-b border-hairline bg-surface-panel px-3 py-2">
          <span className="text-sm font-medium">{t('apps.title')}</span>
          <label className="flex items-center gap-1 text-[11px] text-text-faint">
            <input type="checkbox" checked={includeSystem} onChange={(e) => setIncludeSystem(e.target.checked)} />
            system
          </label>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto">
          {apps.length === 0 && <Empty title={t('apps.empty')} />}
          {apps.map((a) => (
            <button
              key={a.token}
              type="button"
              onClick={() => setSel(a.token)}
              className={`block w-full border-b border-hairline/50 px-3 py-2 text-left hover:bg-surface-hover ${sel === a.token ? 'bg-surface-hover' : ''}`}
            >
              <div className="truncate text-xs text-text-primary">{a.name}</div>
              <div className="text-[11px] text-text-faint">
                {a.publisher} {a.bytes > 0 ? `· ${formatBytes(a.bytes)}` : ''}
              </div>
            </button>
          ))}
        </div>
      </div>
      <div className="min-w-0 flex-1 overflow-y-auto p-4">
        {footprint ? (
          <>
            <div className="text-sm font-medium text-text-primary">
              {formatBytes(footprint.total)} <span className="text-text-faint">total footprint (evidence below)</span>
            </div>
            <div className="mt-2 flex h-3 overflow-hidden rounded">
              {footprint.roots.map((r, i) => (
                <div
                  key={r.label}
                  title={`${r.label}: ${formatBytes(r.bytes)}`}
                  style={{ width: `${footprint.total > 0n ? Number((r.bytes * 100n) / footprint.total) : 0}%` }}
                  className={`h-full ${['bg-chart-1', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4', 'bg-chart-5'][i % 5]}`}
                />
              ))}
            </div>
            {footprint.roots.map((r) => (
              <div key={r.label} className="mt-3">
                <div className="text-xs font-medium text-text-secondary">
                  {r.label} · {formatBytes(r.bytes)}
                </div>
                {r.paths.map((p) => (
                  <div key={p} className="mono truncate py-0.5 text-[11px] text-text-faint">{p}</div>
                ))}
              </div>
            ))}
          </>
        ) : (
          <Empty title={t('apps.empty')} />
        )}
      </div>
    </div>
  );
}

export function LeftoversTab(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const [rows, setRows] = useState<AppRow[]>([]);
  useEffect(() => {
    if (scanId !== null) {
      void leftovers(scanId).then(setRows).catch(() => setRows([]));
    }
  }, [scanId]);
  return (
    <div className="h-full overflow-y-auto bg-surface-app">
      {rows.length === 0 ? (
        <Empty title={t('leftovers.empty')} />
      ) : (
        rows.map((r) => (
          <div key={r.token} className="flex items-center justify-between border-b border-hairline px-4 py-2 text-xs">
            <span className="text-text-primary">{r.name}</span>
            <span className="mono text-text-faint">{formatBytes(r.bytes)}</span>
          </div>
        ))
      )}
    </div>
  );
}

// keep useRef in the import surface (sparkline buffers may use it later)
void useRef;
