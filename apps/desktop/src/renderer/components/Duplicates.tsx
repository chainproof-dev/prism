// Duplicates tab (PRISM-HG-030, docs/10 § 6): run → phases → groups; staging
// extras enters the ledger (guarded there, never auto-deleted).
import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { useScanStore } from '../stores/scan';
import { useCleanupStore, useLicenseStore, useUiStore } from '../stores/ui';
import { t } from '../lib/i18n';
import {
  dupesRun, dupesCancel, dupesGroups, onEngineEvents,
  type DuplicateGroup,
} from '../lib/prism';
import { formatBytes } from '@prism/shared/client';

type Phase = 'idle' | 'running' | 'done' | 'none-found';

export function Duplicates(): React.ReactElement {
  const summary = useScanStore((s) => s.summary);
  const scanId = useScanStore((s) => s.scanId);
  const phase = useLicenseStore((s) => s.phase);
  const setTab = useUiStore((s) => s.setTab);
  const [phaseState, setPhaseState] = useState<Phase>('idle');
  const [progress, setProgress] = useState<{ phase: string; groupsFound: number; hashedBytes: number } | null>(null);
  const [groups, setGroups] = useState<DuplicateGroup[]>([]);
  const [total, setTotal] = useState(0);
  const [reclaimable, setReclaimable] = useState(0n);
  const runId = { current: null as number | null };

  useEffect(() => {
    const off = onEngineEvents((ev) => {
      if (ev.ev === 'dupes-progress') {
        const p = (ev as unknown as { dupes: { phase: string; groupsFound: number; hashedBytes: number } }).dupes;
        setProgress(p);
      }
    });
    return off;
  }, []);

  if (phase === 'unlicensed' || phase === 'expired') {
    setTab('explore');
    return <div />;
  }

  if (!summary || scanId === null) {
    return (
      <Empty
        title={t('dupes.needScan')}
        action={{ label: 'Scan a drive', onClick: () => setTab('explore') }}
      />
    );
  }

  const run = (): void => {
    setPhaseState('running');
    setGroups([]);
    void dupesRun(scanId, 1024n)
      .then(async (id) => {
        runId.current = id;
        // poll groups while running (events drive the phase display)
        for (let i = 0; i < 600; i++) {
          await new Promise((r) => setTimeout(r, 500));
          if (runId.current === null) return;
          const page = await dupesGroups(id, 0, 50).catch(() => null);
          if (page && page.total > 0) break;
        }
        const page = await dupesGroups(runId.current ?? 1, 0, 100);
        setGroups(page.groups);
        setTotal(page.total);
        setReclaimable(page.reclaimable);
        setPhaseState(page.total > 0 ? 'done' : 'none-found');
        runId.current = null;
      })
      .catch(() => {
        setPhaseState('idle');
        toast.error('duplicates run failed');
      });
  };

  const stageExtras = (): void => {
    const extras = groups.flatMap((g) => g.members.filter((m) => !m.kept && !m.hardlinkOfKept).map((m) => m.nodeId));
    if (extras.length === 0) return;
    void useCleanupStore.getState().stage(scanId, extras, 'duplicate').then(() => {
      toast.success(`${extras.length} items staged — review in the cleanup ledger`);
      void useCleanupStore.getState().openLedger();
    });
  };

  return (
    <div className="flex h-full flex-col bg-surface-app">
      <div className="flex items-center justify-between border-b border-hairline bg-surface-panel px-4 py-3">
        <div>
          <div className="text-sm font-medium text-text-primary">{t('dupes.title')}</div>
          {phaseState === 'done' && (
            <div className="mt-0.5 text-xs text-text-secondary">
              {total} groups · {formatBytes(reclaimable)} reclaimable
            </div>
          )}
          {phaseState === 'running' && progress && (
            <div className="mt-0.5 text-xs text-text-secondary mono">
              {progress.phase} · {progress.groupsFound} groups · {formatBytes(progress.hashedBytes)} hashed
            </div>
          )}
        </div>
        <div className="flex gap-2">
          {groups.length > 0 && (
            <button
              type="button"
              onClick={stageExtras}
              className="rounded-md border border-hairline px-3 py-1.5 text-xs text-text-primary hover:bg-surface-hover"
            >
              {t('dupes.stageExtras')}
            </button>
          )}
          {phaseState === 'running' ? (
            <button
              type="button"
              onClick={() => {
                runId.current = null;
                void dupesCancel(scanId);
                setPhaseState('idle');
              }}
              className="rounded-md bg-accent px-4 py-1.5 text-xs font-medium text-on-accent"
            >
              {t('scan.cancel')}
            </button>
          ) : (
            <button
              type="button"
              onClick={run}
              className="rounded-md bg-accent px-4 py-1.5 text-xs font-medium text-on-accent hover:bg-accent-hover"
            >
              {t('dupes.run')}
            </button>
          )}
        </div>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {phaseState === 'none-found' && <Empty title={t('dupes.noneFound')} />}
        {groups.map((g) => (
          <div key={g.groupId} className="border-b border-hairline">
            <div className="flex items-center justify-between bg-surface-panel/50 px-4 py-2">
              <span className="text-xs text-text-secondary">
                {g.members.length} files · {formatBytes(g.reclaimable)} waste
              </span>
              <span className="mono text-[11px] text-text-faint">group #{g.groupId}</span>
            </div>
            {g.members.map((m) => (
              <div
                key={`${g.groupId}-${m.nodeId}`}
                className="flex items-center justify-between px-4 py-1.5 text-xs hover:bg-surface-hover"
              >
                <span className="mono truncate text-text-secondary">{m.path}</span>
                <span className="ml-3 flex shrink-0 items-center gap-2">
                  {m.kept && (
                    <span className="rounded bg-success/15 px-1.5 py-0.5 text-[10px] text-success">
                      {m.hardlinkOfKept ? 'hard link' : t('dupes.kept')}
                    </span>
                  )}
                  <span className="text-text-faint">{formatBytes(m.bytes)}</span>
                </span>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

export function Empty({ title, action }: { title: string; action?: { label: string; onClick: () => void } }): React.ReactElement {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-surface-app">
      <div className="text-sm text-text-secondary">{title}</div>
      {action && (
        <button
          type="button"
          onClick={action.onClick}
          className="rounded-md bg-accent px-4 py-1.5 text-xs font-medium text-on-accent"
        >
          {action.label}
        </button>
      )}
    </div>
  );
}
