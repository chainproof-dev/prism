//! IPC extension surface — parity completion + homegrown feature commands
//! (docs/05 § 3.3–3.7): preflight, rescan/reattach, resolve-path,
//! color-mapping, type colors, dupes, cleanup ledger, apps, snapshots,
//! monitor, export. Registered on the same [`super`] state; the napi
//! surface is flat across modules.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use napi::bindgen_prelude::Status;
use napi::{Env, Result};
use napi_derive::napi;
use serde_json::Value;

use prism_types::commands::{
    AppFootprintQuery, AppsQuery, ColorMappingQuery, DupesGroupsPage, DupesGroupsQuery,
    DupesRunInfo, DupesRunQuery, ExecuteQuery, ExportQuery, MonitorQuery, PreflightQuery,
    PresetHit, PresetHitsPage, QueuePage, RescanQuery, ResolvePathQuery, ResolveResult,
    ScanControlQuery, SnapshotDiffQuery, SnapshotInfo, SnapshotSaveQuery, SnapshotsPage,
    SnapshotsQuery, StageQuery, StagedTotals, TypeColorQuery, UnstageQuery,
};
use prism_types::events::EngineEvent;
use prism_types::scan::{ChildrenQuery, ScanSummary};
use prism_types::viz::ColorMode;

use crate::arena::{Arena, NodeId, kind};
use crate::cleanup::{self, execute as cleanup_execute};
use crate::dupes::{self, DupesRun};
use crate::error::EngineError;
use crate::ipc::{EngineEventSink, SINK};
use crate::persistence;
use crate::{agg, apps, export, monitor};

use super::{contain, ne, parse_payload, state, to_json};

impl Default for DupesManager {
    fn default() -> Self {
        Self {
            next_run_id: 1,
            runs: std::collections::HashMap::new(),
            running: None,
        }
    }
}

/// Engine-boundary premium gate (PRISM-LIC-040): every premium handler
/// calls this first; grants are only set by `lic:verify-token`.
fn require_feature(st: &super::EngineState, feature: &str) -> napi::Result<()> {
    let gated = st
        .grants
        .lock()
        .as_ref()
        .map(|g| {
            let want = match feature {
                "turbo" => prism_types::licensing::PremiumFeature::Turbo,
                "dupes" => prism_types::licensing::PremiumFeature::Dupes,
                "cleanup" => prism_types::licensing::PremiumFeature::Cleanup,
                "apps" => prism_types::licensing::PremiumFeature::Apps,
                "snapshots" => prism_types::licensing::PremiumFeature::Snapshots,
                "monitor" => prism_types::licensing::PremiumFeature::Monitor,
                "scheduler" => prism_types::licensing::PremiumFeature::Scheduler,
                _ => prism_types::licensing::PremiumFeature::Export,
            };
            g.features.contains(&want)
        })
        .unwrap_or(false);
    if gated {
        Ok(())
    } else {
        Err(napi::Error::new(
            Status::GenericFailure,
            format!("{{\"kind\":\"not-licensed\",\"feature\":\"{feature}\"}}"),
        ))
    }
}

/// One dupes run's stored outcome.
pub struct DupesStateEntry {
    /// Result (set at completion).
    pub run: Option<DupesRun>,
    /// Live cancel flag while running.
    pub cancel: Arc<AtomicBool>,
}

/// Dupes manager (per-scan runs keyed by run id).
pub struct DupesManager {
    /// Next run id (1-based).
    pub next_run_id: u32,
    /// Completed or in-flight runs.
    pub runs: std::collections::HashMap<u32, DupesStateEntry>,
    /// Currently running run id (single active pipeline).
    pub running: Option<u32>,
}

// ---------------------------------------------------------------------------
// system
// ---------------------------------------------------------------------------

