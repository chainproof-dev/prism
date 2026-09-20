// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

import { z } from 'zod';

export const CleanupSourceSchema = z.enum(['manual', 'preset', 'duplicate', 'stale', 'leftover']);

export const ColorModeSchema = z.enum(['type', 'branch', 'age']);

export const DupesPhaseSchema = z.enum(['grouping-sizes', 'grouping-ext', 'fingerprinting', 'full-hashing', 'hardlink-collapse', 'done']);

export const EngineEventSchema = z.discriminatedUnion('ev', [z.object({ ev: z.literal('scan-progress'), progress: z.lazy(() => ScanProgressSchema) }), z.object({ ev: z.literal('scan-nodes'), nodes: z.lazy(() => ScanNodesSchema) }), z.object({ ev: z.literal('scan-phase'), phase: z.lazy(() => ScanPhaseEventSchema) }), z.object({ ev: z.literal('scan-error-batch'), errors: z.lazy(() => ScanErrorBatchSchema) }), z.object({ ev: z.literal('scan-done'), done: z.lazy(() => ScanDoneSchema) }), z.object({ ev: z.literal('filter-updated'), filter: z.lazy(() => FilterUpdatedSchema) }), z.object({ ev: z.literal('dupes-progress'), dupes: z.lazy(() => DupesProgressSchema) }), z.object({ ev: z.literal('cleanup-progress'), cleanup: z.lazy(() => CleanupProgressSchema) }), z.object({ ev: z.literal('monitor-sample'), sample: z.lazy(() => MonitorSampleSchema) }), z.object({ ev: z.literal('license-changed'), sku: z.lazy(() => SkuSchema).optional() }), z.object({ ev: z.literal('engine-warning'), warning: z.lazy(() => EngineWarningSchema) })]);

export const EngineFeatureSchema = z.enum(['scanner-win32-nt', 'scanner-posix-dev', 'turbo-ntfs', 'icons', 'recycle-bin', 'monitor']);

export const EntitlementErrorSchema = z.enum(['bad-signature', 'expired', 'stale-iat', 'device-mismatch', 'feature-not-granted', 'malformed', 'revoked']);

export const EntryKindSchema = z.enum(['file', 'dir', 'reparse', 'mount', 'link', 'free-space', 'unknown', 'root']);

export const ExportFormatSchema = z.enum(['csv', 'ndjson']);

export const ExportScopeSchema = z.enum(['full', 'selection', 'filtered']);

export const FilterKindSchema = z.enum(['files', 'dirs', 'both']);

export const FilterPatternKindSchema = z.enum(['glob', 'regex', 'literal']);

export const FilterScopeSchema = z.enum(['all', 'node']);

export const NodeBadgeSchema = z.enum(['junction', 'sparse', 'compressed', 'hardlinked', 'offline', 'package', 'system', 'denied']);

export const PremiumFeatureSchema = z.enum(['turbo', 'dupes', 'cleanup', 'apps', 'snapshots', 'monitor', 'scheduler', 'export']);

export const ScanPhaseSchema = z.enum(['walking', 'aggregating', 'indexing-ext', 'done', 'failed', 'cancelled', 'paused']);

export const ScanStrategySchema = z.enum(['standard', 'turbo']);

export const ScanTargetSchema = z.discriminatedUnion('kind', [z.object({ kind: z.literal('volume'), path: z.string() }), z.object({ kind: z.literal('folder'), paths: z.array(z.string()) }), z.object({ kind: z.literal('home') })]);

export const ScheduleTriggerSchema = z.discriminatedUnion('kind', [z.object({ kind: z.literal('daily'), timeMin: z.number() }), z.object({ kind: z.literal('weekly'), days: z.array(z.number()), timeMin: z.number() }), z.object({ kind: z.literal('at-logon') })]);

export const SizeModeSchema = z.enum(['logical', 'allocated', 'unique']);

export const SkuSchema = z.enum(['yearly', 'lifetime']);

export const SnapshotDeltaKindSchema = z.enum(['grew', 'shrank', 'added', 'removed']);

export const SortDirSchema = z.enum(['asc', 'desc']);

export const SortKeySchema = z.enum(['name', 'logical', 'allocated', 'files', 'folders', 'percent', 'mtime', 'category']);

export const VizModeSchema = z.enum(['treemap', 'sunburst', 'icicle', 'pack', 'mindmap', 'folders', 'table', 'bars', 'age-timeline']);

