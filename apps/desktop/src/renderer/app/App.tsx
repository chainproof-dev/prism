// App root: router-less screen switching (docs/10 § 1) + engine event wiring
// + global keyboard scope (Esc = cancel, docs/10 § 14).
import { useEffect } from 'react';
import { useScanStore } from '../stores/scan';
import { AppShell } from '../components/AppShell';
import { Welcome } from '../components/Welcome';
import { ScanningOverlay } from '../components/ScanningOverlay';
import { Explore } from '../components/Explore';
import { TopBar } from '../components/TopBar';
import { cancelScan } from '../lib/prism';

export function App(): React.ReactElement {
  const screen = useScanStore((s) => s.screen);
  const handleEvent = useScanStore((s) => s.handleEvent);
  const loadVolumes = useScanStore((s) => s.loadVolumes);
  const scanId = useScanStore((s) => s.scanId);

  useEffect(() => {
    const off = window.prism?.onEvents
      ? window.prism.onEvents((batch) => {
          for (const ev of batch) {
            handleEvent(ev);
          }
        })
      : null;
    void loadVolumes();
    return () => off?.();
  }, [handleEvent, loadVolumes]);

  // Esc = cancel scan (docs/10 § 14, context-ordered)
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === 'Escape' && scanId !== null) {
        void cancelScan(scanId);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [scanId]);

  if (screen === 'welcome') {
    return (
      <AppShell regions={{ topbar: false, sidebar: false, inspector: false, status: false }}>
        <Welcome />
      </AppShell>
    );
  }
  if (screen === 'scanning') {
    return (
      <AppShell regions={{ topbar: true, sidebar: false, inspector: false, status: true }} topbar={<TopBar />}>
        <ScanningOverlay />
      </AppShell>
    );
  }
  return <Explore />;
}