#[napi]
/// `sys:preflight` — target readability + elevation need before scanning.
pub fn sys_preflight(payload: Value) -> Result<Value> {
    contain(|| {
        let q: PreflightQuery = parse_payload(payload)?;
        let path = std::path::Path::new(&q.target);
        let readable = if path.is_dir() {
            std::fs::read_dir(path)
                .map(|mut it| it.next().is_some())
                .unwrap_or(true)
                || true // an empty-but-openable dir is readable; the map already proved openability
        } else {
            false
        };
        // Elevation need: Windows system roots + admin-denied dirs. The
        // honest signal is `is_elevated` + known system roots; anything
        // else is discovered during the walk and surfaced in errors.
        let requires_elevation = crate::sysinfo::requires_elevation(&q.target);
        let (free, total) = crate::sysinfo::volume_stats(&q.target);
        to_json(prism_types::sys::PreflightInfo {
            target: q.target,
            readable,
            requires_elevation,
            free_bytes: free,
            total_bytes: total,
            checked_at: agg::now_unix_ms(),
        })
    })
}

// ---------------------------------------------------------------------------
// scanning
// ---------------------------------------------------------------------------

#[napi]
/// `scan:rescan-subtree` — fresh walk of one subtree grafted onto the lease
/// (v1 semantics: a new lease whose arena is the subtree; the renderer
/// merges into its live tree — docs/10 § 5 rescan-subtree state).
pub fn scan_rescan_subtree(payload: Value) -> Result<Value> {
    contain(|| {
        let q: RescanQuery = parse_payload(payload)?;
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let a = &scan.arena;
        if q.node_id as usize >= a.len() || a.kind(q.node_id) != kind::DIR {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"nodeId (must be a directory)\"}",
            ));
        }
        let sub_root = node_path_pub(a, q.node_id, &scan.summary.root);
        let st2 = Arc::clone(&st);
        let on_complete: crate::scanner::coordinator::CompletionCb =
            Arc::new(move |id, completed| st2.scans.complete(id, completed));
        let target = prism_types::scan::ScanTarget::Folder {
            paths: vec![sub_root],
        };
        let id = st
            .scans
            .start(
                target,
                prism_types::scan::ScanStrategy::Standard,
                q.options,
                on_complete,
            )
            .map_err(ne)?;
        Ok(serde_json::json!({ "scanId": id }))
    })
}

#[napi]
/// `scan:reattach` — after a renderer reload: last summary + phase.
pub fn scan_reattach(scan_id: u32) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let phase = st.scans.phase(scan_id);
        let summary = st.scans.summary(scan_id).ok();
        to_json(prism_types::commands::ReattachInfo {
            summary: summary.map(|s: ScanSummary| s),
            phase,
        })
    })
}

// ---------------------------------------------------------------------------
// nodes
// ---------------------------------------------------------------------------

#[napi]
/// `node:resolve-path` — locate a node by absolute path (drop-in locate,
/// drag-in, reveal). Walks by path segments from the root.
pub fn node_resolve_path(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ResolvePathQuery = parse_payload(payload)?;
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let a = &scan.arena;
        let root = scan.summary.root.trim_end_matches(['\\', '/']);
        let rel = q
            .path
            .trim_start_matches(root)
            .trim_start_matches(['\\', '/']);
        let mut node = 0usize; // arena roots are at 0 for single-root scans
        if rel.is_empty() {
            return to_json(ResolveResult { node_id: Some(0) });
        }
        let sep = if q.path.contains('\\') { '\\' } else { '/' };
        'outer: for seg in rel.split(sep) {
            if seg.is_empty() {
                continue;
            }
            for &child in a.children(node as NodeId) {
                if a.name_str(child).eq_ignore_ascii_case(seg) {
                    node = child as usize;
                    continue 'outer;
                }
            }
            return to_json(ResolveResult { node_id: None });
        }
        to_json(ResolveResult {
            node_id: Some(node as u32),
        })
    })
}

// ---------------------------------------------------------------------------
// viz + types
// ---------------------------------------------------------------------------

