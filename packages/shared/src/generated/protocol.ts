// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

// Wire conventions (AMM-003): camelCase fields, kebab enum values,
// u64 byte counts as JSON numbers (exact ≤ 2^53).

export type CategoryId = number;
export type ExtId = number;
export type FileId = bigint;
export type FileTimeTicks = bigint;
export type NodeId = number;
export type ScanId = number;
export type UnixMs = bigint;

export type CleanupSource = 'manual' | 'preset' | 'duplicate' | 'stale' | 'leftover';

export type ColorMode = 'type' | 'branch' | 'age';

export type DupesPhase = 'grouping-sizes' | 'grouping-ext' | 'fingerprinting' | 'full-hashing' | 'hardlink-collapse' | 'done';

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
  | { ev: 'license-changed'; sku?: Sku }
  | { ev: 'engine-warning'; warning: EngineWarning }
  ;

export type EngineFeature = 'scanner-win32-nt' | 'scanner-posix-dev' | 'turbo-ntfs' | 'icons' | 'recycle-bin' | 'monitor';

export type EntitlementError = 'bad-signature' | 'expired' | 'stale-iat' | 'device-mismatch' | 'feature-not-granted' | 'malformed' | 'revoked';

export type EntryKind = 'file' | 'dir' | 'reparse' | 'mount' | 'link' | 'free-space' | 'unknown' | 'root';

export type ExportFormat = 'csv' | 'ndjson';

export type ExportScope = 'full' | 'selection' | 'filtered';

export type FilterKind = 'files' | 'dirs' | 'both';

export type FilterPatternKind = 'glob' | 'regex' | 'literal';

export type FilterScope = 'all' | 'node';

export type NodeBadge = 'junction' | 'sparse' | 'compressed' | 'hardlinked' | 'offline' | 'package' | 'system' | 'denied';

export type PremiumFeature = 'turbo' | 'dupes' | 'cleanup' | 'apps' | 'snapshots' | 'monitor' | 'scheduler' | 'export';

export type ScanPhase = 'walking' | 'aggregating' | 'indexing-ext' | 'done' | 'failed' | 'cancelled' | 'paused';

export type ScanStrategy = 'standard' | 'turbo';

export type ScanTarget =
  | { kind: 'volume'; path: string }
  | { kind: 'folder'; paths: string[] }
  | { kind: 'home' }
  ;

export type ScheduleTrigger =
  | { kind: 'daily'; timeMin: number }
  | { kind: 'weekly'; days: number[]; timeMin: number }
  | { kind: 'at-logon' }
  ;

export type SizeMode = 'logical' | 'allocated' | 'unique';

export type Sku = 'yearly' | 'lifetime';

export type SnapshotDeltaKind = 'grew' | 'shrank' | 'added' | 'removed';

export type SortDir = 'asc' | 'desc';

export type SortKey = 'name' | 'logical' | 'allocated' | 'files' | 'folders' | 'percent' | 'mtime' | 'category';

export type VizMode = 'treemap' | 'sunburst' | 'icicle' | 'pack' | 'mindmap' | 'folders' | 'table' | 'bars' | 'age-timeline';

export type VolumeKind = 'fixed' | 'removable' | 'network' | 'optical' | 'ram' | 'unknown';

export interface AppFootprint {
  token: string;
  total: bigint;
  roots: FootprintRoot[];
}

export interface AppFootprintQuery {
  token: string;
}

export interface AppRow {
  token: string;
  name: string;
  publisher: string;
  bytes: bigint;
  source: string;
  uninstallCmd: string;
  installed: boolean;
}

export interface AppsPage {
  apps: AppRow[];
}

export interface AppsQuery {
  includeSystem: boolean;
}

export interface ChildrenQuery {
  scanId: ScanId;
  nodeId: NodeId;
  sort: SortSpec;
  offset: number;
  limit: number;
}

export interface CleanupItem {
  nodeId: number;
  path: string;
  bytes: bigint;
  source: CleanupSource;
}

export interface CleanupProgress {
  itemsDone: number;
  itemsTotal: number;
  lastError?: string;
}

export interface ColorMapping {
  legend: LegendItem[];
  version: number;
}

export interface ColorMappingQuery {
  scanId: ScanId;
  mode: ColorMode;
}

export interface CommandSpec {
  cmd: string;
  req: string;
  res: string;
  premium?: string;
}

export interface DeviceIdentity {
  instanceId: string;
  deviceName: string;
  osBuild: string;
  appVersion: string;
}

export interface DupesGroupsPage {
  total: number;
  reclaimable: bigint;
  groups: DuplicateGroup[];
}

export interface DupesGroupsQuery {
  runId: number;
  offset: number;
  limit: number;
}

export interface DupesProgress {
  phase: DupesPhase;
  groupsFound: bigint;
  hashedBytes: bigint;
}

export interface DupesRunInfo {
  runId: number;
}

export interface DupesRunQuery {
  scanId: ScanId;
  minSize: bigint;
  ioCapBps: bigint;
}

