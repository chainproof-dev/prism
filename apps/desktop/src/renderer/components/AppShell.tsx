// AppShell — the machined single-surface frame (docs/09 § AppShell):
// TopBar / Center / Inspector / StatusStrip separated by hairlines.
import type { ReactNode } from 'react';

export interface ShellRegions {
  topbar: boolean;
  sidebar: boolean;
  inspector: boolean;
  status: boolean;
}

export function AppShell({
  regions,
  topbar,
  sidebar,
  inspector,
  status,
  children,
}: {
  regions: ShellRegions;
  topbar?: ReactNode;
  sidebar?: ReactNode;
  inspector?: ReactNode;
  status?: ReactNode;
  children: ReactNode;
}): React.ReactElement {
  return (
    <div className="flex h-full w-full flex-col bg-surface-app text-text-primary">
      {regions.topbar ? <div className="h-12 shrink-0 border-b border-hairline bg-surface-panel">{topbar}</div> : null}
      <div className="flex min-h-0 flex-1">
        {regions.sidebar ? (
          <aside className="w-[264px] shrink-0 overflow-y-auto border-r border-hairline bg-surface-panel">{sidebar}</aside>
        ) : null}
        <main className="relative min-w-0 flex-1">{children}</main>
        {regions.inspector ? (
          <aside className="w-[300px] shrink-0 overflow-y-auto border-l border-hairline bg-surface-panel">{inspector}</aside>
        ) : null}
      </div>
      {regions.status ? (
        <div className="flex h-7 shrink-0 items-center gap-4 border-t border-hairline bg-surface-panel px-3 text-xs text-text-secondary">
          {status}
        </div>
      ) : null}
    </div>
  );
}