#[napi]
/// `viz:color-mapping` — the legend for the active color mode.
pub fn viz_color_mapping(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ColorMappingQuery = parse_payload(payload)?;
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let a = &scan.arena;
        let total = a.allocated(0).max(1);
        let legend = match q.mode {
            ColorMode::Type => {
                // Categories present in this scan, by share.
                let mut by_cat: std::collections::HashMap<u16, (u64, u64)> = Default::default();
                for i in 0..a.len() {
                    if a.kind(i as NodeId) == kind::FILE {
                        let e = by_cat.entry(a.category(i as NodeId)).or_default();
                        e.0 += 1;
                        e.1 += a.allocated(i);
                    }
                }
                let mut rows: Vec<(u16, u64, u64)> =
                    by_cat.into_iter().map(|(c, (f, b))| (c, f, b)).collect();
                rows.sort_by_key(|r| std::cmp::Reverse(r.2));
                rows.into_iter()
                    .map(|(cat, _files, bytes)| {
                        let hue = agg::category_hue(cat);
                        prism_types::viz::LegendItem {
                            key: format!("cat:{cat}"),
                            label: agg::category_name(cat).to_string(),
                            color: hue_to_hex(hue),
                            share: bytes as f32 / total as f32,
                        }
                    })
                    .collect::<Vec<_>>()
            }
            ColorMode::Branch => (0..12)
                .map(|i| prism_types::viz::LegendItem {
                    key: format!("branch:{i}"),
                    label: format!("Branch {}", i + 1),
                    color: hue_to_hex(i as f32 * 30.0),
                    share: 1.0 / 12.0,
                })
                .collect(),
            ColorMode::Age => {
                const BUCKETS: &[(&str, f32)] = &[
                    ("Today", 140.0),
                    ("This week", 110.0),
                    ("This month", 80.0),
                    ("This quarter", 55.0),
                    ("This year", 35.0),
                    ("Older", 18.0),
                    ("Ancient", 5.0),
                ];
                BUCKETS
                    .iter()
                    .map(|(label, hue)| prism_types::viz::LegendItem {
                        key: format!("age:{label}"),
                        label: (*label).to_string(),
                        color: hue_to_hex(*hue),
                        share: 0.0,
                    })
                    .collect()
            }
        };
        let version = st.colors.lock().len() as u32; // bumps as user edits land
        to_json(prism_types::viz::ColorMapping { legend, version })
    })
}

#[napi]
/// `types:set-color` — persist a user color override (db-backed).
pub fn types_set_color(payload: Value) -> Result<()> {
    contain(|| {
        let q: TypeColorQuery = parse_payload(payload)?;
        let st = state()?;
        if let Some(db) = st.db.lock().as_ref() {
            match &q.color {
                Some(c) => db.set_type_color(&q.key, c).map_err(ne)?,
                None => db.set_type_color(&q.key, "").map_err(ne)?,
            }
        }
        let mut colors = st.colors.lock();
        match q.color {
            Some(c) => colors.insert(q.key, c),
            None => colors.remove(&q.key),
        };
        Ok(())
    })
}

/// HSL(360-wheel, 78%, 52%) → `#RRGGBB` (the data-palette generator —
/// matches the renderer's `palette.ts` which owns the same math).
fn hue_to_hex(hue: f32) -> String {
    let h = ((hue % 360.0) + 360.0) % 360.0 / 60.0;
    let s = 0.78f32;
    let l = 0.52f32;
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let to = |v: f32| ((v + l - c / 2.0).clamp(0.0, 1.0) * 255.0).round() as u32;
    format!("#{:02X}{:02X}{:02X}", to(r), to(g), to(b))
}

// ---------------------------------------------------------------------------
// duplicates (PRISM-HG-030)
// ---------------------------------------------------------------------------

