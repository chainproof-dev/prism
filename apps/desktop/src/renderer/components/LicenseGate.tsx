// LicenseGate (docs/10 § 2, docs/13 § 5.1): first-run + unlicensed state.
// Explicit trial consent, key input with auto-grouping, checkout handoff
// via shell.openExternal — never an in-app webview (docs/04 § 8).
import { useEffect, useState } from 'react';
import { useLicenseStore } from '../stores/ui';
import { t } from '../lib/i18n';

const GROUP = 5; // XXXXX-XXXXX-… auto-grouping (docs/13 § 5.1)

function groupKey(raw: string): string {
  const clean = raw.toUpperCase().replaceAll(/[^A-Z0-9]/g, '').slice(0, 25);
  const parts: string[] = [];
  for (let i = 0; i < clean.length; i += GROUP) parts.push(clean.slice(i, i + GROUP));
  return parts.join('-');
}

export function LicenseGate(): React.ReactElement {
  const lic = useLicenseStore();
  const [key, setKey] = useState('');

  useEffect(() => {
    void lic.refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const tryActivate = (): void => {
    void lic.activate(key);
  };

  return (
    <div className="flex h-full items-center justify-center bg-surface-app">
      <div className="w-[420px] max-w-[90vw] rounded-2xl border border-hairline bg-surface-panel p-8">
        <div className="mb-1 text-lg font-semibold text-text-primary">Prism</div>
        <div className="mb-6 text-sm text-text-secondary">{t('app.tagline')}</div>

        <button
          type="button"
          onClick={() => void lic.startTrial()}
          className="mb-5 w-full rounded-lg bg-accent px-4 py-2.5 text-sm font-medium text-on-accent transition-colors hover:bg-accent-hover"
        >
          {t('license.startTrial')}
        </button>

        <div className="mb-2 text-[11px] font-medium uppercase tracking-wide text-text-faint">
          {t('license.enterKey')}
        </div>
        <input
          value={key}
          onChange={(e) => setKey(groupKey(e.target.value))}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && key.length >= 5) tryActivate();
          }}
          placeholder="PRSM-XXXXX-XXXXX-XXXXX-XXXXX"
          spellCheck={false}
          className="mono w-full rounded-lg border border-hairline bg-surface-app px-3 py-2 text-sm text-text-primary outline-none focus:border-accent"
        />
        <button
          type="button"
          disabled={key.length < 5 || lic.activating}
          onClick={tryActivate}
          className="mt-3 w-full rounded-lg border border-hairline px-4 py-2 text-sm text-text-primary transition-colors hover:bg-surface-hover disabled:opacity-40"
        >
          {lic.activating ? '…' : t('license.activate')}
        </button>
        {lic.activateError && (
          <div className="mt-3 rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-xs text-danger">
            {lic.activateError}
          </div>
        )}

        <div className="mt-6 border-t border-hairline pt-4 text-center">
          <a
            href="https://prism.example/buy"
            onClick={(e) => {
              e.preventDefault();
              void window.prism?.invoke('shell:open-external', 'https://prism.example/buy').catch(() => undefined);
            }}
            className="text-xs text-accent hover:underline"
          >
            {t('license.buy')}
          </a>
          <div className="mt-3 text-[11px] text-text-faint">{t('app.privacy')}</div>
        </div>
      </div>
    </div>
  );
}

/** Countdown banner for the offline-grace window (docs/13 § 5.3, T−7d). */
export function GraceBanner(): React.ReactElement | null {
  const phase = useLicenseStore((s) => s.phase);
  const daysLeft = useLicenseStore((s) => s.daysLeft);
  if (phase !== 'grace' && !(phase === 'licensed' && daysLeft !== null && daysLeft <= 7)) {
    return null;
  }
  const date = new Date(Date.now() + (daysLeft ?? 0) * 86_400_000).toLocaleDateString();
  return (
    <div className="flex h-8 shrink-0 items-center justify-center gap-2 border-b border-warning/40 bg-warning/10 px-4 text-xs text-warning">
      {t('license.graceBanner', { days: daysLeft ?? 0, date })}
    </div>
  );
}

/** Locked-tab preview (docs/10 § 12): real data shape at 20% opacity + the
 * conversion card. No fake data — the mask explains itself. */
export function LockedTab({ feature }: { feature: string }): React.ReactElement {
  return (
    <div className="relative flex h-full items-center justify-center overflow-hidden bg-surface-app">
      <div
        aria-hidden
        className="pointer-events-none absolute inset-0 flex items-end justify-around opacity-20"
      >
        {Array.from({ length: 12 }, (_, i) => (
          <div
            key={i}
            className="w-14 rounded-t-sm bg-accent"
            style={{ height: `${20 + ((i * 37) % 70)}%` }}
          />
        ))}
      </div>
      <div className="relative z-10 w-[380px] rounded-xl border border-hairline bg-surface-panel p-6 text-center shadow-xl">
        <div className="text-sm font-semibold text-text-primary">{t('license.lockedTitle')}</div>
        <div className="mt-2 text-xs text-text-secondary">{t('license.lockedBody')}</div>
        <div className="mt-4 text-[11px] uppercase tracking-wide text-text-faint">{feature}</div>
      </div>
    </div>
  );
}
