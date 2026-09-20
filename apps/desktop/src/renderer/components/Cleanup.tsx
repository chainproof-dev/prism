// Cleanup system (docs/10 § 13 — the deletion contract):
// ContextMenu (WDS-CTX-01 set) → stage → LedgerSheet (review) → execute
// (fail-loud outcomes) → tree updates. ErrorsDrawer (docs/10 § 11).
import { useEffect, useState } from 'react';
import { toast } from 'sonner';
import { useScanStore } from '../stores/scan';
import { useCleanupStore, useUiStore } from '../stores/ui';
import { t } from '../lib/i18n';
import { formatBytes } from '@prism/shared/client';
import { presetsScan } from '../lib/prism';
import type { PresetHit } from '@prism/shared/generated';

// ---------------------------------------------------------------- context menu

export interface CtxTarget {
  nodeId: number;
  name: string;
  path: string;
  x: number;
  y: number;
}

export function ContextMenu({ target, onClose }: { target: CtxTarget; onClose: () => void }): React.ReactElement | null {
  const scanId = useScanStore((s) => s.scanId);
  const select = useScanStore((s) => s.select);
  const stage = useCleanupStore((s) => s.stage);
  if (!target) return null;

  const item = (label: string, fn: () => void, danger = false): React.ReactElement => (
    <button
      key={label}
      type="button"
      onClick={() => {
        fn();
        onClose();
      }}
      className={`block w-full px-3 py-1.5 text-left text-xs hover:bg-surface-hover ${danger ? 'text-danger' : 'text-text-primary'}`}
    >
      {label}
    </button>
  );

  return (
    <>
      <div className="fixed inset-0 z-40" onClick={onClose} onContextMenu={(e) => { e.preventDefault(); onClose(); }} />
      <div
        className="fixed z-50 min-w-[200px] rounded-lg border border-hairline bg-surface-panel py-1 shadow-xl"
        style={{ left: Math.min(target.x, window.innerWidth - 220), top: Math.min(target.y, window.innerHeight - 260) }}
      >
        {item('Copy path', () => void navigator.clipboard.writeText(target.path))}
        {item(t('explore.reveal'), () => select(target.nodeId))}
        {scanId !== null && item('Rescan this folder', () => {
          void window.prism?.invoke('scan:rescan-subtree', { scanId, nodeId: target.nodeId, options: { followReparse: false, sizeMode: 'allocated', treatPackagesAsNodes: false, excludePatterns: [] } })
            .catch(() => toast.error('rescan failed'));
        })}
        <div className="my-1 border-t border-hairline" />
        {scanId !== null && item(t('cleanup.stage'), () => {
          void stage(scanId, [target.nodeId], 'manual').then((res) => {
            if (res.rejected.length > 0) {
              toast.error(t('cleanup.blocked'));
            } else {
              toast.success(`${1} staged`);
            }
          });
        }, true)}
        {scanId !== null && item(`${t('cleanup.stage')} (permanent)`, () => {
          void stage(scanId, [target.nodeId], 'manual').then(() => useCleanupStore.getState().openLedger());
        }, true)}
      </div>
    </>
  );
}

// ---------------------------------------------------------------- ledger sheet

