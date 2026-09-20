// TopBar (docs/09 § TopBar): tabs, search, theme menu, account chip.
import { useState } from 'react';
import { Search, Settings, Sun, Moon } from 'lucide-react';
import { THEMES } from '../lib/palette';
import { useScanStore } from '../stores/scan';

export function TopBar(): React.ReactElement {
  const summary = useScanStore((s) => s.summary);
  const screen = useScanStore((s) => s.screen);
  const [themeOpen, setThemeOpen] = useState(false);
  const [query, setQuery] = useState('');

  return (
    <div className="flex h-12 items-center gap-3 px-3">
      <div className="flex items-center gap-1.5">
        <span className="grid h-6 w-6 place-items-center rounded-sm bg-accent text-[11px] font-bold text-accent-contrast">P</span>
        <span className="text-sm font-medium">Prism</span>
      </div>

      <nav className="ml-2 flex items-center gap-0.5" aria-label="Workspace">
        {['Explore', 'Duplicates', 'Applications', 'Monitor', 'Snapshots', 'Leftovers'].map((tab, i) => (
          <button
            key={tab}
            className={`rounded-sm px-2.5 py-1 text-xs ${i === 0 ? 'bg-surface-active text-text-primary' : 'text-text-muted hover:text-text-secondary'}`}
            title={i > 0 ? 'Premium tab — activates with a license' : undefined}
          >
            {tab}
            {i > 0 ? <span className="ml-1 text-text-faint">🔒</span> : null}
          </button>
        ))}
      </nav>

      <div className="ml-auto flex items-center gap-2">
        <div className="flex items-center gap-1.5 rounded-sm border border-hairline bg-surface-inset px-2 py-1">
          <Search size={12} className="text-text-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Filter…"
            className="w-40 bg-transparent text-xs text-text-primary placeholder:text-text-faint focus:outline-none"
          />
        </div>
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
              {THEMES.map((t) => (
                <button
                  key={t}
                  className="block w-full rounded-xs px-2 py-1.5 text-left text-xs capitalize hover:bg-surface-hover"
                  onClick={() => {
                    document.documentElement.dataset.theme = t;
                    setThemeOpen(false);
                  }}
                >
                  {t}
                </button>
              ))}
            </div>
          ) : null}
        </div>
        <button className="grid h-7 w-7 place-items-center rounded-sm text-text-muted hover:bg-surface-hover hover:text-text-secondary" aria-label="Settings">
          <Settings size={14} />
        </button>
        {screen === 'explore' && summary ? (
          <span className="mono rounded-sm border border-hairline px-2 py-0.5 text-[10px] text-text-muted">{summary.root}</span>
        ) : null}
      </div>
    </div>
  );
}