export const VolumeKindSchema = z.enum(['fixed', 'removable', 'network', 'optical', 'ram', 'unknown']);

export const AppFootprintSchema = z.object({
  token: z.string(),
  total: z.bigint(),
  roots: z.lazy(() => z.array(FootprintRootSchema)),
});

export const AppFootprintQuerySchema = z.object({
  token: z.string(),
});

export const AppRowSchema = z.object({
  token: z.string(),
  name: z.string(),
  publisher: z.string(),
  bytes: z.bigint(),
  source: z.string(),
  uninstallCmd: z.string(),
  installed: z.boolean(),
});

export const AppsPageSchema = z.object({
  apps: z.lazy(() => z.array(AppRowSchema)),
});

export const AppsQuerySchema = z.object({
  includeSystem: z.boolean(),
});

export const ChildrenQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  nodeId: z.lazy(() => NodeIdSchema),
  sort: z.lazy(() => SortSpecSchema),
  offset: z.number(),
  limit: z.number(),
});

export const CleanupItemSchema = z.object({
  nodeId: z.number(),
  path: z.string(),
  bytes: z.bigint(),
  source: z.lazy(() => CleanupSourceSchema),
});

export const CleanupProgressSchema = z.object({
  itemsDone: z.number(),
  itemsTotal: z.number(),
  lastError: z.string().optional(),
});

export const ColorMappingSchema = z.object({
  legend: z.lazy(() => z.array(LegendItemSchema)),
  version: z.number(),
});

export const ColorMappingQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  mode: z.lazy(() => ColorModeSchema),
});

export const CommandSpecSchema = z.object({
  cmd: z.string(),
  req: z.string(),
  res: z.string(),
  premium: z.string().optional(),
});

export const DeviceIdentitySchema = z.object({
  instanceId: z.string(),
  deviceName: z.string(),
  osBuild: z.string(),
  appVersion: z.string(),
});

export const DupesGroupsPageSchema = z.object({
  total: z.number(),
  reclaimable: z.bigint(),
  groups: z.lazy(() => z.array(DuplicateGroupSchema)),
});

export const DupesGroupsQuerySchema = z.object({
  runId: z.number(),
  offset: z.number(),
  limit: z.number(),
});

export const DupesProgressSchema = z.object({
  phase: z.lazy(() => DupesPhaseSchema),
  groupsFound: z.bigint(),
  hashedBytes: z.bigint(),
});

export const DupesRunInfoSchema = z.object({
  runId: z.number(),
});

export const DupesRunQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  minSize: z.bigint(),
  ioCapBps: z.bigint(),
});

export const DuplicateGroupSchema = z.object({
  groupId: z.number(),
  members: z.lazy(() => z.array(DuplicateMemberSchema)),
  reclaimable: z.bigint(),
});

export const DuplicateMemberSchema = z.object({
  nodeId: z.number(),
  path: z.string(),
  bytes: z.bigint(),
  mtime: z.bigint().optional(),
  kept: z.boolean(),
  partialHashed: z.boolean(),
  fullHashed: z.boolean(),
  hardlinkOfKept: z.boolean(),
});

export const EngineWarningSchema = z.object({
  code: z.string(),
  msg: z.string(),
});

export const EntitlementGrantsSchema = z.object({
  sku: z.lazy(() => SkuSchema),
  features: z.lazy(() => z.array(PremiumFeatureSchema)),
  exp: z.lazy(() => UnixMsSchema),
  iat: z.lazy(() => UnixMsSchema),
});

export const ExecuteOutcomeSchema = z.object({
  path: z.string(),
  ok: z.boolean(),
  error: z.string().optional(),
  reclaimed: z.bigint(),
});

export const ExecuteQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  toRecycleBin: z.boolean(),
  acknowledgedBlocks: z.number(),
});

export const ExecuteResultSchema = z.object({
  outcomes: z.lazy(() => z.array(ExecuteOutcomeSchema)),
  reclaimed: z.bigint(),
  failed: z.number(),
});

export const ExpandStatsSchema = z.object({
  files: z.bigint(),
  folders: z.bigint(),
  logical: z.bigint(),
  allocated: z.bigint(),
  unique: z.bigint(),
});

export const ExportQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  format: z.lazy(() => ExportFormatSchema),
  scope: z.lazy(() => ExportScopeSchema),
  dest: z.string(),
});

export const ExportResultSchema = z.object({
  bytesWritten: z.bigint(),
  rows: z.bigint(),
});

