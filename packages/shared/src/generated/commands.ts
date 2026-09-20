// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

import type * as P from './protocol';

export type CommandName =
  | 'sys:hello'
  | 'sys:volumes'
  | 'sys:preflight'
  | 'scan:start'
  | 'scan:pause'
  | 'scan:resume'
  | 'scan:cancel'
  | 'scan:summary'
  | 'scan:rescan-subtree'
  | 'scan:reattach'
  | 'tree:children'
  | 'tree:expand-stats'
  | 'node:detail'
  | 'node:resolve-path'
  | 'viz:layout'
  | 'viz:color-mapping'
  | 'types:list'
  | 'types:set-color'
  | 'filter:apply'
  | 'filter:clear'
  | 'duplicates:run'
  | 'duplicates:cancel'
  | 'duplicates:groups'
  | 'cleanup:presets-scan'
  | 'cleanup:stage'
  | 'cleanup:unstage'
  | 'cleanup:queue'
  | 'cleanup:execute'
  | 'apps:list'
  | 'apps:footprint'
  | 'apps:leftovers'
  | 'snapshots:save'
  | 'snapshots:list'
  | 'snapshots:diff'
  | 'monitor:start'
  | 'monitor:stop'
  | 'export:scan'
  | 'lic:verify-token'
  | 'engine:recent-scans'
  ;

export interface CommandsMap {
  'sys:hello': { req: null; res: P.HelloInfo };
  'sys:volumes': { req: P.VolumesQuery; res: P.VolumesPage };
  'sys:preflight': { req: P.PreflightQuery; res: P.PreflightInfo };
  'scan:start': { req: P.ScanStartQuery; res: P.ScanStarted };
  'scan:pause': { req: P.ScanControlQuery; res: void };
  'scan:resume': { req: P.ScanControlQuery; res: void };
  'scan:cancel': { req: P.ScanControlQuery; res: void };
  'scan:summary': { req: P.ScanControlQuery; res: P.ScanSummary };
  'scan:rescan-subtree': { req: P.RescanQuery; res: P.ScanStarted };
  'scan:reattach': { req: P.ScanControlQuery; res: P.ReattachInfo };
  'tree:children': { req: P.ChildrenQuery; res: P.NodeRowsPage };
  'tree:expand-stats': { req: P.NodeQuery; res: P.ExpandStats };
  'node:detail': { req: P.NodeQuery; res: P.NodeDetail };
  'node:resolve-path': { req: P.ResolvePathQuery; res: P.ResolveResult };
  'viz:layout': { req: P.VizLayoutQuery; res: void };
  'viz:color-mapping': { req: P.ColorMappingQuery; res: P.ColorMapping };
  'types:list': { req: P.TypesQuery; res: P.TypesPage };
  'types:set-color': { req: P.TypeColorQuery; res: void };
  'filter:apply': { req: P.FilterApplyQuery; res: P.FilterResult };
  'filter:clear': { req: P.ScanControlQuery; res: void };
  'duplicates:run': { req: P.DupesRunQuery; res: P.DupesRunInfo };
  'duplicates:cancel': { req: P.ScanControlQuery; res: void };
  'duplicates:groups': { req: P.DupesGroupsQuery; res: P.DupesGroupsPage };
  'cleanup:presets-scan': { req: P.ScanControlQuery; res: P.PresetHitsPage };
  'cleanup:stage': { req: P.StageQuery; res: P.StagedTotals };
  'cleanup:unstage': { req: P.UnstageQuery; res: P.StagedTotals };
  'cleanup:queue': { req: P.ScanControlQuery; res: P.QueuePage };
  'cleanup:execute': { req: P.ExecuteQuery; res: P.ExecuteResult };
  'apps:list': { req: P.AppsQuery; res: P.AppsPage };
  'apps:footprint': { req: P.AppFootprintQuery; res: P.AppFootprint };
  'apps:leftovers': { req: P.ScanControlQuery; res: P.AppsPage };
  'snapshots:save': { req: P.SnapshotSaveQuery; res: P.SnapshotInfo };
  'snapshots:list': { req: P.SnapshotsQuery; res: P.SnapshotsPage };
  'snapshots:diff': { req: P.SnapshotDiffQuery; res: P.SnapshotDiffPage };
  'monitor:start': { req: P.MonitorQuery; res: void };
  'monitor:stop': { req: P.MonitorQuery; res: void };
  'export:scan': { req: P.ExportQuery; res: P.ExportResult };
  'lic:verify-token': { req: P.VerifyTokenQuery; res: P.EntitlementGrants };
  'engine:recent-scans': { req: P.ScanHistoryQuery; res: P.ScanHistoryPage };
}

