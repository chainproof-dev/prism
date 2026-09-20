//! Command catalogue registry (docs/05 § 3). This table drives TS codegen:
//! the `emit-ts` binary parses it and produces the `CommandsMap` index plus
//! zod request validators. T2 (napi) functions are explicit per command in
//! `prism-core/src/ipc`; T1 (Electron) uses the cmd strings below.

use serde::{Deserialize, Serialize};

/// A command descriptor: `cmd` string, request type, response type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSpec {
    /// Wire command name (`domain:action`).
    pub cmd: &'static str,
    /// Request DTO type name (must exist in this crate; `"()"` = no payload).
    pub req: &'static str,
    /// Response DTO type name (`"()"` = void).
    pub res: &'static str,
    /// Premium entitlement requirement (None = free/parity command).
    pub premium: Option<&'static str>,
}

/// The authoritative command table. Adding a command without a row here is a
/// codegen failure (CI drift check), not a runtime surprise.
pub const COMMANDS: &[CommandSpec] = &[
    // 3.1 lifecycle & system
    CommandSpec {
        cmd: "sys:hello",
        req: "()",
        res: "HelloInfo",
        premium: None,
    },
    CommandSpec {
        cmd: "sys:volumes",
        req: "VolumesQuery",
        res: "VolumesPage",
        premium: None,
    },
    CommandSpec {
        cmd: "sys:preflight",
        req: "PreflightQuery",
        res: "PreflightInfo",
        premium: None,
    },
    // 3.2 scanning
    CommandSpec {
        cmd: "scan:start",
        req: "ScanStartQuery",
        res: "ScanStarted",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:pause",
        req: "ScanControlQuery",
        res: "()",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:resume",
        req: "ScanControlQuery",
        res: "()",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:cancel",
        req: "ScanControlQuery",
        res: "()",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:summary",
        req: "ScanControlQuery",
        res: "ScanSummary",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:rescan-subtree",
        req: "RescanQuery",
        res: "ScanStarted",
        premium: None,
    },
    CommandSpec {
        cmd: "scan:reattach",
        req: "ScanControlQuery",
        res: "ReattachInfo",
        premium: None,
    },
    // 3.3 tree & nodes
    CommandSpec {
        cmd: "tree:children",
        req: "ChildrenQuery",
        res: "NodeRowsPage",
        premium: None,
    },
    CommandSpec {
        cmd: "tree:expand-stats",
        req: "NodeQuery",
        res: "ExpandStats",
        premium: None,
    },
    CommandSpec {
        cmd: "node:detail",
        req: "NodeQuery",
        res: "NodeDetail",
        premium: None,
    },
    CommandSpec {
        cmd: "node:resolve-path",
        req: "ResolvePathQuery",
        res: "ResolveResult",
        premium: None,
    },
    // 3.4 visualization
    CommandSpec {
        cmd: "viz:layout",
        req: "VizLayoutQuery",
        res: "()",
        premium: None,
    }, // binary frame via napi Buffer
    CommandSpec {
        cmd: "viz:color-mapping",
        req: "ColorMappingQuery",
        res: "ColorMapping",
        premium: None,
    },
    // 3.5 types & extensions
    CommandSpec {
        cmd: "types:list",
        req: "TypesQuery",
        res: "TypesPage",
        premium: None,
    },
    CommandSpec {
        cmd: "types:set-color",
        req: "TypeColorQuery",
        res: "()",
        premium: None,
    },
    // 3.6 search & filter
    CommandSpec {
        cmd: "filter:apply",
        req: "FilterApplyQuery",
        res: "FilterResult",
        premium: None,
    },
    CommandSpec {
        cmd: "filter:clear",
        req: "ScanControlQuery",
        res: "()",
        premium: None,
    },
    // 3.7 premium operations (entitlement-gated)
    CommandSpec {
        cmd: "duplicates:run",
        req: "DupesRunQuery",
        res: "DupesRunInfo",
        premium: Some("Dupes"),
    },
    CommandSpec {
        cmd: "duplicates:cancel",
        req: "ScanControlQuery",
        res: "()",
        premium: Some("Dupes"),
    },
    CommandSpec {
        cmd: "duplicates:groups",
        req: "DupesGroupsQuery",
        res: "DupesGroupsPage",
        premium: Some("Dupes"),
    },
    CommandSpec {
        cmd: "cleanup:presets-scan",
        req: "ScanControlQuery",
        res: "PresetHitsPage",
        premium: None,
    },
    CommandSpec {
        cmd: "cleanup:stage",
        req: "StageQuery",
        res: "StagedTotals",
        premium: None,
    },
    CommandSpec {
        cmd: "cleanup:unstage",
        req: "UnstageQuery",
        res: "StagedTotals",
        premium: None,
    },
    CommandSpec {
        cmd: "cleanup:queue",
        req: "ScanControlQuery",
        res: "QueuePage",
        premium: None,
    },
    CommandSpec {
        cmd: "cleanup:execute",
        req: "ExecuteQuery",
        res: "ExecuteResult",
        premium: Some("Cleanup"),
    },
    CommandSpec {
        cmd: "apps:list",
        req: "AppsQuery",
        res: "AppsPage",
        premium: Some("Apps"),
    },
    CommandSpec {
        cmd: "apps:footprint",
        req: "AppFootprintQuery",
        res: "AppFootprint",
        premium: Some("Apps"),
    },
    CommandSpec {
        cmd: "apps:leftovers",
        req: "ScanControlQuery",
        res: "AppsPage",
        premium: Some("Apps"),
    },
    CommandSpec {
        cmd: "snapshots:save",
        req: "SnapshotSaveQuery",
        res: "SnapshotInfo",
        premium: Some("Snapshots"),
    },
    CommandSpec {
        cmd: "snapshots:list",
        req: "SnapshotsQuery",
        res: "SnapshotsPage",
        premium: Some("Snapshots"),
    },
    CommandSpec {
        cmd: "snapshots:diff",
        req: "SnapshotDiffQuery",
        res: "SnapshotDiffPage",
        premium: Some("Snapshots"),
    },
    CommandSpec {
        cmd: "monitor:start",
        req: "MonitorQuery",
        res: "()",
        premium: Some("Monitor"),
    },
    CommandSpec {
        cmd: "monitor:stop",
        req: "MonitorQuery",
        res: "()",
        premium: Some("Monitor"),
    },
    CommandSpec {
        cmd: "export:scan",
        req: "ExportQuery",
        res: "ExportResult",
        premium: Some("Export"),
    },
    // 3.8 licensing (main-handled; engine only verifies tokens)
    CommandSpec {
        cmd: "lic:verify-token",
        req: "VerifyTokenQuery",
        res: "EntitlementGrants",
        premium: None,
    },
];