export const FilterApplyQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  query: z.lazy(() => FilterQuerySchema),
  scope: z.lazy(() => FilterScopeSchema),
});

export const FilterQuerySchema = z.object({
  name: z.string(),
  categories: z.lazy(() => z.array(CategoryIdSchema)),
  size: z.lazy(() => RangeSchema).optional(),
  age: z.lazy(() => RangeSchema).optional(),
  kind: z.lazy(() => FilterKindSchema),
});

export const FilterResultSchema = z.object({
  filterId: z.number(),
  matched: z.bigint(),
  patternKind: z.lazy(() => FilterPatternKindSchema),
  patternDisplay: z.string(),
});

export const FilterUpdatedSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  filterId: z.number(),
  matchedPage: z.lazy(() => z.array(NodeIdSchema)),
  total: z.bigint(),
  done: z.boolean(),
});

export const FootprintRootSchema = z.object({
  label: z.string(),
  paths: z.array(z.string()),
  bytes: z.bigint(),
});

export const HelloInfoSchema = z.object({
  engineVersion: z.string(),
  rustc: z.string(),
  features: z.lazy(() => z.array(EngineFeatureSchema)),
  volumes: z.lazy(() => z.array(VolumeInfoSchema)),
  isAdmin: z.boolean(),
  platform: z.string(),
});

export const LegendItemSchema = z.object({
  key: z.string(),
  label: z.string(),
  color: z.string(),
  share: z.number(),
});

export const MonitorQuerySchema = z.object({
  periodMs: z.number(),
});

export const MonitorSampleSchema = z.object({
  ts: z.lazy(() => UnixMsSchema),
  cpuTotal: z.number(),
  memTotal: z.bigint(),
  memUsed: z.bigint(),
  diskReadBps: z.bigint(),
  diskWriteBps: z.bigint(),
  processes: z.lazy(() => z.array(ProcessSampleSchema)),
});

export const NodeDeltaSchema = z.object({
  id: z.lazy(() => NodeIdSchema),
  parent: z.lazy(() => NodeIdSchema),
  name: z.string(),
  depth: z.number(),
  logical: z.bigint(),
  allocated: z.bigint(),
  kind: z.number(),
});

export const NodeDetailSchema = z.object({
  id: z.lazy(() => NodeIdSchema),
  path: z.string(),
  name: z.string(),
  kind: z.lazy(() => EntryKindSchema),
  category: z.lazy(() => CategoryIdSchema),
  categoryName: z.string(),
  extension: z.string(),
  logical: z.bigint(),
  allocated: z.bigint(),
  unique: z.bigint(),
  badges: z.lazy(() => z.array(NodeBadgeSchema)),
  created: z.lazy(() => UnixMsSchema).optional(),
  modified: z.lazy(() => UnixMsSchema).optional(),
  accessed: z.lazy(() => UnixMsSchema).optional(),
  attributes: z.number(),
  linkCount: z.number(),
  linkPaths: z.array(z.string()),
  fileId: z.lazy(() => FileIdSchema),
  parent: z.lazy(() => NodeIdSchema),
  depth: z.number(),
  files: z.number(),
  folders: z.number(),
});

export const NodeQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  nodeId: z.lazy(() => NodeIdSchema),
});

export const NodeRowSchema = z.object({
  id: z.lazy(() => NodeIdSchema),
  name: z.string(),
  kind: z.lazy(() => EntryKindSchema),
  logical: z.bigint(),
  allocated: z.bigint(),
  files: z.number(),
  folders: z.number(),
  category: z.lazy(() => CategoryIdSchema),
  badges: z.lazy(() => z.array(NodeBadgeSchema)),
  mtime: z.lazy(() => UnixMsSchema).optional(),
  parentShare: z.number(),
});

export const NodeRowsPageSchema = z.object({
  total: z.number(),
  items: z.lazy(() => z.array(NodeRowSchema)),
});

export const PathErrorSchema = z.object({
  path: z.string(),
  code: z.number(),
  message: z.string(),
  denied: z.boolean(),
});

export const PreflightInfoSchema = z.object({
  target: z.string(),
  readable: z.boolean(),
  requiresElevation: z.boolean(),
  freeBytes: z.bigint(),
  totalBytes: z.bigint(),
  checkedAt: z.lazy(() => UnixMsSchema),
});

export const PreflightQuerySchema = z.object({
  target: z.string(),
});