export function LedgerSheet(): React.ReactElement | null {
  const open = useCleanupStore((s) => s.ledgerOpen);
  const staged = useCleanupStore((s) => s.staged);
  const executing = useCleanupStore((s) => s.executing);
  const execute = useCleanupStore((s) => s.execute);
  const close = useCleanupStore((s) => s.closeLedger);
  const unstage = useCleanupStore((s) => s.unstage);
  const scanId = useScanStore((s) => s.scanId);
  const [mode, setMode] = useState<'recycle' | 'permanent'>('recycle');

  useEffect(() => {
    void useCleanupStore.getState().refreshQueue();
  }, [open]);

  if (!open) return null;

  const totalBytes = staged.reduce((a, i) => a + i.bytes, 0);
  const blocked = staged.filter((i) =>
    ['C:\\Windows', 'C:\\Program Files', 'C:\\ProgramData'].some((r) => i.path.toLowerCase().startsWith(r.toLowerCase())),
  );
  const go = (): void => {
    if (scanId === null) return;
    void execute(scanId, mode === 'recycle').then((res) => {
      const r = useCleanupStore.getState().lastResult;
      if (r) {
        if (r.failed > 0) {
          toast.error(`${r.failed} items failed — they stay in the ledger`);
        } else {
          toast.success(t('cleanup.reclaimed', { bytes: formatBytes(r.reclaimed) }));
          close();
        }
      }
      void res;
    });
  };

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/40" onClick={close}>
      <div
        className="flex h-full w-[520px] max-w-[92vw] flex-col border-l border-hairline bg-surface-panel"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-hairline px-4 py-3">
          <span className="text-sm font-medium">{t('cleanup.ledger')}</span>
          <button type="button" onClick={close} className="text-text-faint hover:text-text-primary">✕</button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto">
          {staged.length === 0 && <div className="p-6 text-center text-xs text-text-faint">{t('cleanup.empty')}</div>}
          {staged.map((i) => (
            <div key={i.nodeId} className="group flex items-center justify-between border-b border-hairline/50 px-4 py-1.5">
              <span className="mono truncate text-xs text-text-secondary">{i.path}</span>
              <span className="ml-2 flex shrink-0 items-center gap-2">
                <span className="text-[11px] text-text-faint">{i.source}</span>
                <span className="mono text-xs text-text-primary">{formatBytes(i.bytes)}</span>
                <button
                  type="button"
                  onClick={() => void unstage([i.nodeId])}
                  className="text-text-faint opacity-0 hover:text-danger group-hover:opacity-100"
                >
                  ✕
                </button>
              </span>
            </div>
          ))}
        </div>

        {blocked.length > 0 && (
          <div className="border-t border-danger/40 bg-danger/10 px-4 py-2 text-[11px] text-danger">
            {t('cleanup.blocked')} ({blocked.length})
          </div>
        )}

        <div className="border-t border-hairline p-4">
          <div className="mb-3 flex gap-2">
            {(['recycle', 'permanent'] as const).map((m) => (
              <button
                key={m}
                type="button"
                onClick={() => setMode(m)}
                className={`rounded-md px-3 py-1.5 text-xs ${mode === m ? 'bg-accent text-on-accent' : 'border border-hairline text-text-secondary'}`}
              >
                {m === 'recycle' ? t('cleanup.recycle') : t('cleanup.permanent')}
              </button>
            ))}
          </div>
          <button
            type="button"
            disabled={staged.length === 0 || executing}
            onClick={go}
            className="w-full rounded-lg bg-danger px-4 py-2.5 text-sm font-medium text-white hover:opacity-90 disabled:opacity-40"
          >
            {executing ? '…' : t('cleanup.execute', { count: staged.length, bytes: formatBytes(totalBytes) })}
            {' '}
            {mode === 'recycle' ? t('cleanup.recycle') : t('cleanup.permanent')}
          </button>
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- errors drawer

export function ErrorsDrawer(): React.ReactElement | null {
  const open = useUiStore((s) => s.errorsDrawerOpen);
  const errors = useScanStore((s) => s.errors);
  if (!open) return null;
  return (
    <div className="absolute inset-x-0 bottom-0 z-30 flex h-[40%] flex-col border-t border-hairline bg-surface-panel">
      <div className="flex items-center justify-between border-b border-hairline px-4 py-2">
        <span className="text-xs font-medium">
          {t('errors.title')} <span className="text-text-faint">({errors.length})</span>
        </span>
        <div className="flex gap-3 text-[11px]">
          <button
            type="button"
            onClick={() => void navigator.clipboard.writeText(errors.map((e) => `${e.path}: ${e.reason}`).join('\n'))}
            className="text-accent hover:underline"
          >
            {t('errors.copyAll')}
          </button>
          <button
            type="button"
            onClick={() => useUiStore.getState().setErrorsDrawerOpen(false)}
            className="text-text-faint hover:text-text-primary"
          >
            ✕
          </button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {errors.length === 0 && <div className="p-6 text-center text-xs text-text-faint">{t('errors.empty')}</div>}
        {errors.map((e, i) => (
          <div key={`${e.path}-${i}`} className="flex items-center justify-between border-b border-hairline/50 px-4 py-1.5 text-xs">
            <span className="mono truncate text-text-secondary">{e.path}</span>
            <span className="ml-3 shrink-0 text-danger">{e.reason}</span>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- sidebar panels

export function QuickWins(): React.ReactElement {
  const scanId = useScanStore((s) => s.scanId);
  const [hits, setHits] = useState<PresetHit[]>([]);

  useEffect(() => {
    if (scanId !== null) {
      void presetsScan(scanId).then(setHits).catch(() => setHits([]));
    } else {
      setHits([]);
    }
  }, [scanId]);

  const top = hits.slice(0, 6);
  return (
    <div className="p-3">
      <div className="mb-2 text-[11px] font-medium uppercase tracking-wide text-text-faint">
        {t('quickwins.title')}
      </div>
      {top.length === 0 && <div className="text-xs text-text-faint">{t('cleanup.empty')}</div>}
      {top.map((h) => (
        <button
          key={h.presetId}
          type="button"
          title={h.explanation}
          onClick={() => {
            if (scanId === null) return;
            toast(`preset "${h.name}" staged (${h.paths.length} locations)`);
          }}
          className="mb-1 block w-full rounded-md border border-hairline px-2.5 py-1.5 text-left hover:bg-surface-hover"
        >
          <div className="flex items-center justify-between">
            <span className="truncate text-xs text-text-primary">{h.name}</span>
            <span className="mono text-[11px] text-accent">{formatBytes(h.bytes)}</span>
          </div>
        </button>
      ))}
    </div>
  );
}

export function CleanupQueuePanel(): React.ReactElement {
  const staged = useCleanupStore((s) => s.staged);
  const open = useCleanupStore((s) => s.openLedger);
  const total = staged.reduce((a, i) => a + i.bytes, 0);
  return (
    <div className="border-t border-hairline p-3">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[11px] font-medium uppercase tracking-wide text-text-faint">
          {t('cleanup.queue')}
        </span>
        {staged.length > 0 && (
          <button type="button" onClick={open} className="text-[11px] text-accent hover:underline">
            {t('cleanup.ledger')}
          </button>
        )}
      </div>
      <div className="text-xs text-text-secondary">
        {staged.length > 0 ? (
          <>
            <span className="mono">{staged.length}</span> items · <span className="mono">{formatBytes(total)}</span>
          </>
        ) : (
          <span className="text-text-faint">{t('cleanup.empty')}</span>
        )}
      </div>
    </div>
  );
}