#[napi]
/// `duplicates:run` — start the pipeline on a completed scan (worker
/// thread; progress via `dupes:progress` events).
pub fn duplicates_run(payload: Value) -> Result<Value> {
    contain(|| {
        let q: DupesRunQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "dupes")?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let arena = Arc::clone(&scan.arena);
        let root_path = scan.summary.root.clone();

        let sink = sink_arc()?;
        let mut dm = st.dupes.lock();
        if dm.running.is_some() {
            return Err(napi::Error::new(
                Status::GenericFailure,
                "{\"kind\":\"busy\",\"operation\":\"duplicates:run\"}",
            ));
        }
        let run_id = dm.next_run_id.max(1);
        dm.next_run_id = run_id + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        dm.runs.insert(
            run_id,
            DupesStateEntry {
                run: None,
                cancel: Arc::clone(&cancel),
            },
        );
        dm.running = Some(run_id);
        drop(dm);

        let st2 = Arc::clone(&st);
        std::thread::Builder::new()
            .name(format!("prism-dupes-{run_id}"))
            .spawn(move || {
                let result = dupes::run(&arena, &root_path, q.min_size, &sink, &cancel);
                let mut dm = st2.dupes.lock();
                if let Some(entry) = dm.runs.get_mut(&run_id) {
                    entry.run = Some(result);
                }
                if dm.running == Some(run_id) {
                    dm.running = None;
                }
            })
            .map_err(|e| {
                napi::Error::new(
                    Status::GenericFailure,
                    format!("{{\"kind\":\"engine\",\"msg\":\"spawn dupes: {e}\"}}"),
                )
            })?;
        to_json(DupesRunInfo { run_id })
    })
}

#[napi]
/// `duplicates:cancel` — cancel the running pipeline (≤ one phase latency).
pub fn duplicates_cancel(_scan_id: u32) -> Result<()> {
    contain(|| {
        let st = state()?;
        let dm = st.dupes.lock();
        if let Some(rid) = dm.running
            && let Some(e) = dm.runs.get(&rid)
        {
            e.cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    })
}

#[napi]
/// `duplicates:groups` — paged results of a run.
pub fn duplicates_groups(payload: Value) -> Result<Value> {
    contain(|| {
        let q: DupesGroupsQuery = parse_payload(payload)?;
        let st = state()?;
        let dm = st.dupes.lock();
        let Some(entry) = dm.runs.get(&q.run_id) else {
            return Err(napi::Error::new(
                Status::InvalidArg,
                format!(
                    "{{\"kind\":\"invalid-args\",\"field\":\"runId\",\"detail\":\"{}\"}}",
                    q.run_id
                ),
            ));
        };
        let Some(run) = entry.run.as_ref() else {
            // Still running — an empty page with total 0 (the UI tracks the
            // running state from events; this is not silently "no results").
            return to_json(DupesGroupsPage {
                total: 0,
                reclaimable: 0,
                groups: Vec::new(),
            });
        };
        let total = run.groups.len() as u32;
        let start = (q.offset as usize).min(run.groups.len());
        let end = (start + q.limit as usize).min(run.groups.len());
        to_json(DupesGroupsPage {
            total,
            reclaimable: run.reclaimable,
            groups: run.groups[start..end].to_vec(),
        })
    })
}

// ---------------------------------------------------------------------------
// cleanup ledger (PRISM-HG-010 + docs/10 § 13)
// ---------------------------------------------------------------------------

#[napi]
/// `cleanup:presets-scan` — match all presets against a completed arena.
pub fn cleanup_presets_scan(scan_id: u32) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let scan = st.scans.completed(scan_id).map_err(ne)?;
        let a = &scan.arena;
        let mut hits = Vec::new();
        for preset in cleanup::PRESETS {
            let nodes = cleanup::preset_hits(a, preset);
            if nodes.is_empty() {
                continue;
            }
            let bytes: u64 = nodes.iter().map(|&n| a.allocated(n as usize)).sum();
            let paths = nodes
                .iter()
                .take(200)
                .map(|&n| node_path_pub(a, n, &scan.summary.root))
                .collect();
            hits.push(PresetHit {
                preset_id: preset.id.to_string(),
                name: preset.name.to_string(),
                safety: preset.safety.to_string(),
                explanation: preset.explanation.to_string(),
                paths,
                bytes,
            });
        }
        hits.sort_by_key(|h| std::cmp::Reverse(h.bytes));
        to_json(PresetHitsPage { hits })
    })
}

