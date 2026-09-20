// Welcome (docs/10 § 3, parity-SEL-01..06): drive cards with DonutGauge,
// folder scan, recents. Premium entrance stagger (docs/09 § 2.3).
import { useCallback, useEffect, useState } from 'react';
import { FolderOpen, HardDrive, RefreshCw, Usb, Network, Cpu } from 'lucide-react';
import { useScanStore } from '../stores/scan';
import { formatBytes, formatPercent } from '@prism/shared/client';
import type { VolumeInfo } from '@prism/shared/generated';

const RECENTS_KEY = 'prism.recents';

function loadRecents(): string[] {
  try {
    return JSON.parse(localStorage.getItem(RECENTS_KEY) ?? '[]') as string[];
  } catch {
    return [];
  }
}

function pushRecent(path: string): void {
  const recents = [path, ...loadRecents().filter((p) => p !== path)].slice(0, 10);
  localStorage.setItem(RECENTS_KEY, JSON.stringify(recents));
}

function volumeIcon(kind: VolumeInfo['kind']): React.ReactElement {
  switch (kind) {
    case 'removable':
      return <Usb size={20} strokeWidth={1.75} />;
    case 'network':
      return <Network size={20} strokeWidth={1.75} />;
    case 'ram':
      return <Cpu size={20} strokeWidth={1.75} />;
    default:
      return <HardDrive size={20} strokeWidth={1.75} />;
  }
}

function DonutGauge({ used, total }: { used: number | bigint; total: number | bigint }): React.ReactElement {
  const share = Number(total) > 0 ? Math.min(Number(used) / Number(total), 1) : 0;
  const r = 22;
  const circ = 2 * Math.PI * r;
  return (
    <svg width={56} height={56} viewBox="0 0 56 56" aria-hidden>
      <circle cx={28} cy={28} r={r} fill="none" stroke="var(--border-hairline)" strokeWidth={5} />
      <circle
        cx={28}
        cy={28}
        r={r}
        fill="none"
        stroke="var(--accent)"
        strokeWidth={5}
        strokeDasharray={`${share * circ} ${circ}`}
        strokeLinecap="round"
        transform="rotate(-90 28 28)"
      />
      <text x={28} y={31} textAnchor="middle" className="mono" fontSize={11} fill="var(--text-secondary)">
        {formatPercent(share)}
      </text>
    </svg>
  );
}

function DriveCard({ vol, onScan, index }: { vol: VolumeInfo; onScan: (path: string) => void; index: number }): React.ReactElement | null {
  if (!vol.hasMedia || Number(vol.total) === 0) {
    return null; // no-media drives excluded (parity-SEL-03)
  }
  const used = Number(vol.total) - Number(vol.free);
  const slow = vol.kind === 'network';
  return (
    <button
      onClick={() => onScan(vol.path)}
      disabled={!vol.hasMedia}
      className="group flex h-[120px] flex-col justify-between rounded-lg border border-hairline bg-surface-raised p-4 text-left transition-colors duration-100 hover:border-border-strong"
      style={{ animation: `card-in 200ms var(--ease-screen) ${Math.min(index, 5) * 24}ms both` }}
    >
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2 text-text-secondary">
          {volumeIcon(vol.kind)}
          <div>
            <div className="font-medium text-text-primary">{vol.path}</div>
            <div className="text-xs text-text-muted">{vol.label || 'Local disk'} · {vol.fs}</div>
          </div>
        </div>
        <DonutGauge used={used} total={vol.total} />
      </div>
      <div className="mono flex items-baseline justify-between text-xs text-text-secondary">
        <span>{formatBytes(used)} used</span>
        <span className="text-text-muted">{formatBytes(vol.free)} free of {formatBytes(vol.total)}</span>
      </div>
      {slow ? <div className="text-2xs text-warning">Network share — scanning may be slow</div> : null}
    </button>
  );
}

export function Welcome(): React.ReactElement {
  const volumes = useScanStore((s) => s.volumes);
  const loadVolumes = useScanStore((s) => s.loadVolumes);
  const beginScan = useScanStore((s) => s.beginScan);
  const [recents, setRecents] = useState<string[]>(loadRecents());
  const [folderPath, setFolderPath] = useState('');

  useEffect(() => {
    void loadVolumes();
  }, [loadVolumes]);

  const scan = useCallback(
    (target: { kind: 'volume'; path: string } | { kind: 'folder'; paths: string[] }) => {
      const path = target.kind === 'volume' ? target.path : target.paths[0]!;
      pushRecent(path);
      setRecents(loadRecents());
      void beginScan(target);
    },
    [beginScan],
  );

  return (
    <div className="flex h-full items-start justify-center overflow-y-auto px-8 py-16">
      <div className="w-full max-[880px] max-w-[880px]">
        <header className="mb-10 text-center">
          <h1 className="text-[28px] font-semibold tracking-[-0.02em]">Prism</h1>
          <p className="mt-1 text-text-secondary">Every byte, accounted for.</p>
        </header>

        <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {volumes
            .filter((v) => v.hasMedia && v.total > 0)
            .map((vol, i) => (
              <DriveCard key={vol.path + vol.serial} vol={vol} index={i} onScan={(p) => scan({ kind: 'volume', path: p })} />
            ))}
          {volumes.length === 0 ? (
            <div className="col-span-full rounded-lg border border-hairline bg-surface-raised p-6 text-center text-text-secondary">
              No volumes detected. <button className="ml-2 inline-flex items-center gap-1 text-accent" onClick={() => void loadVolumes()}>Retry <RefreshCw size={12} /></button>
            </div>
          ) : null}
        </div>

        <div className="mt-6 flex flex-wrap items-center justify-center gap-3">
          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (folderPath.trim()) {
                scan({ kind: 'folder', paths: [folderPath.trim()] });
              }
            }}
          >
            <FolderOpen size={16} className="text-text-muted" />
            <input
              value={folderPath}
              onChange={(e) => setFolderPath(e.target.value)}
              placeholder="Scan a folder path…"
              className="mono w-[320px] rounded-sm border border-hairline bg-surface-inset px-3 py-1.5 text-xs text-text-primary placeholder:text-text-faint focus:border-border-strong focus:outline-none"
            />
            <button
              type="submit"
              className="rounded-sm bg-accent px-3 py-1.5 text-xs font-medium text-accent-contrast transition-colors hover:bg-accent-hover"
            >
              Scan folder
            </button>
          </form>
        </div>

        {recents.length > 0 ? (
          <section className="mt-10">
            <div className="micro-label mb-2">Recent</div>
            <ul className="flex flex-wrap gap-2">
              {recents.map((r) => (
                <li key={r}>
                  <button
                    onClick={() => scan(r.includes('/') && !r.endsWith('/') && r.length <= 3 ? { kind: 'volume', path: r } : { kind: 'folder', paths: [r] })}
                    className="mono rounded-sm border border-hairline bg-surface-raised px-2.5 py-1 text-xs text-text-secondary hover:border-border-strong hover:text-text-primary"
                  >
                    {r}
                  </button>
                </li>
              ))}
            </ul>
          </section>
        ) : null}

        <footer className="mt-12 text-center text-xs text-text-faint">
          Nothing about your files leaves this computer. · 0.1.0-dev
        </footer>
      </div>
      <style>{`@keyframes card-in { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: none; } }`}</style>
    </div>
  );
}