export interface DuplicateGroup {
  groupId: number;
  members: DuplicateMember[];
  reclaimable: bigint;
}

export interface DuplicateMember {
  nodeId: number;
  path: string;
  bytes: bigint;
  mtime?: bigint;
  kept: boolean;
  partialHashed: boolean;
  fullHashed: boolean;
  hardlinkOfKept: boolean;
}

export interface EngineWarning {
  code: string;
  msg: string;
}

export interface EntitlementGrants {
  sku: Sku;
  features: PremiumFeature[];
  exp: UnixMs;
  iat: UnixMs;
}

export interface ExecuteOutcome {
  path: string;
  ok: boolean;
  error?: string;
  reclaimed: bigint;
}

export interface ExecuteQuery {
  scanId: ScanId;
  toRecycleBin: boolean;
  acknowledgedBlocks: number;
}

export interface ExecuteResult {
  outcomes: ExecuteOutcome[];
  reclaimed: bigint;
  failed: number;
}

export interface ExpandStats {
  files: bigint;
  folders: bigint;
  logical: bigint;
  allocated: bigint;
  unique: bigint;
}

export interface ExportQuery {
  scanId: ScanId;
  format: ExportFormat;
  scope: ExportScope;
  dest: string;
}

export interface ExportResult {
  bytesWritten: bigint;
  rows: bigint;
}

export interface FilterApplyQuery {
  scanId: ScanId;
  query: FilterQuery;
  scope: FilterScope;
}

export interface FilterQuery {
  name: string;
  categories: CategoryId[];
  size?: Range;
  age?: Range;
  kind: FilterKind;
}

export interface FilterResult {
  filterId: number;
  matched: bigint;
  patternKind: FilterPatternKind;
  patternDisplay: string;
}

export interface FilterUpdated {
  scanId: ScanId;
  filterId: number;
  matchedPage: NodeId[];
  total: bigint;
  done: boolean;
}

export interface FootprintRoot {
  label: string;
  paths: string[];
  bytes: bigint;
}

export interface HelloInfo {
  engineVersion: string;
  rustc: string;
  features: EngineFeature[];
  volumes: VolumeInfo[];
  isAdmin: boolean;
  platform: string;
}

export interface LegendItem {
  key: string;
  label: string;
  color: string;
  share: number;
}

export interface MonitorQuery {
  periodMs: number;
}

export interface MonitorSample {
  ts: UnixMs;
  cpuTotal: number;
  memTotal: bigint;
  memUsed: bigint;
  diskReadBps: bigint;
  diskWriteBps: bigint;
  processes: ProcessSample[];
}

export interface NodeDelta {
  id: NodeId;
  parent: NodeId;
  name: string;
  depth: number;
  logical: bigint;
  allocated: bigint;
  kind: number;
}

export interface NodeDetail {
  id: NodeId;
  path: string;
  name: string;
  kind: EntryKind;
  category: CategoryId;
  categoryName: string;
  extension: string;
  logical: bigint;
  allocated: bigint;
  unique: bigint;
  badges: NodeBadge[];
  created?: UnixMs;
  modified?: UnixMs;
  accessed?: UnixMs;
  attributes: number;
  linkCount: number;
  linkPaths: string[];
  fileId: FileId;
  parent: NodeId;
  depth: number;
  files: number;
  folders: number;
}

export interface NodeQuery {
  scanId: ScanId;
  nodeId: NodeId;
}

export interface NodeRow {
  id: NodeId;
  name: string;
  kind: EntryKind;
  logical: bigint;
  allocated: bigint;
  files: number;
  folders: number;
  category: CategoryId;
  badges: NodeBadge[];
  mtime?: UnixMs;
  parentShare: number;
}

export interface NodeRowsPage {
  total: number;
  items: NodeRow[];
}

export interface PathError {
  path: string;
  code: number;
  message: string;
  denied: boolean;
}

export interface PreflightInfo {
  target: string;
  readable: boolean;
  requiresElevation: boolean;
  freeBytes: bigint;
  totalBytes: bigint;
  checkedAt: UnixMs;
}

export interface PreflightQuery {
  target: string;
}

export interface PresetHit {
  presetId: string;
  name: string;
  safety: string;
  explanation: string;
  paths: string[];
  bytes: bigint;
}

export interface PresetHitsPage {
  hits: PresetHit[];
}

export interface ProcessSample {
  pid: number;
  name: string;
  cpu: number;
  workingSet: bigint;
  readBps: bigint;
  writeBps: bigint;
  threads: number;
}

export interface QueuePage {
  items: CleanupItem[];
}

export interface Range {
  min: number;
  max: number;
}

export interface ReattachInfo {
  summary?: ScanSummary;
  phase?: ScanPhase;
}

export interface RescanQuery {
  scanId: ScanId;
  nodeId: NodeId;
  options: ScanOptions;
}

export interface ResolvePathQuery {
  scanId: ScanId;
  path: string;
}

export interface ResolveResult {
  nodeId?: NodeId;
}

export interface ScanControlQuery {
  scanId: ScanId;
}

export interface ScanDone {
  summary: ScanSummary;
}