import * as S from './schemas';

export interface RequestSchemas {
  'sys:hello': null;
  'sys:volumes': typeof S.VolumesQuerySchema;
  'sys:preflight': typeof S.PreflightQuerySchema;
  'scan:start': typeof S.ScanStartQuerySchema;
  'scan:pause': typeof S.ScanControlQuerySchema;
  'scan:resume': typeof S.ScanControlQuerySchema;
  'scan:cancel': typeof S.ScanControlQuerySchema;
  'scan:summary': typeof S.ScanControlQuerySchema;
  'scan:rescan-subtree': typeof S.RescanQuerySchema;
  'scan:reattach': typeof S.ScanControlQuerySchema;
  'tree:children': typeof S.ChildrenQuerySchema;
  'tree:expand-stats': typeof S.NodeQuerySchema;
  'node:detail': typeof S.NodeQuerySchema;
  'node:resolve-path': typeof S.ResolvePathQuerySchema;
  'viz:layout': typeof S.VizLayoutQuerySchema;
  'viz:color-mapping': typeof S.ColorMappingQuerySchema;
  'types:list': typeof S.TypesQuerySchema;
  'types:set-color': typeof S.TypeColorQuerySchema;
  'filter:apply': typeof S.FilterApplyQuerySchema;
  'filter:clear': typeof S.ScanControlQuerySchema;
  'duplicates:run': typeof S.DupesRunQuerySchema;
  'duplicates:cancel': typeof S.ScanControlQuerySchema;
  'duplicates:groups': typeof S.DupesGroupsQuerySchema;
  'cleanup:presets-scan': typeof S.ScanControlQuerySchema;
  'cleanup:stage': typeof S.StageQuerySchema;
  'cleanup:unstage': typeof S.UnstageQuerySchema;
  'cleanup:queue': typeof S.ScanControlQuerySchema;
  'cleanup:execute': typeof S.ExecuteQuerySchema;
  'apps:list': typeof S.AppsQuerySchema;
  'apps:footprint': typeof S.AppFootprintQuerySchema;
  'apps:leftovers': typeof S.ScanControlQuerySchema;
  'snapshots:save': typeof S.SnapshotSaveQuerySchema;
  'snapshots:list': typeof S.SnapshotsQuerySchema;
  'snapshots:diff': typeof S.SnapshotDiffQuerySchema;
  'monitor:start': typeof S.MonitorQuerySchema;
  'monitor:stop': typeof S.MonitorQuerySchema;
  'export:scan': typeof S.ExportQuerySchema;
  'lic:verify-token': typeof S.VerifyTokenQuerySchema;
  'engine:recent-scans': typeof S.ScanHistoryQuerySchema;
}

export const PremiumCommands: Record<string, P.PremiumFeature> = {
  'duplicates:run': 'dupes',
  'duplicates:cancel': 'dupes',
  'duplicates:groups': 'dupes',
  'cleanup:execute': 'cleanup',
  'apps:list': 'apps',
  'apps:footprint': 'apps',
  'apps:leftovers': 'apps',
  'snapshots:save': 'snapshots',
  'snapshots:list': 'snapshots',
  'snapshots:diff': 'snapshots',
  'monitor:start': 'monitor',
  'monitor:stop': 'monitor',
  'export:scan': 'export',
};