export const PresetHitSchema = z.object({
  presetId: z.string(),
  name: z.string(),
  safety: z.string(),
  explanation: z.string(),
  paths: z.array(z.string()),
  bytes: z.bigint(),
});

export const PresetHitsPageSchema = z.object({
  hits: z.lazy(() => z.array(PresetHitSchema)),
});

export const ProcessSampleSchema = z.object({
  pid: z.number(),
  name: z.string(),
  cpu: z.number(),
  workingSet: z.bigint(),
  readBps: z.bigint(),
  writeBps: z.bigint(),
  threads: z.number(),
});

export const QueuePageSchema = z.object({
  items: z.lazy(() => z.array(CleanupItemSchema)),
});

export const RangeSchema = z.object({
  min: z.number(),
  max: z.number(),
});

export const ReattachInfoSchema = z.object({
  summary: z.lazy(() => ScanSummarySchema).optional(),
  phase: z.lazy(() => ScanPhaseSchema).optional(),
});

export const RescanQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  nodeId: z.lazy(() => NodeIdSchema),
  options: z.lazy(() => ScanOptionsSchema),
});

export const ResolvePathQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  path: z.string(),
});

export const ResolveResultSchema = z.object({
  nodeId: z.lazy(() => NodeIdSchema).optional(),
});

export const ScanControlQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
});

export const ScanDoneSchema = z.object({
  summary: z.lazy(() => ScanSummarySchema),
});

export const ScanErrorBatchSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  errors: z.lazy(() => z.array(PathErrorSchema)),
});

export const ScanHistoryPageSchema = z.object({
  records: z.lazy(() => z.array(ScanRecordDtoSchema)),
});

export const ScanHistoryQuerySchema = z.object({
  limit: z.number(),
});

export const ScanNodesSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  deltas: z.lazy(() => z.array(NodeDeltaSchema)),
});

export const ScanOptionsSchema = z.object({
  followReparse: z.boolean(),
  sizeMode: z.lazy(() => SizeModeSchema),
  treatPackagesAsNodes: z.boolean(),
  excludePatterns: z.array(z.string()),
});

export const ScanPhaseEventSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  phase: z.lazy(() => ScanPhaseSchema),
  detail: z.string().optional(),
});

export const ScanProgressSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  filesSeen: z.bigint(),
  bytesSeen: z.bigint(),
  dirsSeen: z.bigint(),
  currentPath: z.string(),
  elapsedMs: z.bigint(),
  rateFilesPerSec: z.number(),
});

export const ScanRecordDtoSchema = z.object({
  startedAt: z.bigint(),
  target: z.string(),
  strategy: z.string(),
  files: z.bigint(),
  folders: z.bigint(),
  bytes: z.bigint(),
  durationMs: z.bigint(),
});

export const ScanStartQuerySchema = z.object({
  target: z.lazy(() => ScanTargetSchema),
  strategy: z.lazy(() => ScanStrategySchema),
  options: z.lazy(() => ScanOptionsSchema),
});

export const ScanStartedSchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
});

export const ScanSummarySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  root: z.string(),
  strategy: z.lazy(() => ScanStrategySchema),
  sizeMode: z.lazy(() => SizeModeSchema),
  files: z.bigint(),
  folders: z.bigint(),
  logical: z.bigint(),
  allocated: z.bigint(),
  unique: z.bigint(),
  unknown: z.bigint(),
  free: z.bigint(),
  durationMs: z.bigint(),
  errors: z.number(),
  truncated: z.boolean(),
});

export const ScheduleDigestSchema = z.object({
  target: z.string(),
  beforeMs: z.bigint(),
  afterMs: z.bigint(),
  bytesDelta: z.bigint(),
  filesDelta: z.bigint(),
  top: z.lazy(() => z.array(SnapshotDeltaSchema)),
});

export const ScheduleSpecSchema = z.object({
  id: z.string(),
  label: z.string(),
  target: z.string(),
  trigger: z.lazy(() => ScheduleTriggerSchema),
  enabled: z.boolean(),
  lastRunMs: z.bigint(),
});

export const SchedulerDeleteQuerySchema = z.object({
  id: z.string(),
});

export const SchedulerDigestQuerySchema = z.object({
  target: z.string(),
  limit: z.number(),
});

export const SchedulerListQuerySchema = z.object({});

export const SchedulerPageSchema = z.object({
  schedules: z.lazy(() => z.array(ScheduleSpecSchema)),
  backend: z.string(),
});