/// Helper DTOs referenced only by the table above live here so the emitter
/// finds them alongside the core modules.
/// `sys:volumes` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumesQuery {
    /// Force a refresh (else cached ≤ 2 s).
    pub refresh: bool,
}

/// `sys:volumes` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumesPage {
    /// Enumerated volumes.
    pub volumes: Vec<crate::sys::VolumeInfo>,
}

/// `sys:preflight` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightQuery {
    /// Candidate target path.
    pub target: String,
}

/// `scan:start` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStartQuery {
    /// What to scan.
    pub target: crate::scan::ScanTarget,
    /// Strategy (never silently substituted — ADR-06).
    pub strategy: crate::scan::ScanStrategy,
    /// Options.
    pub options: crate::scan::ScanOptions,
}

/// `scan:start` response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStarted {
    /// New scan lease id.
    pub scan_id: crate::ids::ScanId,
}

/// `scan:*` control request (pause/resume/cancel/summary/clear/queue).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanControlQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
}

/// `scan:rescan-subtree` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RescanQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Subtree root node.
    pub node_id: crate::ids::NodeId,
    /// Options (may differ from original scan).
    pub options: crate::scan::ScanOptions,
}

/// `scan:reattach` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReattachInfo {
    /// Last summary if the scan already finished.
    pub summary: Option<crate::scan::ScanSummary>,
    /// Current phase (None = unknown lease).
    pub phase: Option<crate::scan::ScanPhase>,
}

