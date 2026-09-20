// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

import type * as P from './protocol';

export type EngineEvent =
  | { ev: 'scan-progress'; progress: ScanProgress }
  | { ev: 'scan-nodes'; nodes: ScanNodes }
  | { ev: 'scan-phase'; phase: ScanPhaseEvent }
  | { ev: 'scan-error-batch'; errors: ScanErrorBatch }
  | { ev: 'scan-done'; done: ScanDone }
  | { ev: 'filter-updated'; filter: FilterUpdated }
  | { ev: 'dupes-progress'; dupes: DupesProgress }
  | { ev: 'cleanup-progress'; cleanup: CleanupProgress }
  | { ev: 'monitor-sample'; sample: MonitorSample }
  | { ev: 'license-changed'; sku: Sku }
  | { ev: 'engine-warning'; warning: EngineWarning }
  ;

export interface EventsMap {
  'scan:progress': { payload: ScanProgress };
  'scan:nodes': { payload: ScanNodes };
  'scan:phase': { payload: ScanPhaseEvent };
  'scan:error-batch': { payload: ScanErrorBatch };
  'scan:done': { payload: ScanDone };
  'filter:updated': { payload: FilterUpdated };
  'dupes:progress': { payload: DupesProgress };
  'cleanup:progress': { payload: CleanupProgress };
  'monitor:sample': { payload: MonitorSample };
  'license:changed': { payload: Sku };
  'engine:warning': { payload: EngineWarning };
}

export type EventName = keyof EventsMap;