export interface ScanErrorBatch {
  scanId: ScanId;
  errors: PathError[];
}

export interface ScanHistoryPage {
  records: ScanRecordDto[];
}

export interface ScanHistoryQuery {
  limit: number;
}

export interface ScanNodes {
  scanId: ScanId;
  deltas: NodeDelta[];
}

export interface ScanOptions {
  followReparse: boolean;
  sizeMode: SizeMode;
  treatPackagesAsNodes: boolean;
  excludePatterns: string[];
}

export interface ScanPhaseEvent {
  scanId: ScanId;
  phase: ScanPhase;
  detail?: string;
}

export interface ScanProgress {
  scanId: ScanId;
  filesSeen: bigint;
  bytesSeen: bigint;
  dirsSeen: bigint;
  currentPath: string;
  elapsedMs: bigint;
  rateFilesPerSec: number;
}

export interface ScanRecordDto {
  startedAt: bigint;
  target: string;
  strategy: string;
  files: bigint;
  folders: bigint;
  bytes: bigint;
  durationMs: bigint;
}

export interface ScanStartQuery {
  target: ScanTarget;
  strategy: ScanStrategy;
  options: ScanOptions;
}

export interface ScanStarted {
  scanId: ScanId;
}

export interface ScanSummary {
  scanId: ScanId;
  root: string;
  strategy: ScanStrategy;
  sizeMode: SizeMode;
  files: bigint;
  folders: bigint;
  logical: bigint;
  allocated: bigint;
  unique: bigint;
  unknown: bigint;
  free: bigint;
  durationMs: bigint;
  errors: number;
  truncated: boolean;
}

export interface ScheduleDigest {
  target: string;
  beforeMs: bigint;
  afterMs: bigint;
  bytesDelta: bigint;
  filesDelta: bigint;
  top: SnapshotDelta[];
}

export interface ScheduleSpec {
  id: string;
  label: string;
  target: string;
  trigger: ScheduleTrigger;
  enabled: boolean;
  lastRunMs: bigint;
}

export interface SchedulerDeleteQuery {
  id: string;
}

export interface SchedulerDigestQuery {
  target: string;
  limit: number;
}

export interface SchedulerListQuery {}

export interface SchedulerPage {
  schedules: ScheduleSpec[];
  backend: string;
}

export interface SchedulerUpsertQuery {
  spec: ScheduleSpec;
  runnerExe: string;
}

export interface SnapshotDelta {
  pathKey: string;
  path: string;
  kind: SnapshotDeltaKind;
  deltaBytes: bigint;
  deltaFiles: bigint;
}

export interface SnapshotDiffPage {
  deltas: SnapshotDelta[];
  net: bigint;
}

export interface SnapshotDiffQuery {
  before: number;
  after: number;
  floorBytes: bigint;
}

export interface SnapshotInfo {
  id: number;
  rootPath: string;
  createdAt: bigint;
  depth: number;
  files: bigint;
  bytes: bigint;
}

export interface SnapshotSaveQuery {
  scanId: ScanId;
  depth: number;
}

export interface SnapshotsPage {
  snapshots: SnapshotInfo[];
}

export interface SnapshotsQuery {
  root: string;
}

export interface SortSpec {
  key: SortKey;
  dir: SortDir;
}

export interface StageQuery {
  scanId: ScanId;
  nodeIds: NodeId[];
  source: CleanupSource;
}

export interface StagedTotals {
  count: number;
  bytes: bigint;
}

export interface TypeColorQuery {
  key: string;
  color?: string;
}

export interface TypeRow {
  key: string;
  extId: ExtId;
  category: CategoryId;
  files: bigint;
  logical: bigint;
  allocated: bigint;
  share: number;
  userColor?: string;
}

export interface TypesPage {
  items: TypeRow[];
  totalShare: number;
}

export interface TypesQuery {
  scanId: ScanId;
  sort: SortKey;
  dir: SortDir;
}

export interface UnstageQuery {
  nodeIds: NodeId[];
}

export interface VerifyTokenQuery {
  tokenB64: string;
  feature: PremiumFeature;
}

export interface Viewport {
  w: number;
  h: number;
  dpr: number;
}

export interface VizLayoutQuery {
  scanId: ScanId;
  mode: VizMode;
  root: NodeId;
  viewport: Viewport;
  options: VizOptions;
}

export interface VizOptions {
  maxTiles: number;
  maxArcs: number;
  maxCircles: number;
  maxGraphNodes: number;
  minTilePx: number;
  minShare: number;
  drawnDepth: number;
  gapPx: number;
  cushionElevation: number;
  cushionFalloff: number;
  labelDensity: number;
  colorMode: ColorMode;
  sizeMode: SizeMode;
}

export interface VolumeInfo {
  path: string;
  label: string;
  fs: string;
  kind: VolumeKind;
  total: bigint;
  free: bigint;
  serial: number;
  hasMedia: boolean;
}

export interface VolumesPage {
  volumes: VolumeInfo[];
}

export interface VolumesQuery {
  refresh: boolean;
}