/// `node:detail` / `tree:expand-stats` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Node id.
    pub node_id: crate::ids::NodeId,
}

/// `node:resolve-path` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvePathQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Absolute path to locate.
    pub path: String,
}

/// `node:resolve-path` response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    /// Found node id (null = not under this scan).
    pub node_id: Option<crate::ids::NodeId>,
}

/// `viz:color-mapping` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMappingQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Color mode.
    pub mode: crate::viz::ColorMode,
}

/// `types:list` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypesQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Sort key.
    pub sort: crate::ids::SortKey,
    /// Sort direction.
    pub dir: crate::ids::SortDir,
}

/// `types:set-color` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeColorQuery {
    /// Extension key.
    pub key: String,
    /// `#RRGGBB` or null to inherit.
    pub color: Option<String>,
}

/// `duplicates:run` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupesRunQuery {
    /// Scan lease to operate on.
    pub scan_id: crate::ids::ScanId,
    /// Minimum size to consider (bytes).
    pub min_size: u64,
    /// IO cap bytes/s (0 = uncapped).
    pub io_cap_bps: u64,
}

/// `duplicates:run` response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupesRunInfo {
    /// Run id.
    pub run_id: u32,
}

/// `duplicates:groups` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupesGroupsQuery {
    /// Run id.
    pub run_id: u32,
    /// Page offset.
    pub offset: u32,
    /// Page size.
    pub limit: u32,
}

/// `duplicates:groups` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupesGroupsPage {
    /// Total groups.
    pub total: u32,
    /// Total reclaimable bytes.
    pub reclaimable: u64,
    /// This page.
    pub groups: Vec<crate::types_list::DuplicateGroup>,
}

/// Preset hit row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetHit {
    /// Preset id.
    pub preset_id: String,
    /// Display name.
    pub name: String,
    /// Safety class (drives ledger confirmations).
    pub safety: String,
    /// Explanation line (localized renderer-side by id).
    pub explanation: String,
    /// Hit paths.
    pub paths: Vec<String>,
    /// Total bytes.
    pub bytes: u64,
}

/// `cleanup:presets-scan` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetHitsPage {
    /// Hits grouped by preset.
    pub hits: Vec<PresetHit>,
}

/// `cleanup:stage` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageQuery {
    /// Scan lease (0 = non-scan items).
    pub scan_id: crate::ids::ScanId,
    /// Nodes to stage.
    pub node_ids: Vec<crate::ids::NodeId>,
    /// Source label.
    pub source: crate::types_list::CleanupSource,
}

/// `cleanup:unstage` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnstageQuery {
    /// Nodes to unstage (empty = clear all).
    pub node_ids: Vec<crate::ids::NodeId>,
}

/// Staged totals response.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedTotals {
    /// Item count.
    pub count: u32,
    /// Total bytes.
    pub bytes: u64,
}

/// `cleanup:queue` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueuePage {
    /// Staged items.
    pub items: Vec<crate::types_list::CleanupItem>,
}

/// `cleanup:execute` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Recycle (true) or permanent (false).
    pub to_recycle_bin: bool,
    /// Block-list acknowledgements provided (count of typed confirmations).
    pub acknowledged_blocks: u32,
}

/// Per-item execution outcome.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteOutcome {
    /// Path.
    pub path: String,
    /// Succeeded.
    pub ok: bool,
    /// Error message when failed.
    pub error: Option<String>,
    /// Reclaimed bytes (0 on failure).
    pub reclaimed: u64,
}

/// `cleanup:execute` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecuteResult {
    /// Per-item outcomes (fail-loud ledger, never silent partials).
    pub outcomes: Vec<ExecuteOutcome>,
    /// Total reclaimed.
    pub reclaimed: u64,
    /// Failures count.
    pub failed: u32,
}

