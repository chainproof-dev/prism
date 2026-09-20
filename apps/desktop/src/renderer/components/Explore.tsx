// Explore (docs/10 § 5): the core screen — viz rail (9 modes), color rail,
// scope segmented, search, tree + types + inspector + quick wins + cleanup
// queue. Selection syncs across panes (parity-TRE-04).
import { useEffect, useState } from 'react';
import { VizCanvas } from './VizCanvas';
import { FileTree } from './FileTree';
import { TypeList } from './TypeList';
import { Inspector } from './Inspector';
import { AppShell } from './AppShell';
import { TopBar } from './TopBar';
import { QuickWins, CleanupQueuePanel, ErrorsDrawer } from './Cleanup';
import { useScanStore } from '../stores/scan';
import { useUiStore } from '../stores/ui';
import { t } from '../lib/i18n';
import { formatBytes, formatDuration } from '@prism/shared/client';
import { client } from '../lib/prism';

const VIZ_MODES = ['treemap', 'sunburst', 'icicle', 'pack', 'mindmap', 'age-timeline', 'folders', 'table', 'bars'] as const;

export function Explore(): React.ReactElement {
  const summary = useScanStore((s) => s.summary);
  const scanId = useScanStore((s) => s.scanId);
  const errors = useScanStore((s) => s.errors);
  const ui = useUiStore();

  if (!summary || scanId === null) {
    return <div className="flex h-full items-center justify-center text-sm text-text-muted">Finishing scan…</div>;
  }
  return (
    <AppShell
      regions={{ topbar: true, sidebar: true, inspector: true, status: true }}
      topbar={<TopBar />}
      sidebar={
        <div className="flex h-full flex-col">
          <div className="min-h-0 flex-[1.4]">
            <FileTree />
          </div>
          <div className="min-h-0 flex-1 border-t border-hairline">
            <TypeList />
          </div>
          <div className="border-t border-hairline">
            <QuickWins />
          </div>
          <CleanupQueuePanel />
        </div>
      }
      inspector={<Inspector />}
      status={
        <StatusStrip
          left={`${summary.files.toLocaleString('en-US')} files · ${formatBytes(summary.allocated)} · ${formatDuration(summary.durationMs)}`}
          right={
            <>
              {errors.length > 0 && (
                <button
                  type="button"
                  onClick={() => ui.setErrorsDrawerOpen(true)}
                  className="text-danger hover:underline"
                >
                  {errors.length} errors
                </button>
              )}
              {summary.truncated ? ' · Truncated (arena ceiling)' : ` · scan #${summary.scanId}`}
            </>
          }
        />
      }
    >
      <div className="flex h-full flex-col">
        {/* center sub-header: viz rail + color rail + scope (docs/10 § 5) */}
        <div className="flex h-9 shrink-0 items-center gap-3 border-b border-hairline bg-surface-panel px-3">
          <div className="flex items-center gap-0.5">
            {VIZ_MODES.map((mode, i) => (
              <button
                key={mode}
                onClick={() => ui.setVizMode(mode)}
                title={`${mode} (Ctrl+${i + 1})`}
                className={`rounded-sm px-2 py-0.5 text-[11px] capitalize ${
                  ui.vizMode === mode ? 'bg-surface-active text-text-primary' : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                {mode}
              </button>
            ))}
          </div>
          <div className="ml-2 flex items-center gap-0.5 border-l border-hairline pl-3">
            {(['type', 'branch', 'age'] as const).map((cm) => (
              <button
                key={cm}
                onClick={() => ui.setColorMode(cm)}
                className={`rounded-sm px-2 py-0.5 text-[11px] capitalize ${
                  ui.colorMode === cm ? 'bg-surface-active text-text-primary' : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                {cm}
              </button>
            ))}
          </div>
          <div className="ml-auto flex items-center gap-0.5">
            {(
              [
                ['folder', t('explore.inThisFolder')],
                ['files-anywhere', t('explore.biggestFiles')],
                ['folders-anywhere', t('explore.biggestFolders')],
              ] as const
            ).map(([scope, label]) => (
              <button
                key={scope}
                onClick={() => ui.setScope(scope)}
                className={`rounded-sm px-2 py-0.5 text-[11px] ${
                  ui.scope === scope ? 'bg-surface-active text-text-primary' : 'text-text-muted hover:text-text-secondary'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="relative min-h-0 flex-1">
          {ui.vizMode === 'table' ? (
            <TableMode scanId={scanId} mode="table" />
          ) : ui.vizMode === 'bars' ? (
            <TableMode scanId={scanId} mode="bars" />
          ) : (
            <VizCanvas root={0} />
          )}
          <ErrorsDrawer />
        </div>
      </div>
    </AppShell>
  );
}

/** DOM viz modes (docs/11 § 5): Table + Bars served by typed queries — the
 * canvas modes get binary frames; these get rows. */
function TableMode({ scanId, mode }: { scanId: number; mode: 'table' | 'bars' }): React.ReactElement {
  const [rows, setRows] = useState<{
    id: number; name: string; kind: string; logical: bigint; allocated: bigint; files: number; folders: number;
  }[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    setLoading(true);
    void client
      .invoke('tree:children', {
        scanId,
        nodeId: 0,
        sort: { key: 'allocated', dir: 'desc' },
        offset: 0,
        limit: 200,
      })
      .then((page) => setRows(page.items))
      .catch(() => setRows([]))
      .finally(() => setLoading(false));
  }, [scanId]);

  const max = rows.length > 0 ? Number(rows[0]?.allocated ?? 0n) : 1;
  if (loading) return <div className="grid h-full place-items-center text-xs text-text-faint">…</div>;

  return (
    <div className="h-full overflow-y-auto">
      <table className="w-full text-xs">
        <thead className="sticky top-0 bg-surface-panel text-left text-[11px] uppercase tracking-wide text-text-faint">
          <tr>
            <th className="px-4 py-2 font-medium">name</th>
            {mode === 'bars' && <th className="w-[40%] py-2 font-medium">share</th>}
            <th className="px-2 py-2 text-right font-medium">size</th>
            <th className="px-2 py-2 text-right font-medium">{t('tree.files')}</th>
            <th className="px-4 py-2 text-right font-medium">{t('tree.folders')}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr
              key={r.id}
              onClick={() => useScanStore.getState().select(r.id)}
              className="cursor-pointer border-b border-hairline/50 hover:bg-surface-hover"
            >
              <td className="px-4 py-1.5 text-text-primary">
                <span className={r.kind === 'dir' || r.kind === 'root' ? '' : 'text-text-secondary'}>{r.name}</span>
              </td>
              {mode === 'bars' && (
                <td className="py-1.5 pr-4">
                  <div className="h-3 overflow-hidden rounded-sm bg-surface-inset">
                    <div
                      className="h-full rounded-sm bg-accent"
                      style={{ width: `${Math.max(0.5, (Number(r.allocated) / max) * 100)}%` }}
                    />
                  </div>
                </td>
              )}
              <td className="mono px-2 py-1.5 text-right text-text-secondary">{formatBytes(r.allocated)}</td>
              <td className="mono px-2 py-1.5 text-right text-text-faint">{r.files.toLocaleString('en-US')}</td>
              <td className="mono px-4 py-1.5 text-right text-text-faint">{r.folders.toLocaleString('en-US')}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export function StatusStrip({ left, right }: { left: React.ReactNode; right?: React.ReactNode }): React.ReactElement {
  return (
    <div className="flex h-7 shrink-0 items-center justify-between border-t border-hairline bg-surface-panel px-3 text-xs text-text-secondary">
      <span className="mono">{left}</span>
      <span className="text-text-faint">{right}</span>
    </div>
  );
}