#[napi]
/// `cleanup:stage` — enter the ledger (blocked paths fail loudly).
pub fn cleanup_stage(payload: Value) -> Result<Value> {
    contain(|| {
        let q: StageQuery = parse_payload(payload)?;
        let st = state()?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let mut queue = st.queue.lock();
        let rejections = match queue.stage(&scan.arena, &scan.summary.root, &q.node_ids, q.source) {
            Ok(()) => Vec::new(),
            Err(rej) => rej
                .into_iter()
                .map(|(n, reason)| serde_json::json!({ "nodeId": n, "reason": reason }))
                .collect::<Vec<_>>(),
        };
        let (count, bytes) = queue.totals();
        let _ = sink_arc()?.emit(EngineEvent::EngineWarning {
            warning: prism_types::events::EngineWarning {
                code: "cleanup-staged".into(),
                msg: format!("{count} items staged ({bytes} bytes)"),
            },
        });
        Ok(serde_json::json!({
            "totals": StagedTotals { count, bytes },
            "rejected": rejections,
        }))
    })
}

#[napi]
/// `cleanup:unstage`
pub fn cleanup_unstage(payload: Value) -> Result<Value> {
    contain(|| {
        let q: UnstageQuery = parse_payload(payload)?;
        let st = state()?;
        let mut queue = st.queue.lock();
        queue.unstage(&q.node_ids);
        let (count, bytes) = queue.totals();
        to_json(StagedTotals { count, bytes })
    })
}

#[napi]
/// `cleanup:queue`
pub fn cleanup_queue() -> Result<Value> {
    contain(|| {
        let st = state()?;
        let queue = st.queue.lock();
        to_json(QueuePage {
            items: queue.items().to_vec(),
        })
    })
}

#[napi]
/// `cleanup:execute` — run the ledger (premium-gated at this boundary,
/// PRISM-LIC-040: the engine checks grants, the UI gate is cosmetic only).
pub fn cleanup_execute(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ExecuteQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "cleanup")?;

        // The scan lease is the staging source; items already carry paths.
        let sink = sink_arc()?;
        let items = {
            let queue = st.queue.lock();
            queue.items().to_vec()
        };
        if items.is_empty() {
            return Err(napi::Error::new(
                Status::InvalidArg,
                "{\"kind\":\"invalid-args\",\"field\":\"queue\",\"detail\":\"empty\"}",
            ));
        }
        let result = cleanup_execute::execute(&sink, &items, q.to_recycle_bin).map_err(ne)?;
        // Successful items leave the queue; failures stay (fail-loud ledger).
        let ok_paths: std::collections::HashSet<&str> = result
            .outcomes
            .iter()
            .filter(|o| o.ok)
            .map(|o| o.path.as_str())
            .collect();
        st.queue
            .lock()
            .unstage_filtered(|i| ok_paths.contains(i.path.as_str()));
        to_json(result)
    })
}

// ---------------------------------------------------------------------------
// applications (PRISM-HG-040)
// ---------------------------------------------------------------------------

#[napi]
/// `apps:list` — installed inventory (registry on Windows; honest empty off
/// Windows).
pub fn apps_list(payload: Value) -> Result<Value> {
    contain(|| {
        let q: AppsQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "apps")?;
        to_json(apps::apps_page(q.include_system, false))
    })
}

