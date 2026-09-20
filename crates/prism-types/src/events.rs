//! Event catalogue payloads (docs/05 § 4). All events are append-only facts;
//! the drain thread batches ≤ 512 events / ≤ 16 ms before the TSFN hop.

use serde::{Deserialize, Serialize};

use crate::ids::{NodeId, ScanId, UnixMs};
use crate::licensing::Sku;
use crate::scan::{PathError, ScanPhase, ScanSummary};
use crate::types_list::DupesPhase;

/// A streamed node delta (live tree building, parity-SCN-02).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDelta {
    /// Arena node id.
    pub id: NodeId,
    /// Parent node id.
    pub parent: NodeId,
    /// Name (display).
    pub name: String,
    /// Depth from root (root = 0).
    pub depth: u8,
    /// Running subtree logical size (grows during walk).
    pub logical: u64,
    /// Running subtree allocated size.
    pub allocated: u64,
    /// Entry kind wire form.
    pub kind: u8,
}

/// `scan:progress` payload (coalesced ≤ 4 Hz).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Files seen so far.
    pub files_seen: u64,
    /// Bytes seen so far (active size mode).
    pub bytes_seen: u64,
    /// Directories seen so far.
    pub dirs_seen: u64,
    /// Current frontier path (display, redacted in logs).
    pub current_path: String,
    /// Elapsed ms since start.
    pub elapsed_ms: u64,
    /// Files/sec rolling rate.
    pub rate_files_per_sec: f64,
}

/// `scan:nodes` payload (batched deltas).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanNodes {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Batch of deltas (≤ 512).
    pub deltas: Vec<NodeDelta>,
}

/// `scan:phase` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPhaseEvent {
    /// Scan lease.
    pub scan_id: ScanId,
    /// New phase.
    pub phase: ScanPhase,
    /// Optional detail line.
    pub detail: Option<String>,
}

/// `scan:error-batch` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanErrorBatch {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Errors in this batch.
    pub errors: Vec<PathError>,
}

/// `scan:done` payload (once per scan).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanDone {
    /// Final summary.
    pub summary: ScanSummary,
}

/// `filter:updated` payload (paged matched ids).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterUpdated {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Filter id.
    pub filter_id: u32,
    /// This page of matched node ids.
    pub matched_page: Vec<NodeId>,
    /// Total matches.
    pub total: u64,
    /// Streaming done flag.
    pub done: bool,
}

/// `dupes:progress` payload.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DupesProgress {
    /// Current pipeline phase.
    pub phase: DupesPhase,
    /// Groups found so far.
    pub groups_found: u64,
    /// Bytes hashed so far.
    pub hashed_bytes: u64,
}

/// `cleanup:progress` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupProgress {
    /// Items completed.
    pub items_done: u32,
    /// Items total.
    pub items_total: u32,
    /// Last per-item error (typed, surfaced in ledger).
    pub last_error: Option<String>,
}

/// `monitor:sample` payload (1 Hz).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSample {
    /// Sample timestamp (unix ms).
    pub ts: UnixMs,
    /// Total CPU utilization 0–1.
    pub cpu_total: f32,
    /// Total physical memory bytes.
    pub mem_total: u64,
    /// Used memory bytes.
    pub mem_used: u64,
    /// Disk read bytes/s across all volumes.
    pub disk_read_bps: u64,
    /// Disk write bytes/s.
    pub disk_write_bps: u64,
    /// Top processes by the active sort.
    pub processes: Vec<ProcessSample>,
}

/// One process row (monitor table).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSample {
    /// PID.
    pub pid: u32,
    /// Process name.
    pub name: String,
    /// CPU utilization 0–1 (delta-based).
    pub cpu: f32,
    /// Working set bytes.
    pub working_set: u64,
    /// Read bytes/s.
    pub read_bps: u64,
    /// Write bytes/s.
    pub write_bps: u64,
    /// Thread count.
    pub threads: u32,
}

/// `engine:warning` payload (rare, diagnostic).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineWarning {
    /// Stable warning code.
    pub code: String,
    /// Human message (no paths).
    pub msg: String,
}

/// The single event envelope that crosses T2 (TSFN) as a JSON-serialized batch
/// (`Vec<EngineEvent>` → one string per drain batch — see
/// docs/amendments/AMM-003-event-transport.md); main fans out as `prism:events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "kebab-case")]
pub enum EngineEvent {
    /// Progress counters (coalesced).
    ScanProgress {
        /// Payload.
        progress: ScanProgress,
    },
    /// Batched node deltas.
    ScanNodes {
        /// Payload.
        nodes: ScanNodes,
    },
    /// Phase transition.
    ScanPhase {
        /// Payload.
        phase: ScanPhaseEvent,
    },
    /// Batched path errors.
    ScanErrorBatch {
        /// Payload.
        errors: ScanErrorBatch,
    },
    /// Scan finished (summary inside).
    ScanDone {
        /// Payload.
        done: ScanDone,
    },
    /// Filter results page.
    FilterUpdated {
        /// Payload.
        filter: FilterUpdated,
    },
    /// Duplicates pipeline progress.
    DupesProgress {
        /// Payload.
        dupes: DupesProgress,
    },
    /// Cleanup execution progress.
    CleanupProgress {
        /// Payload.
        cleanup: CleanupProgress,
    },
    /// Monitor sample.
    MonitorSample {
        /// Payload.
        sample: MonitorSample,
    },
    /// License state changed (mirror of main-process licensing client).
    LicenseChanged {
        /// Active SKU (null = unlicensed).
        sku: Option<Sku>,
    },
    /// Engine warning.
    EngineWarning {
        /// Payload.
        warning: EngineWarning,
    },
}
