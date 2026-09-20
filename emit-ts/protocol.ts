// GENERATED from crates/prism-types — DO NOT EDIT (PRISM-IPC-001).
// Regenerate: cargo run -p prism-types --features emit -- emit-ts <dir>

// Wire conventions (AMM-003): camelCase fields, kebab enum values,
// u64 byte counts as JSON numbers (exact ≤ 2^53).

export type CategoryId = number;
export type ExtId = number;
export type FileId = number;
export type FileTimeTicks = number;
export type NodeId = number;
export type ScanId = number;
export type UnixMs = number;

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

export type SizeMode = 'logical' | 'allocated' | 'unique';

export type Sku = 'yearly' | 'lifetime';

export type SnapshotDeltaKind = 'grew' | 'shrank' | 'added' | 'removed';

export type SortDir = 'asc' | 'desc';

export type SortKey = 'name' | 'logical' | 'allocated' | 'files' | 'folders' | 'percent' | 'mtime' | 'category';

export type VizMode = 'treemap' | 'sunburst' | 'icicle' | 'pack' | 'mindmap' | 'folders' | 'table' | 'bars' | 'age-timeline';

export type VolumeKind = 'fixed' | 'removable' | 'network' | 'optical' | 'ram' | 'unknown';

export interface AppFootprint {
  token: string;
  total: number;
  roots: FootprintRoot[];
}

export interface AppFootprintQuery {
  token: string;
}

export interface AppRow {
  token: string;
  name: string;
  publisher: string;
  bytes: number;
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
  bytes: number;
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
  cmd: str;
  req: str;
  res: str;
  premium?: str;
}

export interface DeviceIdentity {
  instanceId: string;
  deviceName: string;
  osBuild: string;
  appVersion: string;
}

export interface DupesGroupsPage {
  total: number;
  reclaimable: number;
  groups: DuplicateGroup[];
}

export interface DupesGroupsQuery {
  runId: number;
  offset: number;
  limit: number;
}

export interface DupesProgress {
  phase: DupesPhase;
  groupsFound: number;
  hashedBytes: number;
}

export interface DupesRunInfo {
  runId: number;
}

export interface DupesRunQuery {
  scanId: ScanId;
  minSize: number;
  ioCapBps: number;
}

export interface DuplicateGroup {
  groupId: number;
  members: DuplicateMember[];
  reclaimable: number;
}

export interface DuplicateMember {
  nodeId: number;
  path: string;
  bytes: number;
  mtime?: number;
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
  reclaimed: number;
}

export interface ExecuteQuery {
  scanId: ScanId;
  toRecycleBin: boolean;
  acknowledgedBlocks: number;
}

export interface ExecuteResult {
  outcomes: ExecuteOutcome[];
  reclaimed: number;
  failed: number;
}

export interface ExpandStats {
  files: number;
  folders: number;
  logical: number;
  allocated: number;
  unique: number;
}

export interface ExportQuery {
  scanId: ScanId;
  format: ExportFormat;
  scope: ExportScope;
  dest: string;
}

export interface ExportResult {
  bytesWritten: number;
  rows: number;
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
  matched: number;
  patternKind: FilterPatternKind;
  patternDisplay: string;
}

export interface FilterUpdated {
  scanId: ScanId;
  filterId: number;
  matchedPage: NodeId[];
  total: number;
  done: boolean;
}

export interface FootprintRoot {
  label: string;
  paths: string[];
  bytes: number;
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
  memTotal: number;
  memUsed: number;
  diskReadBps: number;
  diskWriteBps: number;
  processes: ProcessSample[];
}

export interface NodeDelta {
  id: NodeId;
  parent: NodeId;
  name: string;
  depth: number;
  logical: number;
  allocated: number;
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
  logical: number;
  allocated: number;
  unique: number;
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
  logical: number;
  allocated: number;
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
  freeBytes: number;
  totalBytes: number;
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
  bytes: number;
}

export interface PresetHitsPage {
  hits: PresetHit[];
}

export interface ProcessSample {
  pid: number;
  name: string;
  cpu: number;
  workingSet: number;
  readBps: number;
  writeBps: number;
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
  filesSeen: number;
  bytesSeen: number;
  dirsSeen: number;
  currentPath: string;
  elapsedMs: number;
  rateFilesPerSec: number;
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
  files: number;
  folders: number;
  logical: number;
  allocated: number;
  unique: number;
  unknown: number;
  free: number;
  durationMs: number;
  errors: number;
  truncated: boolean;
}

export interface SnapshotDelta {
  pathKey: string;
  path: string;
  kind: SnapshotDeltaKind;
  deltaBytes: number;
  deltaFiles: number;
}

export interface SnapshotDiffPage {
  deltas: SnapshotDelta[];
  net: number;
}

export interface SnapshotDiffQuery {
  before: number;
  after: number;
  floorBytes: number;
}

export interface SnapshotInfo {
  id: number;
  rootPath: string;
  createdAt: number;
  depth: number;
  files: number;
  bytes: number;
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
  bytes: number;
}

export interface TypeColorQuery {
  key: string;
  color?: string;
}

export interface TypeRow {
  key: string;
  extId: ExtId;
  category: CategoryId;
  files: number;
  logical: number;
  allocated: number;
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
  total: number;
  free: number;
  serial: number;
  hasMedia: boolean;
}

export interface VolumesPage {
  volumes: VolumeInfo[];
}

export interface VolumesQuery {
  refresh: boolean;
}

