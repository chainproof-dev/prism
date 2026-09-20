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
}

import * as S from './schemas';

export interface RequestSchemas {
  'sys:hello': null;
  'sys:volumes': S.VolumesQuerySchema;
  'sys:preflight': S.PreflightQuerySchema;
  'scan:start': S.ScanStartQuerySchema;
  'scan:pause': S.ScanControlQuerySchema;
  'scan:resume': S.ScanControlQuerySchema;
  'scan:cancel': S.ScanControlQuerySchema;
  'scan:summary': S.ScanControlQuerySchema;
  'scan:rescan-subtree': S.RescanQuerySchema;
  'scan:reattach': S.ScanControlQuerySchema;
  'tree:children': S.ChildrenQuerySchema;
  'tree:expand-stats': S.NodeQuerySchema;
  'node:detail': S.NodeQuerySchema;
  'node:resolve-path': S.ResolvePathQuerySchema;
  'viz:layout': S.VizLayoutQuerySchema;
  'viz:color-mapping': S.ColorMappingQuerySchema;
  'types:list': S.TypesQuerySchema;
  'types:set-color': S.TypeColorQuerySchema;
  'filter:apply': S.FilterApplyQuerySchema;
  'filter:clear': S.ScanControlQuerySchema;
  'duplicates:run': S.DupesRunQuerySchema;
  'duplicates:cancel': S.ScanControlQuerySchema;
  'duplicates:groups': S.DupesGroupsQuerySchema;
  'cleanup:presets-scan': S.ScanControlQuerySchema;
  'cleanup:stage': S.StageQuerySchema;
  'cleanup:unstage': S.UnstageQuerySchema;
  'cleanup:queue': S.ScanControlQuerySchema;
  'cleanup:execute': S.ExecuteQuerySchema;
  'apps:list': S.AppsQuerySchema;
  'apps:footprint': S.AppFootprintQuerySchema;
  'apps:leftovers': S.ScanControlQuerySchema;
  'snapshots:save': S.SnapshotSaveQuerySchema;
  'snapshots:list': S.SnapshotsQuerySchema;
  'snapshots:diff': S.SnapshotDiffQuerySchema;
  'monitor:start': S.MonitorQuerySchema;
  'monitor:stop': S.MonitorQuerySchema;
  'export:scan': S.ExportQuerySchema;
  'lic:verify-token': S.VerifyTokenQuerySchema;
}

export const PremiumCommands: Record<string, P.PremiumFeature> = {
  'duplicates:run': 'Dupes',
  'duplicates:cancel': 'Dupes',
  'duplicates:groups': 'Dupes',
  'cleanup:execute': 'Cleanup',
  'apps:list': 'Apps',
  'apps:footprint': 'Apps',
  'apps:leftovers': 'Apps',
  'snapshots:save': 'Snapshots',
  'snapshots:list': 'Snapshots',
  'snapshots:diff': 'Snapshots',
  'monitor:start': 'Monitor',
  'monitor:stop': 'Monitor',
  'export:scan': 'Export',
};
