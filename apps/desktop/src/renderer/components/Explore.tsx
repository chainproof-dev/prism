// Explore (docs/10 § 5): the core screen — treemap + tree + types + inspector
// + status. Selection syncs across panes (parity-TRE-04).
import { VizCanvas } from './VizCanvas';
import { FileTree } from './FileTree';
import { TypeList } from './TypeList';
import { Inspector } from './Inspector';
import { AppShell } from './AppShell';
import { TopBar } from './TopBar';
import { useScanStore } from '../stores/scan';
import { formatBytes, formatDuration } from '@prism/shared/client';

export function Explore(): React.ReactElement {
  const summary = useScanStore((s) => s.summary);
  const scanId = useScanStore((s) => s.scanId);

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
        </div>
      }
      inspector={<Inspector />}
      status={
        <StatusStrip
          left={`${summary.files.toLocaleString('en-US')} files · ${formatBytes(summary.allocated)} · ${formatDuration(summary.durationMs)}`}
          right={summary.truncated ? 'Truncated (arena ceiling)' : `scan #${summary.scanId}`}
        />
      }
    >
      <VizCanvas root={0} />
    </AppShell>
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