/// `apps:list` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsQuery {
    /// Include system/Store apps.
    pub include_system: bool,
}

/// Installed app row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppRow {
    /// Normalized app token.
    pub token: String,
    /// Display name.
    pub name: String,
    /// Publisher.
    pub publisher: String,
    /// Estimated total footprint.
    pub bytes: u64,
    /// Source (registry/steam/store).
    pub source: String,
    /// Uninstall command ("" = none).
    pub uninstall_cmd: String,
    /// Currently installed (false = leftovers-only entry).
    pub installed: bool,
}

/// `apps:list` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppsPage {
    /// Apps.
    pub apps: Vec<AppRow>,
}

/// `apps:footprint` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppFootprintQuery {
    /// App token.
    pub token: String,
}

/// Footprint root row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FootprintRoot {
    /// Root label (Program files, AppData…).
    pub label: String,
    /// Evidence paths.
    pub paths: Vec<String>,
    /// Bytes under this root.
    pub bytes: u64,
}

/// `apps:footprint` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppFootprint {
    /// App token.
    pub token: String,
    /// Total bytes.
    pub total: u64,
    /// Per-root breakdown (evidence list).
    pub roots: Vec<FootprintRoot>,
}

/// `snapshots:save` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSaveQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Capture depth (default 6).
    pub depth: u8,
}

/// Snapshot metadata row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotInfo {
    /// Snapshot id.
    pub id: u32,
    /// Scan root path.
    pub root_path: String,
    /// Captured at (unix ms).
    pub created_at: i64,
    /// Capture depth.
    pub depth: u8,
    /// Total files captured.
    pub files: u64,
    /// Total bytes captured.
    pub bytes: u64,
}

/// `snapshots:list` request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsQuery {
    /// Filter by root path (empty = all).
    pub root: String,
}

/// `snapshots:list` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotsPage {
    /// Snapshots (newest first).
    pub snapshots: Vec<SnapshotInfo>,
}

/// `snapshots:diff` request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiffQuery {
    /// BEFORE snapshot id.
    pub before: u32,
    /// AFTER snapshot id.
    pub after: u32,
    /// Significance floor bytes (default 10 MB).
    pub floor_bytes: u64,
}

/// Diff row kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotDeltaKind {
    /// Grew.
    Grew,
    /// Shrank.
    Shrank,
    /// Added.
    Added,
    /// Removed.
    Removed,
}

/// Diff row.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDelta {
    /// Path key (hash-based, privacy-safe).
    pub path_key: String,
    /// Display path (depth-capped).
    pub path: String,
    /// Kind.
    pub kind: SnapshotDeltaKind,
    /// Byte delta (signed).
    pub delta_bytes: i64,
    /// File count delta.
    pub delta_files: i64,
}

/// `snapshots:diff` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiffPage {
    /// Rows ≥ floor.
    pub deltas: Vec<SnapshotDelta>,
    /// Net change bytes.
    pub net: i64,
}

/// `monitor:start/stop` request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorQuery {
    /// Sampling period ms (min 250).
    pub period_ms: u32,
}

/// `export:scan` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportQuery {
    /// Scan lease.
    pub scan_id: crate::ids::ScanId,
    /// Format.
    pub format: ExportFormat,
    /// Scope.
    pub scope: ExportScope,
    /// Destination path (main resolves the save dialog).
    pub dest: String,
}

/// Export formats (docs/07 § 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportFormat {
    /// RFC 4180 CSV, UTF-8 BOM.
    Csv,
    /// NDJSON lines.
    Ndjson,
}

/// Export scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportScope {
    /// Whole scan.
    Full,
    /// Current selection.
    Selection,
    /// Current filter results.
    Filtered,
}

/// `export:scan` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// Bytes written.
    pub bytes_written: u64,
    /// Rows written.
    pub rows: u64,
}

/// `lic:verify-token` request (main → engine only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyTokenQuery {
    /// Base64 CBOR+Ed25519 token.
    pub token_b64: String,
    /// Feature being requested.
    pub feature: crate::licensing::PremiumFeature,
}
