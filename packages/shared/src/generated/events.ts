// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

import type * as P from './protocol';

export type EngineEvent =
  | { ev: 'scan-progress'; progress: P.ScanProgress }
  | { ev: 'scan-nodes'; nodes: P.ScanNodes }
  | { ev: 'scan-phase'; phase: P.ScanPhaseEvent }
  | { ev: 'scan-error-batch'; errors: P.ScanErrorBatch }
  | { ev: 'scan-done'; done: P.ScanDone }
  | { ev: 'filter-updated'; filter: P.FilterUpdated }
  | { ev: 'dupes-progress'; dupes: P.DupesProgress }
  | { ev: 'cleanup-progress'; cleanup: P.CleanupProgress }
  | { ev: 'monitor-sample'; sample: P.MonitorSample }
  | { ev: 'license-changed'; sku: P.Sku }
  | { ev: 'engine-warning'; warning: P.EngineWarning }
  ;

export interface EventsMap {
  'scan:progress': { payload: P.ScanProgress };
  'scan:nodes': { payload: P.ScanNodes };
  'scan:phase': { payload: P.ScanPhaseEvent };
  'scan:error-batch': { payload: P.ScanErrorBatch };
  'scan:done': { payload: P.ScanDone };
  'filter:updated': { payload: P.FilterUpdated };
  'dupes:progress': { payload: P.DupesProgress };
  'cleanup:progress': { payload: P.CleanupProgress };
  'monitor:sample': { payload: P.MonitorSample };
  'license:changed': { payload: P.Sku };
  'engine:warning': { payload: P.EngineWarning };
}

export type EventName = keyof EventsMap;
