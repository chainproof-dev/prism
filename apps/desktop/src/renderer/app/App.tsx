// App root: router-less screen switching (docs/10 § 1) + engine event wiring
// + the full keyboard map (docs/10 § 14) + workspace tabs + overlays.
import { useEffect, useState } from 'react';
import { useScanStore } from '../stores/scan';
import { useUiStore, useLicenseStore, type WorkspaceTab } from '../stores/ui';
import { AppShell } from '../components/AppShell';
import { Welcome } from '../components/Welcome';
import { ScanningOverlay } from '../components/ScanningOverlay';
import { Explore } from '../components/Explore';
import { TopBar } from '../components/TopBar';
import { CommandPalette } from '../components/CommandPalette';
import { SettingsSheet } from '../components/SettingsSheet';
import { LedgerSheet, ContextMenu, type CtxTarget } from '../components/Cleanup';
import { LicenseGate, GraceBanner } from '../components/LicenseGate';
import { Duplicates } from '../components/Duplicates';
import { MonitorTab, SnapshotsTab, ApplicationsTab, LeftoversTab } from '../components/Tabs';
import { LockedTab } from '../components/LicenseGate';
import { cancelScan } from '../lib/prism';
import { useCleanupStore } from '../stores/ui';

export function App(): React.ReactElement {
  const screen = useScanStore((s) => s.screen);
  const handleEvent = useScanStore((s) => s.handleEvent);
  const loadVolumes = useScanStore((s) => s.loadVolumes);
  const scanId = useScanStore((s) => s.scanId);
  const ui = useUiStore();
  const lic = useLicenseStore();
  const [ctx, setCtx] = useState<CtxTarget | null>(null);

  useEffect(() => {
    const off = window.prism?.onEvents
      ? window.prism.onEvents((batch) => {
          for (const ev of batch) {
            handleEvent(ev);
          }
        })
      : null;
    const offLic = window.prism?.onLicense
      ? window.prism.onLicense((mirror) => {
          useLicenseStore.setState({
            phase: mirror.state.phase as never,
            daysLeft: mirror.daysLeft,
          });
        })
      : null;
    const offMenu = window.prism?.onMenu
      ? window.prism.onMenu((cmd) => {
          if (cmd === 'new-scan') useScanStore.setState({ screen: 'welcome' });
          if (cmd === 'open-settings') ui.setSettingsOpen(true);
          if (cmd === 'command-palette') ui.openPalette();
          if (cmd === 'focus-search') document.getElementById('prism-search')?.focus();
          if (cmd === 'toggle-inspector') window.dispatchEvent(new CustomEvent('prism:toggle-inspector'));
        })
      : null;
    void loadVolumes();
    void lic.refresh();
    void useCleanupStore.getState().refreshQueue();
    return () => {
      off?.();
      offLic?.();
      offMenu?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Full keyboard map (docs/10 § 14)
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      const mod = e.ctrlKey || e.metaKey;
      if (mod && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        ui.paletteOpen ? ui.closePalette() : ui.openPalette();
        return;
      }
      if (mod && e.key === ',') {
        e.preventDefault();
        ui.setSettingsOpen(true);
        return;
      }
      if (mod && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        document.getElementById('prism-search')?.focus();
        return;
      }
      if (e.altKey && /^[1-6]$/.test(e.key)) {
        e.preventDefault();
        const tabs: WorkspaceTab[] = ['explore', 'duplicates', 'applications', 'monitor', 'snapshots', 'leftovers'];
        ui.setTab(tabs[Number(e.key) - 1]!);
        return;
      }
      if (e.altKey && e.key === 'ArrowUp') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('prism:zoom-out'));
        return;
      }
      if (e.key === 'F5') {
        e.preventDefault();
        window.dispatchEvent(new CustomEvent('prism:rescan'));
        return;
      }
      if (e.key === 'Escape') {
        if (ui.paletteOpen) {
          ui.closePalette();
          return;
        }
        if (scanId !== null && screen === 'scanning') {
          void cancelScan(scanId);
          return;
        }
        if (ui.filter) {
          ui.setFilter('');
          ui.setFilterCount(null);
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [scanId, screen, ui]);

  // License gate: unlicensed first-run shows the gate (docs/10 § 2). Free
  // tier after expiry: Explore stays; premium tabs lock.
  const gate = lic.phase === 'unlicensed';

  const tabContent = (): React.ReactElement => {
    if (ui.tab === 'explore') return screen === 'welcome' ? <Welcome /> : screen === 'scanning' ? <Explore /> : <Explore />;
    const premiumLive = lic.phase === 'licensed' || lic.phase === 'trial' || lic.phase === 'grace';
    if (!premiumLive) return <LockedTab feature={ui.tab} />;
    switch (ui.tab) {
      case 'duplicates':
        return <Duplicates />;
      case 'monitor':
        return <MonitorTab />;
      case 'snapshots':
        return <SnapshotsTab />;
      case 'applications':
        return <ApplicationsTab />;
      case 'leftovers':
        return <LeftoversTab />;
      default:
        return <Explore />;
    }
  };

  if (gate) {
    return (
      <AppShell regions={{ topbar: false, sidebar: false, inspector: false, status: false }}>
        <LicenseGate />
      </AppShell>
    );
  }

  const showChrome = ui.tab !== 'explore' || screen === 'explore';

  return (
    <div className="flex h-full w-full flex-col">
      <GraceBanner />
      {ui.tab === 'explore' && screen === 'welcome' ? (
        <AppShell regions={{ topbar: false, sidebar: false, inspector: false, status: false }}>
          <Welcome />
        </AppShell>
      ) : ui.tab === 'explore' && screen === 'scanning' ? (
        <AppShell regions={{ topbar: true, sidebar: false, inspector: false, status: true }} topbar={<TopBar />}>
          <ScanningOverlay />
        </AppShell>
      ) : (
        <div className="flex min-h-0 flex-1 flex-col">
          {showChrome ? (
            <div className="flex h-12 shrink-0 items-center border-b border-hairline bg-surface-panel">
              <TopBar />
            </div>
          ) : null}
          <div className="min-h-0 flex-1">{tabContent()}</div>
        </div>
      )}
      <CommandPalette />
      <SettingsSheet />
      <LedgerSheet />
      {ctx && <ContextMenu target={ctx} onClose={() => setCtx(null)} />}
    </div>
  );
}