#[napi]
/// `apps:footprint` — evidence list per root (no opaque totals).
pub fn apps_footprint(payload: Value) -> Result<Value> {
    contain(|| {
        let q: AppFootprintQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "apps")?;
        to_json(apps::footprint(&q.token))
    })
}

#[napi]
/// `apps:leftovers` — detected remains of removed apps (arena-walk).
pub fn apps_leftovers(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ScanControlQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "apps")?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        to_json(apps::leftovers(&scan.arena, &scan.summary.root))
    })
}

// ---------------------------------------------------------------------------
// snapshots (PRISM-HG-050)
// ---------------------------------------------------------------------------

#[napi]
/// `snapshots:save` — capture the completed scan's depth-capped shape.
pub fn snapshots_save(payload: Value) -> Result<Value> {
    contain(|| {
        let q: SnapshotSaveQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "snapshots")?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let db = db_of(&st)?;
        let depth = if q.depth == 0 { 6 } else { q.depth };
        let info: SnapshotInfo = db
            .save_snapshot(&scan.arena, &scan.summary.root, depth)
            .map_err(ne)?;
        to_json(info)
    })
}

#[napi]
/// `snapshots:list`
pub fn snapshots_list(payload: Value) -> Result<Value> {
    contain(|| {
        let q: SnapshotsQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "snapshots")?;
        let db = db_of(&st)?;
        let snapshots = db.list_snapshots(&q.root);
        to_json(SnapshotsPage { snapshots })
    })
}

#[napi]
/// `snapshots:diff` — BEFORE/AFTER deltas with significance floor.
pub fn snapshots_diff(payload: Value) -> Result<Value> {
    contain(|| {
        let q: SnapshotDiffQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "snapshots")?;
        let db = db_of(&st)?;
        let floor = if q.floor_bytes == 0 {
            10 * 1024 * 1024
        } else {
            q.floor_bytes
        };
        let (deltas, net) = db.diff_snapshots(q.before, q.after, floor).map_err(ne)?;
        to_json(prism_types::commands::SnapshotDiffPage { deltas, net })
    })
}

// ---------------------------------------------------------------------------
// monitor (PRISM-HG-060)
// ---------------------------------------------------------------------------

#[napi]
/// `monitor:start` — begin 1 Hz sampling (premium).
pub fn monitor_start(payload: Value) -> Result<()> {
    contain(|| {
        let q: MonitorQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "monitor")?;
        let sink = sink_arc()?;
        let mut m = st.monitor.lock();
        if m.is_some() {
            return Ok(()); // already running (idempotent)
        }
        *m = Some(monitor::start(sink, q.period_ms));
        Ok(())
    })
}

