// Command palette (PRISM-HG-110, docs/09 § CommandPalette): static commands
// + recent paths + theme switching + jump-to-biggest (via tree:children
// sort). cmdk core, themed to the token system.
import { Command } from 'cmdk';
import { useEffect, useState } from 'react';
import { useUiStore } from '../stores/ui';
import { useScanStore } from '../stores/scan';
import { t } from '../lib/i18n';
import { client } from '../lib/prism';

const THEMES = ['nocturne', 'graphite', 'verdigris', 'ember', 'alabaster', 'terracotta'] as const;

export function CommandPalette(): React.ReactElement | null {
  const open = useUiStore((s) => s.paletteOpen);
  const close = useUiStore((s) => s.closePalette);
  const ui = useUiStore();
  const scanId = useScanStore((s) => s.scanId);
  const select = useScanStore((s) => s.select);
  const [recent, setRecent] = useState<{ target: string; files: number }[]>([]);

  useEffect(() => {
    if (!open) return;
    void client
      .invoke('engine:recent-scans', { limit: 6 })
      .then((page) => setRecent(page.records.map((r) => ({ target: r.target, files: Number(r.files) }))))
      .catch(() => setRecent([]));
  }, [open]);

  if (!open) return null;

  const run = (fn: () => void): void => {
    fn();
    close();
  };

  const jumpBiggest = (): void => {
    if (scanId === null) return;
    void client
      .invoke('tree:children', { scanId, nodeId: 0, sort: { key: 'allocated', dir: 'desc' }, offset: 0, limit: 1 })
      .then((page) => {
        const top = page.items[0];
        if (top) select(top.id);
      })
      .catch(() => undefined);
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/50 pt-[12vh]"
      onClick={close}
    >
      <Command
        label={t('palette.placeholder')}
        className="w-[560px] max-w-[90vw] overflow-hidden rounded-xl border border-hairline bg-surface-panel shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <Command.Input
          autoFocus
          placeholder={t('palette.placeholder')}
          className="w-full border-b border-hairline bg-transparent px-4 py-3 text-sm text-text-primary outline-none"
          onKeyDown={(e) => {
            if (e.key === 'Escape') close();
          }}
        />
        <Command.List className="max-h-[320px] overflow-y-auto p-1">
          <Command.Empty className="px-3 py-6 text-center text-sm text-text-faint">
            —
          </Command.Empty>

          <Command.Group heading="View" className="[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-text-faint">
            {(['treemap', 'sunburst', 'icicle', 'pack', 'mindmap', 'age-timeline', 'folders', 'table', 'bars'] as const).map((mode) => (
              <Command.Item
                key={mode}
                value={`viz ${mode}`}
                onSelect={() =>
                  run(() => {
                    ui.setTab('explore');
                    ui.setVizMode(mode);
                  })
                }
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
              >
                {mode === 'table' || mode === 'bars' || mode === 'folders' ? `View: ${mode}` : `View: ${mode}`}
              </Command.Item>
            ))}
            {(['type', 'branch', 'age'] as const).map((cm) => (
              <Command.Item
                key={cm}
                value={`color ${cm}`}
                onSelect={() => run(() => ui.setColorMode(cm))}
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
              >
                Color by {cm}
              </Command.Item>
            ))}
            <Command.Item
              value="inspector"
              onSelect={() => run(() => window.dispatchEvent(new CustomEvent('prism:toggle-inspector')))}
              className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
            >
              Toggle inspector
            </Command.Item>
          </Command.Group>

          <Command.Group heading={t('palette.themes')} className="[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-text-faint">
            {THEMES.map((th) => (
              <Command.Item
                key={th}
                value={`theme ${th}`}
                onSelect={() => run(() => ui.setTheme(th))}
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
              >
                {th}
              </Command.Item>
            ))}
          </Command.Group>

          <Command.Group heading="Go" className="[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-text-faint">
            <Command.Item
              value="jump biggest file"
              onSelect={() => run(jumpBiggest)}
              className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
            >
              {t('palette.jumpBiggest')}
            </Command.Item>
            {(['explore', 'duplicates', 'applications', 'monitor', 'snapshots', 'leftovers'] as const).map((tab) => (
              <Command.Item
                key={tab}
                value={`go ${tab}`}
                onSelect={() => run(() => ui.setTab(tab))}
                className="cursor-pointer rounded-md px-3 py-1.5 text-sm capitalize data-[selected=true]:bg-surface-hover"
              >
                {t(`${tab}.title`)}
              </Command.Item>
            ))}
          </Command.Group>

          {recent.length > 0 && (
            <Command.Group heading={t('welcome.recents')} className="[&_[cmdk-group-heading]]:px-3 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:uppercase [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-text-faint">
              {recent.map((r) => (
                <Command.Item
                  key={r.target}
                  value={r.target}
                  onSelect={() =>
                    run(() => {
                      void useScanStore.getState().beginScan({ kind: 'folder', paths: [r.target] });
                    })
                  }
                  className="cursor-pointer rounded-md px-3 py-1.5 text-sm data-[selected=true]:bg-surface-hover"
                >
                  <span className="mono truncate">{r.target}</span>
                  <span className="ml-2 text-xs text-text-faint">{r.files.toLocaleString('en-US')} files</span>
                </Command.Item>
              ))}
            </Command.Group>
          )}
        </Command.List>
      </Command>
    </div>
  );
}
