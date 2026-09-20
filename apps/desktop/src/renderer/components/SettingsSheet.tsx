// Settings sheet (docs/10 § 10): right sheet, 420px, sections with
// reset-per-section (parity-CFG-03). Account section: license state +
// deactivate (docs/13 § 5.4).
import { useEffect, useState } from 'react';
import { useUiStore, useLicenseStore } from '../stores/ui';
import { t, setLocale, type Locale } from '../lib/i18n';
import { SchedulerSection } from './Scheduler';

interface SettingsShape {
  general: { theme: string; density: string; sizeDisplay: string; language: string; autostart: boolean; shellIntegration: boolean };
  scanner: { strategyDefault: string; followReparse: boolean; packagesAsNodes: boolean; exclusions: string[] };
  viz: { defaultMode: string; labels: boolean; gap: number; cushionBrightness: number };
  cleanup: { defaultMode: string; confirmThresholdMb: number; presetsEnabled: Record<string, boolean> };
  updates: { channel: string };
}

const SECTIONS = ['general', 'scanner', 'viz', 'cleanup', 'updates'] as const;

export function SettingsSheet(): React.ReactElement | null {
  const open = useUiStore((s) => s.settingsOpen);
  const setOpen = useUiStore((s) => s.setSettingsOpen);
  const lic = useLicenseStore();
  const [s, setS] = useState<SettingsShape | null>(null);

  useEffect(() => {
    if (open) {
      const api = window.prism?.settings;
      if (api) void api('get').then((v) => setS(v as SettingsShape));
    }
  }, [open]);

  if (!open || !s) return null;

  const save = (next: SettingsShape): void => {
    setS(next);
const api2 = window.prism?.settings;
    if (api2) void api2('set', next);
  };
  const reset = (section: string): void => {
const api3 = window.prism?.settings;
    if (api3) void api3('reset', section).then((v) => setS(v as SettingsShape));
  };

  const Row = ({ label, children }: { label: string; children: React.ReactNode }): React.ReactElement => (
    <label className="flex items-center justify-between py-2 text-xs">
      <span className="text-text-secondary">{label}</span>
      {children}
    </label>
  );
  const Check = ({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }): React.ReactElement => (
    <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="accent-[var(--color-accent)]" />
  );
  const Text = ({ value, onChange }: { value: string; onChange: (v: string) => void }): React.ReactElement => (
    <input value={value} onChange={(e) => onChange(e.target.value)} className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs" />
  );

  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/40" onClick={() => setOpen(false)}>
      <div className="flex h-full w-[420px] max-w-[92vw] flex-col border-l border-hairline bg-surface-panel" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center justify-between border-b border-hairline px-4 py-3">
          <span className="text-sm font-medium">{t('settings.title')}</span>
          <button type="button" onClick={() => setOpen(false)} className="text-text-faint hover:text-text-primary">✕</button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-4">
          {/* Account */}
          <Section title={t('settings.account')} onReset={null}>
            <div className="rounded-md border border-hairline bg-surface-app p-3 text-xs">
              <div className="text-text-primary">
                {lic.phase === 'licensed' && <>Pro · {lic.sku}</>}
                {lic.phase === 'trial' && <>Trial · {lic.daysLeft} days left</>}
                {lic.phase === 'grace' && <>Grace · reconnect to validate</>}
                {(lic.phase === 'unlicensed' || lic.phase === 'expired') && <>Free</>}
              </div>
              {(lic.phase === 'licensed' || lic.phase === 'grace') && (
                <button
                  type="button"
                  onClick={() => void lic.deactivate()}
                  className="mt-2 rounded border border-hairline px-2.5 py-1 text-[11px] text-text-secondary hover:text-danger"
                >
                  {t('settings.deactivate')}
                </button>
              )}
            </div>
          </Section>

          <Section title={t('settings.general')} onReset={() => reset('general')}>
            <Row label="Theme">
              <select
                value={s.general.theme}
                onChange={(e) => {
                  const next = { ...s, general: { ...s.general, theme: e.target.value } };
                  save(next);
                  useUiStore.getState().setTheme(e.target.value);
                }}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                {['nocturne', 'graphite', 'verdigris', 'ember', 'alabaster', 'terracotta'].map((th) => (
                  <option key={th}>{th}</option>
                ))}
              </select>
            </Row>
            <Row label="Language">
              <select
                value={s.general.language}
                onChange={(e) => {
                  const next = { ...s, general: { ...s.general, language: e.target.value } };
                  save(next);
                  setLocale(e.target.value as Locale);
                }}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                <option value="en-US">English (US)</option>
                <option value="de-DE">Deutsch</option>
              </select>
            </Row>
            <Row label="Size display">
              <select
                value={s.general.sizeDisplay}
                onChange={(e) => save({ ...s, general: { ...s.general, sizeDisplay: e.target.value } })}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                <option value="allocated">Allocated</option>
                <option value="logical">Logical</option>
              </select>
            </Row>
            <Row label="Autostart"><Check checked={s.general.autostart} onChange={(v) => save({ ...s, general: { ...s.general, autostart: v } })} /></Row>
            <Row label="Explorer context menu"><Check checked={s.general.shellIntegration} onChange={(v) => save({ ...s, general: { ...s.general, shellIntegration: v } })} /></Row>
          </Section>

          <Section title={t('settings.scanner')} onReset={() => reset('scanner')}>
            <Row label="Default strategy">
              <select
                value={s.scanner.strategyDefault}
                onChange={(e) => save({ ...s, scanner: { ...s.scanner, strategyDefault: e.target.value } })}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                <option value="standard">Standard walk</option>
                <option value="turbo">Turbo (raw NTFS)</option>
              </select>
            </Row>
            <Row label="Follow reparse points"><Check checked={s.scanner.followReparse} onChange={(v) => save({ ...s, scanner: { ...s.scanner, followReparse: v } })} /></Row>
            <Row label="Treat packages as nodes"><Check checked={s.scanner.packagesAsNodes} onChange={(v) => save({ ...s, scanner: { ...s.scanner, packagesAsNodes: v } })} /></Row>
            <Row label="Exclusions (globs)">
              <Text value={s.scanner.exclusions.join('; ')} onChange={(v) => save({ ...s, scanner: { ...s.scanner, exclusions: v.split(';').map((x) => x.trim()).filter(Boolean) } })} />
            </Row>
          </Section>

          {/* Scheduler (PRISM-HG-080) — premium, self-locks honestly */}
          <Section title="Scheduler" onReset={null}>
            <SchedulerSection />
          </Section>

          <Section title={t('settings.viz')} onReset={() => reset('viz')}>
            <Row label="Labels"><Check checked={s.viz.labels} onChange={(v) => save({ ...s, viz: { ...s.viz, labels: v } })} /></Row>
            <Row label="Gap (px)">
              <input type="number" min={0} max={8} value={s.viz.gap} onChange={(e) => save({ ...s, viz: { ...s.viz, gap: Number(e.target.value) } })} className="w-20 rounded border border-hairline bg-surface-app px-2 py-1 text-xs" />
            </Row>
          </Section>

          <Section title={t('settings.cleanup')} onReset={() => reset('cleanup')}>
            <Row label="Default mode">
              <select
                value={s.cleanup.defaultMode}
                onChange={(e) => save({ ...s, cleanup: { ...s.cleanup, defaultMode: e.target.value } })}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                <option value="recycle">{t('cleanup.recycle')}</option>
                <option value="permanent">{t('cleanup.permanent')}</option>
              </select>
            </Row>
          </Section>

          <Section title={t('settings.updates')} onReset={() => reset('updates')}>
            <Row label="Channel">
              <select
                value={s.updates.channel}
                onChange={(e) => save({ ...s, updates: { ...s.updates, channel: e.target.value } })}
                className="w-40 rounded border border-hairline bg-surface-app px-2 py-1 text-xs"
              >
                <option value="stable">Stable</option>
                <option value="beta">Beta</option>
              </select>
            </Row>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section({ title, onReset, children }: { title: string; onReset: (() => void) | null; children: React.ReactNode }): React.ReactElement {
  return (
    <div className="mb-6">
      <div className="mb-1 flex items-center justify-between">
        <div className="text-[11px] font-medium uppercase tracking-wide text-text-faint">{title}</div>
        {onReset && (
          <button type="button" onClick={onReset} className="text-[11px] text-text-faint hover:text-accent">
            {t('settings.reset')}
          </button>
        )}
      </div>
      {children}
    </div>
  );
}

void SECTIONS;