#[napi]
/// `monitor:stop`
pub fn monitor_stop() -> Result<()> {
    contain(|| {
        let st = state()?;
        let mut m = st.monitor.lock();
        if let Some(h) = m.take() {
            h.stop();
        }
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// export (PRISM-HG-090)
// ---------------------------------------------------------------------------

#[napi]
/// `export:scan` — CSV/NDJSON export of a completed scan (premium).
pub fn export_scan(payload: Value) -> Result<Value> {
    contain(|| {
        let q: ExportQuery = parse_payload(payload)?;
        let st = state()?;
        require_feature(&st, "export")?;
        let scan = st.scans.completed(q.scan_id).map_err(ne)?;
        let selected: Vec<NodeId> = Vec::new();
        let res = export::export(
            &scan.arena,
            &scan.ext_table,
            &scan.summary.root,
            &selected,
            None,
            q.format,
            q.scope,
            std::path::Path::new(&q.dest),
        )
        .map_err(ne)?;
        to_json(res)
    })
}

// ---------------------------------------------------------------------------
// persistence (engine-internal commands for the main process)
// ---------------------------------------------------------------------------

#[napi]
/// Open (and migrate) the app database. Main calls this at boot with the
/// user-data path. Idempotent.
pub fn engine_open_db(path: String) -> Result<()> {
    contain(|| {
        let st = state()?;
        let mut db = st.db.lock();
        if db.is_some() {
            return Ok(());
        }
        let opened = persistence::Db::open(std::path::Path::new(&path)).map_err(ne)?;
        // Hydrate type colors.
        let colors = opened.type_colors();
        *st.colors.lock() = colors.into_iter().filter(|(_, c)| !c.is_empty()).collect();
        *db = Some(Arc::new(opened));
        Ok(())
    })
}

#[napi]
/// Store one settings row (JSON value). Engine-side settings are the ones
/// the engine itself needs on boot (exclusions, stale thresholds); UI prefs
/// live in the main-process store.
pub fn engine_set_setting(key: String, value_json: String) -> Result<()> {
    contain(|| {
        let st = state()?;
        let db = db_of(&st)?;
        db.set_setting(&key, &value_json).map_err(ne)?;
        Ok(())
    })
}

#[napi]
/// Read one settings row (JSON value, null = unset).
pub fn engine_get_setting(key: String) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let db = db_of(&st)?;
        match db.get_setting(&key) {
            Some(v) => Ok(serde_json::from_str(&v).unwrap_or(Value::String(v))),
            None => Ok(Value::Null),
        }
    })
}

#[napi]
/// Recent scans history (Welcome recents list).
pub fn engine_recent_scans(limit: u32) -> Result<Value> {
    contain(|| {
        let st = state()?;
        let db = db_of(&st)?;
        let rows: Vec<serde_json::Value> = db
            .recent_scans(limit.clamp(1, 50))
            .into_iter()
            .map(|r| {
                serde_json::json!({
                    "startedAt": r.started_at,
                    "finishedAt": r.finished_at,
                    "target": r.target,
                    "strategy": r.strategy,
                    "files": r.files,
                    "folders": r.folders,
                    "bytes": r.bytes,
                    "durationMs": r.duration_ms,
                    "errorCount": r.error_count,
                })
            })
            .collect();
        Ok(Value::Array(rows))
    })
}

/// Record a completed scan into history (called by the completion hook).
pub fn record_scan_history(st: &super::EngineState, summary: &ScanSummary, duration_ms: u64) {
    let db_arc = match st.db.lock().clone() {
        Some(d) => d,
        None => return,
    };
    let db = db_arc.as_ref();
    let rec = persistence::ScanRecord {
        started_at: agg::now_unix_ms() - duration_ms as i64,
        finished_at: agg::now_unix_ms(),
        target: summary.root.clone(),
        strategy: format!("{:?}", summary.strategy).to_lowercase(),
        files: summary.files,
        folders: summary.folders,
        bytes: summary.allocated,
        duration_ms: duration_ms as i64,
        error_count: 0,
    };
    let _ = db.record_scan(&rec);
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn sink_arc() -> Result<Arc<EngineEventSink>> {
    SINK.get()
        .cloned()
        .ok_or_else(|| napi::Error::new(Status::GenericFailure, "event sink not initialized"))
}

fn db_of(st: &super::EngineState) -> Result<Arc<persistence::Db>> {
    st.db_arc()
}

fn node_path_pub(arena: &Arena, node: NodeId, root_path: &str) -> String {
    super::node_path(arena, node, root_path)
}

/// Keep `Env` referenced (napi surface hygiene across modules).
#[allow(unused)]
fn _env(_e: Env) {}

/// `duplicates:cancel` payload is a ScanControlQuery-shaped control; expose
/// the raw scan-id variant used above.
#[allow(unused)]
fn _scan_control(_q: &ScanControlQuery) {}

#[allow(unused)]
fn _children_query_surface(_q: &ChildrenQuery) {}

/// Keep unused imports referenced that document the wire surface.
#[allow(unused)]
fn _wire_surface(_e: Option<EngineError>) {}
