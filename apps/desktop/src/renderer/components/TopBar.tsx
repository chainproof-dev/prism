// TopBar (docs/09 § TopBar): workspace tabs (Alt+1..6, docs/10 § 1),
// search (Ctrl+F), theme menu, settings (Ctrl+,), account chip.
import { useState } from 'react';
import { Search, Settings, Sun, Moon, Command as CommandIcon } from 'lucide-react';
import { THEMES } from '../lib/palette';
import { useScanStore } from '../stores/scan';
import { useUiStore, useLicenseStore, type WorkspaceTab } from '../stores/ui';
import { t } from '../lib/i18n';
import { client } from '../lib/prism';

const TABS: WorkspaceTab[] = ['explore', 'duplicates', 'applications', 'monitor', 'snapshots', 'leftovers'];
const PREMIUM = new Set<WorkspaceTab>(['duplicates', 'applications', 'monitor', 'snapshots', 'leftovers']);

export function TopBar(): React.ReactElement {
  const summary = useScanStore((s) => s.summary);
  const screen = useScanStore((s) => s.screen);
  const ui = useUiStore();
  const licPhase = useLicenseStore((s) => s.phase);
  const [themeOpen, setThemeOpen] = useState(false);
  const [query, setQuery] = useState('');

  const licensed = licPhase === 'licensed' || licPhase === 'trial' || licPhase === 'grace';

  return (
    <div className="flex h-12 items-center gap-3 px-3">
      <div className="flex items-center gap-1.5">
        <span className="grid h-6 w-6 place-items-center rounded-sm bg-accent text-[11px] font-bold text-accent-contrast">P</span>
        <span className="text-sm font-medium">Prism</span>
      </div>

      <nav className="ml-2 flex items-center gap-0.5" aria-label="Workspace">
        {TABS.map((tab, i) => {
          const locked = PREMIUM.has(tab) && !licensed;
          return (
            <button
              key={tab}
              onClick={() => ui.setTab(tab)}
              className={`rounded-sm px-2.5 py-1 text-xs capitalize transition-colors ${
                ui.tab === tab
                  ? 'bg-surface-active text-text-primary'
                  : 'text-text-muted hover:text-text-secondary'
              }`}
              title={locked ? t('license.lockedTitle') : `Alt+${i + 1}`}
            >
              {t(`${tab}.title`)}
              {locked ? <span className="ml-1 text-text-faint">•</span> : null}
            </button>
          );
        })}
      </nav>

      <div className="ml-auto flex items-center gap-2">
        <div className="flex items-center gap-1.5 rounded-sm border border-hairline bg-surface-inset px-2 py-1">
          <Search size={12} className="text-text-faint" />
          <input
            id="prism-search"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              ui.setFilter(e.target.value);
              if (e.target.value.length >= 2) {
                // engine debounce contract: 150ms idle → filter:apply (docs/10 § 13)
                window.clearTimeout((window as unknown as { __ft?: number }).__ft);
                (window as unknown as { __ft?: number }).__ft = window.setTimeout(() => {
                  const sid = useScanStore.getState().scanId;
                  if (sid !== null) {
                    void client
                      .invoke('filter:apply', {
                        scanId: sid,
                        query: { name: e.target.value, categories: [], kind: 'both' },
                        scope: 'all',
                      })
                      .then((res) => useUiStore.getState().setFilterCount(Number(res.matched)))
                      .catch(() => useUiStore.getState().setFilterCount(null));
                  }
                }, 150);
              } else {
                ui.setFilterCount(null);
              }
            }}
            placeholder={t('search.placeholder')}
            className="w-44 bg-transparent text-xs text-text-primary placeholder:text-text-faint focus:outline-none"
          />
          {ui.filterCount !== null && (
            <span className="mono text-[10px] text-accent">
              {ui.filterCount.toLocaleString('en-US')} {t('search.filtered')}
            </span>
          )}
        </div>
        <button
          onClick={() => ui.openPalette()}
          className="grid h-7 w-7 place-items-center rounded-sm text-text-muted hover:bg-surface-hover hover:text-text-secondary"
          aria-label="Command palette (Ctrl+K)"
        >
          <CommandIcon size={14} />
        </button>
        <div className="relative">
          <button
            onClick={() => setThemeOpen((v) => !v)}
            className="grid h-7 w-7 place-items-center rounded-sm text-text-muted hover:bg-surface-hover hover:text-text-secondary"
            aria-label="Theme"
          >
            {typeof document !== 'undefined' && (document.documentElement.dataset.theme ?? '').match(/alabaster|terracotta/) ? <Sun size={14} /> : <Moon size={14} />}
          </button>
          {themeOpen ? (
            <div className="absolute right-0 top-9 z-20 w-40 rounded-md border border-hairline bg-surface-overlay p-1 shadow-lg">
              {THEMES.map((th) => (
                <button
                  key={th}
                  className="block w-full rounded-xs px-2 py-1.5 text-left text-xs capitalize hover:bg-surface-hover"
                  onClick={() => {
                    ui.setTheme(th);
                    setThemeOpen(false);
                  }}
                >
                  {th}
                </button>
              ))}
            </div>
          ) : null}
        </div>
        <button
          onClick={() => ui.setSettingsOpen(true)}
          className="grid h-7 w-7 place-items-center rounded-sm text-text-muted hover:bg-surface-hover hover:text-text-secondary"
          aria-label="Settings (Ctrl+,)"
        >
          <Settings size={14} />
        </button>
        {licPhase === 'licensed' || licPhase === 'grace' ? (
          <span className="rounded-sm bg-accent/15 px-2 py-0.5 text-[10px] font-medium text-accent">PRO</span>
        ) : licPhase === 'trial' ? (
          <span className="rounded-sm bg-warning/15 px-2 py-0.5 text-[10px] font-medium text-warning">
            TRIAL {useLicenseStore.getState().daysLeft}d
          </span>
        ) : null}
        {screen === 'explore' && summary ? (
          <span className="mono rounded-sm border border-hairline px-2 py-0.5 text-[10px] text-text-muted">{summary.root}</span>
        ) : null}
      </div>
    </div>
  );
}