export const SchedulerUpsertQuerySchema = z.object({
  spec: z.lazy(() => ScheduleSpecSchema),
  runnerExe: z.string(),
});

export const SnapshotDeltaSchema = z.object({
  pathKey: z.string(),
  path: z.string(),
  kind: z.lazy(() => SnapshotDeltaKindSchema),
  deltaBytes: z.bigint(),
  deltaFiles: z.bigint(),
});

export const SnapshotDiffPageSchema = z.object({
  deltas: z.lazy(() => z.array(SnapshotDeltaSchema)),
  net: z.bigint(),
});

export const SnapshotDiffQuerySchema = z.object({
  before: z.number(),
  after: z.number(),
  floorBytes: z.bigint(),
});

export const SnapshotInfoSchema = z.object({
  id: z.number(),
  rootPath: z.string(),
  createdAt: z.bigint(),
  depth: z.number(),
  files: z.bigint(),
  bytes: z.bigint(),
});

export const SnapshotSaveQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  depth: z.number(),
});

export const SnapshotsPageSchema = z.object({
  snapshots: z.lazy(() => z.array(SnapshotInfoSchema)),
});

export const SnapshotsQuerySchema = z.object({
  root: z.string(),
});

export const SortSpecSchema = z.object({
  key: z.lazy(() => SortKeySchema),
  dir: z.lazy(() => SortDirSchema),
});

export const StageQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  nodeIds: z.lazy(() => z.array(NodeIdSchema)),
  source: z.lazy(() => CleanupSourceSchema),
});

export const StagedTotalsSchema = z.object({
  count: z.number(),
  bytes: z.bigint(),
});

export const TypeColorQuerySchema = z.object({
  key: z.string(),
  color: z.string().optional(),
});

export const TypeRowSchema = z.object({
  key: z.string(),
  extId: z.lazy(() => ExtIdSchema),
  category: z.lazy(() => CategoryIdSchema),
  files: z.bigint(),
  logical: z.bigint(),
  allocated: z.bigint(),
  share: z.number(),
  userColor: z.string().optional(),
});

export const TypesPageSchema = z.object({
  items: z.lazy(() => z.array(TypeRowSchema)),
  totalShare: z.number(),
});

export const TypesQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  sort: z.lazy(() => SortKeySchema),
  dir: z.lazy(() => SortDirSchema),
});

export const UnstageQuerySchema = z.object({
  nodeIds: z.lazy(() => z.array(NodeIdSchema)),
});

export const VerifyTokenQuerySchema = z.object({
  tokenB64: z.string(),
  feature: z.lazy(() => PremiumFeatureSchema),
});

export const ViewportSchema = z.object({
  w: z.number(),
  h: z.number(),
  dpr: z.number(),
});

export const VizLayoutQuerySchema = z.object({
  scanId: z.lazy(() => ScanIdSchema),
  mode: z.lazy(() => VizModeSchema),
  root: z.lazy(() => NodeIdSchema),
  viewport: z.lazy(() => ViewportSchema),
  options: z.lazy(() => VizOptionsSchema),
});

export const VizOptionsSchema = z.object({
  maxTiles: z.number(),
  maxArcs: z.number(),
  maxCircles: z.number(),
  maxGraphNodes: z.number(),
  minTilePx: z.number(),
  minShare: z.number(),
  drawnDepth: z.number(),
  gapPx: z.number(),
  cushionElevation: z.number(),
  cushionFalloff: z.number(),
  labelDensity: z.number(),
  colorMode: z.lazy(() => ColorModeSchema),
  sizeMode: z.lazy(() => SizeModeSchema),
});

export const VolumeInfoSchema = z.object({
  path: z.string(),
  label: z.string(),
  fs: z.string(),
  kind: z.lazy(() => VolumeKindSchema),
  total: z.bigint(),
  free: z.bigint(),
  serial: z.number(),
  hasMedia: z.boolean(),
});

export const VolumesPageSchema = z.object({
  volumes: z.lazy(() => z.array(VolumeInfoSchema)),
});

export const VolumesQuerySchema = z.object({
  refresh: z.boolean(),
});

export const CategoryIdSchema = z.number();
export const ExtIdSchema = z.number();
export const FileIdSchema = z.bigint();
export const FileTimeTicksSchema = z.bigint();
export const NodeIdSchema = z.number();
export const ScanIdSchema = z.number();
export const UnixMsSchema = z.bigint();
