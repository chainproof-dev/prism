//! # Scan coordinator (docs/06 § 2.2)
//!
//! One scan = one coordinator thread (arena single-writer) + N enumeration
//! workers sharing a crossbeam MPMC frontier channel (v1 topology; the
//! work-stealing deque upgrade is a measured change — bench-scan gates it).
//! Cancellation is cooperative (≤ 50 ms); pause parks workers on a condvar.
//! Directory completion cascades bottom-up so the UI sees live sizes
//! (parity-SCN-02). Free-space/unknown pseudo-nodes attach before CSR finalize.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use parking_lot::Mutex as PlMutex;

use prism_types::events::{
    EngineEvent, NodeDelta, ScanDone, ScanNodes, ScanPhaseEvent, ScanProgress,
};
use prism_types::ids::{NodeId, ScanId};
use prism_types::scan::{
    PathError, ScanOptions, ScanPhase, ScanStrategy, ScanSummary, ScanTarget, SizeMode,
};

use crate::agg::{self, Aggregates, ExtensionTable};
use crate::arena::{ARENA_NODE_CEILING, Arena, NodeInput, kind};
use crate::error::{EngineError, Result};
use crate::ipc::EngineEventSink;
use crate::scanner::exclusions::ExclusionSet;
use crate::scanner::{DirBatch, DirEnumerator, DirTask, EntryClass};

/// Shared control flags for one scan.
pub struct Control {
    /// Cooperative cancel flag.
    pub cancel: AtomicBool,
    paused: Mutex<bool>,
    pause_cv: Condvar,
    files_seen: AtomicU64,
    bytes_seen: AtomicU64,
    dirs_seen: AtomicU64,
}

impl Control {
    fn new() -> Self {
        Self {
            cancel: AtomicBool::new(false),
            paused: Mutex::new(false),
            pause_cv: Condvar::new(),
            files_seen: AtomicU64::new(0),
            bytes_seen: AtomicU64::new(0),
            dirs_seen: AtomicU64::new(0),
        }
    }

    fn wait_if_paused(&self) {
        let mut g = self.paused.lock().unwrap_or_else(|e| e.into_inner());
        while *g {
            let (g2, _t) = self
                .pause_cv
                .wait_timeout(g, std::time::Duration::from_millis(50))
                .unwrap_or_else(|e| e.into_inner());
            g = g2;
        }
    }
}

/// Post-completion scan state available to queries.
pub struct CompletedScan {
    /// Frozen arena.
    pub arena: Arc<Arena>,
    /// Interned extensions.
    pub ext_table: Arc<ExtensionTable>,
    /// Aggregate tables.
    pub aggregates: Arc<Aggregates>,
    /// Final summary.
    pub summary: ScanSummary,
    /// Errors collected during the walk (errors drawer source).
    pub errors: Vec<PathError>,
}

/// A live or completed scan lease.
pub enum ScanLease {
    /// Walking right now — query surface returns `Busy`; the renderer
    /// consumes `scan:nodes` deltas (ADR-03).
    Walking {
        /// Live phase mirror (probe surface).
        phase: Arc<PlMutex<ScanPhase>>,
        /// Cancel/pause control flags.
        control: Arc<Control>,
    },
    /// Completed.
    Done(Arc<CompletedScan>),
}

/// Completion callback: (scan id, completed arena when successful).
pub type CompletionCb = Arc<dyn Fn(ScanId, Option<Arc<CompletedScan>>) + Send + Sync + 'static>;

/// The scan manager: leases, ids, cancellation fan-out.
pub struct ScanManager {
    inner: PlMutex<ScanManagerInner>,
    sink: Arc<EngineEventSink>,
}

struct ScanManagerInner {
    next_id: ScanId,
    leases: HashMap<ScanId, ScanLease>,
    active: Option<ScanId>,
}

impl ScanManager {
    /// New manager bound to the engine event sink.
    pub fn new(sink: Arc<EngineEventSink>) -> Self {
        Self {
            inner: PlMutex::new(ScanManagerInner {
                next_id: 1,
                leases: HashMap::new(),
                active: None,
            }),
            sink,
        }
    }

    /// Start a scan. Returns the lease id immediately; the coordinator thread
    /// reports outcome through the completion callback and events.
    pub fn start(
        &self,
        target: ScanTarget,
        _strategy: ScanStrategy,
        options: ScanOptions,
        on_complete: CompletionCb,
    ) -> Result<ScanId> {
        let mut inner = self.inner.lock();
        if inner.active.is_some() {
            return Err(EngineError::Busy {
                op: "scan:start (one active scan per v1)",
            });
        }
        let id = inner.next_id;
        inner.next_id = inner
            .next_id
            .checked_add(1)
            .ok_or_else(|| EngineError::Internal("scan id overflow".into()))?;
        let phase = Arc::new(PlMutex::new(ScanPhase::Walking));
        let control = Arc::new(Control::new());
        inner.leases.insert(
            id,
            ScanLease::Walking {
                phase: Arc::clone(&phase),
                control: Arc::clone(&control),
            },
        );
        inner.active = Some(id);
        drop(inner);

        let sink = Arc::clone(&self.sink);
        std::thread::Builder::new()
            .name(format!("prism-scan-{id}"))
            .spawn(move || {
                let started = std::time::Instant::now();
                let outcome = run_scan(
                    id,
                    &target,
                    &options,
                    Arc::clone(&control),
                    Arc::clone(&sink),
                    Arc::clone(&phase),
                );
                let final_phase = match &outcome {
                    Ok(_) => ScanPhase::Done,
                    Err(EngineError::Cancelled) => ScanPhase::Cancelled,
                    Err(_) => ScanPhase::Failed,
                };
                let _ = sink.emit(EngineEvent::ScanPhase {
                    phase: ScanPhaseEvent {
                        scan_id: id,
                        phase: final_phase,
                        detail: outcome.as_ref().err().map(|e| e.to_string()),
                    },
                });
                match outcome {
                    Ok(completed) => {
                        let summary = completed.summary.clone();
                        let _ = sink.emit(EngineEvent::ScanDone {
                            done: ScanDone { summary },
                        });
                        on_complete(id, Some(completed));
                    }
                    Err(e) => {
                        tracing::warn!(scan = id, error = %e, "scan ended without result");
                        let _ = sink.emit(EngineEvent::ScanDone {
                            done: ScanDone {
                                summary: cancelled_summary(id, started),
                            },
                        });
                        on_complete(id, None);
                    }
                }
            })
            .map_err(|e| EngineError::Internal(format!("spawn scan thread: {e}")))?;
        Ok(id)
    }

    /// Cancel a scan (cooperative, ≤ 50 ms ack).
    pub fn cancel(&self, scan_id: ScanId) -> Result<()> {
        let inner = self.inner.lock();
        match inner.leases.get(&scan_id) {
            Some(ScanLease::Walking { control, .. }) => {
                control.cancel.store(true, Ordering::SeqCst);
                Ok(())
            }
            Some(ScanLease::Done(_)) | None => Err(EngineError::Scan {
                id: scan_id,
                stage: "cancel",
                msg: "not walking".into(),
            }),
        }
    }

    /// Pause (standard strategy only — turbo is cancel-only, ADR-06).
    pub fn pause(&self, scan_id: ScanId) -> Result<()> {
        let inner = self.inner.lock();
        match inner.leases.get(&scan_id) {
            Some(ScanLease::Walking { control, .. }) => {
                let mut g = control.paused.lock().unwrap_or_else(|e| e.into_inner());
                *g = true;
                Ok(())
            }
            _ => Err(EngineError::Scan {
                id: scan_id,
                stage: "pause",
                msg: "not walking".into(),
            }),
        }
    }

    /// Resume.
    pub fn resume(&self, scan_id: ScanId) -> Result<()> {
        let inner = self.inner.lock();
        match inner.leases.get(&scan_id) {
            Some(ScanLease::Walking { control, .. }) => {
                let mut g = control.paused.lock().unwrap_or_else(|e| e.into_inner());
                *g = false;
                control.pause_cv.notify_all();
                Ok(())
            }
            _ => Err(EngineError::Scan {
                id: scan_id,
                stage: "resume",
                msg: "not walking".into(),
            }),
        }
    }

    /// Install a completed scan (engine completion callback).
    pub fn complete(&self, scan_id: ScanId, completed: Option<Arc<CompletedScan>>) {
        let mut inner = self.inner.lock();
        if inner.active == Some(scan_id) {
            inner.active = None;
        }
        match completed {
            Some(c) => {
                inner.leases.insert(scan_id, ScanLease::Done(c));
            }
            None => {
                inner.leases.remove(&scan_id);
            }
        }
    }

    /// Get a completed scan for queries.
    pub fn completed(&self, scan_id: ScanId) -> Result<Arc<CompletedScan>> {
        let inner = self.inner.lock();
        match inner.leases.get(&scan_id) {
            Some(ScanLease::Done(c)) => Ok(Arc::clone(c)),
            Some(ScanLease::Walking { .. }) => Err(EngineError::Busy {
                op: "query during walk (consume scan:nodes deltas)",
            }),
            None => Err(EngineError::Scan {
                id: scan_id,
                stage: "query",
                msg: "unknown scan id".into(),
            }),
        }
    }

    /// Phase probe.
    pub fn phase(&self, scan_id: ScanId) -> Option<ScanPhase> {
        let inner = self.inner.lock();
        match inner.leases.get(&scan_id) {
            Some(ScanLease::Walking { phase, .. }) => Some(*phase.lock()),
            Some(ScanLease::Done(_)) => Some(ScanPhase::Done),
            None => None,
        }
    }

    /// Summary of a completed scan.
    pub fn summary(&self, scan_id: ScanId) -> Result<ScanSummary> {
        self.completed(scan_id).map(|c| c.summary.clone())
    }
}

fn cancelled_summary(id: ScanId, started: std::time::Instant) -> ScanSummary {
    ScanSummary {
        scan_id: id,
        root: String::new(),
        strategy: ScanStrategy::Standard,
        size_mode: SizeMode::Allocated,
        files: 0,
        folders: 0,
        logical: 0,
        allocated: 0,
        unique: 0,
        unknown: 0,
        free: 0,
        duration_ms: started.elapsed().as_millis() as u64,
        errors: 0,
        truncated: false,
    }
}

// ---------------------------------------------------------------------------
// the scan run (coordinator thread body)
// ---------------------------------------------------------------------------

fn run_scan(
    id: ScanId,
    target: &ScanTarget,
    options: &ScanOptions,
    control: Arc<Control>,
    sink: Arc<EngineEventSink>,
    phase: Arc<PlMutex<ScanPhase>>,
) -> Result<Arc<CompletedScan>> {
    let started = std::time::Instant::now();
    let roots: Vec<String> = match target {
        ScanTarget::Volume { path } => vec![path.clone()],
        ScanTarget::Folder { paths } => {
            if paths.is_empty() {
                return Err(EngineError::Invalid {
                    field: "target.paths",
                });
            }
            paths.clone()
        }
        ScanTarget::Home => vec![home_dir()],
    };

    let exclusions =
        ExclusionSet::compile(&options.exclude_patterns).map_err(|m| EngineError::Scan {
            id,
            stage: "options",
            msg: m,
        })?;

    let mut arena = Arena::with_capacity(1 << 16);
    let mut ext_table = ExtensionTable::default();
    let mut errors: Vec<PathError> = Vec::new();
    // (volume_serial, file_id) → owner node; loop + hard-link protection
    let mut visited: HashMap<(u32, u64), NodeId> = HashMap::new();
    // pending[dir] = number of child dirs not yet completed (+1 while the
    // dir's own batch is in flight). Indexed by NodeId.
    let mut pending: Vec<u32> = Vec::new();
    // Absolute paths for DIRECTORIES only (files rebuild paths from the name
    // chain on demand — zero per-file path storage on the production path).
    let mut dir_paths: Vec<Vec<u16>> = Vec::new();
    let mut truncated = false;

    // --- root node(s) ---------------------------------------------------------
    let root_id = {
        let (name, single): (String, bool) = if roots.len() == 1 {
            (root_name(&roots[0]), true)
        } else {
            ("This PC".to_string(), false) // synthetic multi-root (parity-SEL-04)
        };
        let name16: Vec<u16> = name.encode_utf16().collect();
        let rid = arena.push(NodeInput {
            parent: 0,
            name_utf16: &name16,
            logical: 0,
            allocated: 0,
            files: 0,
            folders: 1,
            mtime: 0,
            kind: kind::ROOT,
            category: prism_types::ids::CATEGORY_ROOT,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        pending.push(0);
        dir_paths.push(if single {
            roots[0].encode_utf16().collect()
        } else {
            Vec::new()
        });
        rid
    };

    // --- worker pool ------------------------------------------------------------
    let worker_count = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(32);
    let (task_tx, task_rx) = bounded::<DirTask>(worker_count * 4);
    let (result_tx, result_rx): (Sender<DirBatch>, Receiver<DirBatch>) = unbounded();

    let mut workers: Vec<JoinHandle<()>> = {
        let mut handles = Vec::with_capacity(worker_count);
        for w in 0..worker_count {
            let rx = task_rx.clone();
            let tx = result_tx.clone();
            let ctrl = Arc::clone(&control);
            let mut enumr: Box<dyn DirEnumerator> = make_enumerator();
            handles.push(
                std::thread::Builder::new()
                    .name(format!("prism-worker-{w}"))
                    .spawn(move || {
                        loop {
                            if ctrl.cancel.load(Ordering::SeqCst) {
                                break;
                            }
                            ctrl.wait_if_paused();
                            match rx.recv_timeout(std::time::Duration::from_millis(50)) {
                                Ok(task) => {
                                    if tx.send(enumr.enumerate(&task)).is_err() {
                                        break;
                                    }
                                }
                                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    })
                    .map_err(|e| EngineError::Internal(format!("worker spawn: {e}")))?,
            );
        }
        handles
    };
    drop(task_rx);
    drop(result_tx);

    // --- prime frontier -----------------------------------------------------------
    let mut outstanding: u64 = 0;
    {
        let roots_to_queue: Vec<(String, NodeId, bool)> = if roots.len() == 1 {
            vec![(roots[0].clone(), root_id, true)]
        } else {
            let mut v = Vec::new();
            for p in &roots {
                let name16: Vec<u16> = root_name(p).encode_utf16().collect();
                let rid = arena.push(NodeInput {
                    parent: root_id,
                    name_utf16: &name16,
                    logical: 0,
                    allocated: 0,
                    files: 0,
                    folders: 1,
                    mtime: 0,
                    kind: kind::DIR,
                    category: prism_types::ids::CATEGORY_ROOT,
                    ext_id: 0,
                    attr_flags: 0,
                    link_to: 0,
                    err_code: 0,
                });
                arena.attach(root_id, rid);
                pending.push(1);
                dir_paths.push(p.encode_utf16().collect());
                v.push((p.clone(), rid, false));
            }
            v
        };
        for (path, node, is_root) in roots_to_queue {
            let path16: Vec<u16> = if is_root {
                path.encode_utf16().collect()
            } else {
                dir_paths[node as usize].clone()
            };
            if is_root {
                dir_paths[root_id as usize] = path16.clone();
                pending[root_id as usize] = 1;
            } else {
                pending[root_id as usize] += 1;
            }
            task_tx
                .send(DirTask {
                    node,
                    path: path16,
                    depth: if is_root { 0 } else { 1 },
                })
                .map_err(|_| EngineError::Internal("frontier send".into()))?;
            outstanding += 1;
        }
    }

    // --- coordinator loop -------------------------------------------------------
    let mut delta_buf: Vec<NodeDelta> = Vec::with_capacity(512);
    let mut last_progress = std::time::Instant::now();
    let mut current_path = String::new();

    while outstanding > 0 {
        if control.cancel.load(Ordering::SeqCst) {
            drop(task_tx);
            for w in workers.drain(..) {
                let _ = w.join();
            }
            return Err(EngineError::Cancelled);
        }
        let batch = result_rx
            .recv_timeout(std::time::Duration::from_millis(500))
            .map_err(|_| EngineError::Scan {
                id,
                stage: "coordinator",
                msg: "worker pipeline stalled".into(),
            })?;
        outstanding -= 1;
        let dir_node = batch.dir;

        // clear the "own batch in flight" marker
        if pending[dir_node as usize] > 0 {
            pending[dir_node as usize] -= 1;
        }

        if let Some((code, msg)) = &batch.error {
            errors.push(PathError {
                path: String::from_utf16_lossy(
                    dir_paths
                        .get(dir_node as usize)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                ),
                code: *code,
                message: msg.clone(),
                denied: *code == 13 || *code == -1073741790,
            });
            arena.set_err(dir_node, (code & 0xFFFF).max(1) as u16);
        }

        let mut new_dirs: Vec<(NodeId, Vec<u16>)> = Vec::new();
        for i in 0..batch.count {
            if arena.len() >= ARENA_NODE_CEILING {
                truncated = true;
                break;
            }
            if control.cancel.load(Ordering::SeqCst) {
                drop(task_tx);
                for w in workers.drain(..) {
                    let _ = w.join();
                }
                return Err(EngineError::Cancelled);
            }
            let name16: Vec<u16> = batch.name(i).to_vec();
            let meta = batch.metas[i];
            let name_str = String::from_utf16_lossy(&name16);

            // exclusion: sticky directory names prune whole subtrees (preset
            // pattern class); full-path globs match the root-relative path
            if !exclusions.is_empty() {
                if exclusions.is_sticky_dir(&name_str) {
                    continue;
                }
                let rel_dir = rel_dir_prefix(
                    &roots,
                    dir_paths
                        .get(dir_node as usize)
                        .map(Vec::as_slice)
                        .unwrap_or(&[]),
                );
                let rel = format!("{rel_dir}{name_str}");
                if exclusions.is_excluded(&rel) {
                    continue;
                }
            }

            // reparse handling (PRISM-ENG-010): not descended by default
            let (node_kind, descend) = match meta.kind {
                EntryClass::File => (kind::FILE, false),
                EntryClass::Dir => (kind::DIR, true),
                EntryClass::Reparse(_) => (kind::REPARSE, options.follow_reparse),
                EntryClass::Mount => (kind::MOUNT, false),
            };

            // identity: hard links + revisit protection (PRISM-ENG-012)
            let mut revisit = false;
            let mut link_to = 0u64;
            if meta.file_id.file_id != 0 {
                let key = (meta.file_id.volume_serial, meta.file_id.file_id);
                match visited.get(&key) {
                    Some(_owner) => {
                        revisit = true;
                        if node_kind == kind::FILE {
                            link_to = meta.file_id.file_id; // secondary link
                        }
                        // revisited dirs (reparse loops) become link nodes — never walked twice
                    }
                    None => {
                        visited.insert(key, dir_node);
                    }
                }
            }
            let descend = descend && !revisit;

            let ext_key = if node_kind == kind::FILE {
                agg::extension_key(&name16)
            } else {
                String::new()
            };
            let ext_id = ext_table.intern(&ext_key);
            let category = if node_kind == kind::FILE {
                agg::category_for_ext(&ext_key)
            } else {
                prism_types::ids::CATEGORY_ROOT
            };
            let attr_flags = meta.attrs
                | u32::from(link_to != 0) * crate::arena::flags::INTERNAL_LINK2
                | u32::from(meta.reparse != 0) * crate::arena::flags::REPARSE;

            let is_file = node_kind == kind::FILE;
            let nid = arena.push(NodeInput {
                parent: dir_node,
                name_utf16: &name16,
                logical: if is_file { meta.size } else { 0 },
                allocated: if is_file { meta.alloc } else { 0 },
                files: u32::from(is_file),
                folders: u32::from(matches!(node_kind, kind::DIR | kind::REPARSE | kind::MOUNT)),
                mtime: meta.mtime,
                kind: if revisit && node_kind == kind::DIR {
                    kind::LINK
                } else {
                    node_kind
                },
                category,
                ext_id,
                attr_flags,
                link_to,
                err_code: 0,
            });
            arena.attach(dir_node, nid);
            pending.push(0);
            dir_paths.push(Vec::new()); // files/leaves don't store paths

            control
                .files_seen
                .fetch_add(u64::from(node_kind == kind::FILE), Ordering::Relaxed);
            control.bytes_seen.fetch_add(
                if node_kind == kind::FILE {
                    meta.alloc
                } else {
                    0
                },
                Ordering::Relaxed,
            );
            if node_kind == kind::DIR {
                control.dirs_seen.fetch_add(1, Ordering::Relaxed);
            }
            current_path = String::from_utf16_lossy(&name16);

            delta_buf.push(NodeDelta {
                id: nid,
                parent: dir_node,
                name: name_str.clone(),
                depth: 0, // renderer computes depth from parent chain
                logical: meta.size,
                allocated: meta.alloc,
                kind: arena.kind(nid),
            });

            if (node_kind == kind::DIR || node_kind == kind::REPARSE) && descend {
                let abs = join_path(dir_paths[dir_node as usize].as_slice(), &name16);
                dir_paths[nid as usize] = abs.clone();
                new_dirs.push((nid, abs));
            }

            // files complete immediately → roll into the parent now
            if node_kind == kind::FILE {
                arena.add_metrics(dir_node, meta.size, meta.alloc, 1, 0);
            }
            arena.bump_mtime(dir_node, meta.mtime);
        }

        // queue child dirs (each marks itself "in flight")
        for (nid, path) in new_dirs {
            pending[dir_node as usize] += 1;
            pending[nid as usize] = 1;
            if task_tx
                .send(DirTask {
                    node: nid,
                    path,
                    depth: 0,
                })
                .is_ok()
            {
                outstanding += 1;
            }
        }

        // completion cascade for this directory
        if pending[dir_node as usize] == 0 {
            complete_cascade(&mut arena, &mut pending, dir_node, &mut delta_buf);
        }

        // batched event emission
        if delta_buf.len() >= 512 {
            let _ = sink.emit(EngineEvent::ScanNodes {
                nodes: ScanNodes {
                    scan_id: id,
                    deltas: std::mem::take(&mut delta_buf),
                },
            });
        }
        if last_progress.elapsed() >= std::time::Duration::from_millis(250) {
            last_progress = std::time::Instant::now();
            let elapsed = started.elapsed().as_millis().max(1) as u64;
            let files = control.files_seen.load(Ordering::Relaxed);
            let _ = sink.emit(EngineEvent::ScanProgress {
                progress: ScanProgress {
                    scan_id: id,
                    files_seen: files,
                    bytes_seen: control.bytes_seen.load(Ordering::Relaxed),
                    dirs_seen: control.dirs_seen.load(Ordering::Relaxed),
                    current_path: current_path.clone(),
                    elapsed_ms: elapsed,
                    rate_files_per_sec: files as f64 * 1000.0 / elapsed as f64,
                },
            });
        }
    }

    // --- walk done: workers out, pseudo-nodes in, then CSR finalize -----------
    drop(task_tx);
    for w in workers {
        let _ = w.join();
    }
    *phase.lock() = ScanPhase::Aggregating;
    let _ = sink.emit(EngineEvent::ScanPhase {
        phase: ScanPhaseEvent {
            scan_id: id,
            phase: ScanPhase::Aggregating,
            detail: None,
        },
    });
    if !delta_buf.is_empty() {
        let _ = sink.emit(EngineEvent::ScanNodes {
            nodes: ScanNodes {
                scan_id: id,
                deltas: std::mem::take(&mut delta_buf),
            },
        });
    }

    // free-space pseudo-node (parity-SCN-04) + unknown (parity-SCN-05)
    let free_bytes = crate::sysinfo::volume_free_bytes(&roots[0]);
    {
        let name16: Vec<u16> = "<free space>".encode_utf16().collect();
        let fid = arena.push(NodeInput {
            parent: root_id,
            name_utf16: &name16,
            logical: free_bytes,
            allocated: free_bytes,
            files: 0,
            folders: 0,
            mtime: 0,
            kind: kind::FREE_SPACE,
            category: prism_types::ids::CATEGORY_FREE_SPACE,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        arena.attach(root_id, fid);
        pending.push(0);
        dir_paths.push(Vec::new());
        if !errors.is_empty() {
            let name16: Vec<u16> = "<unknown>".encode_utf16().collect();
            let uid = arena.push(NodeInput {
                parent: root_id,
                name_utf16: &name16,
                logical: 0,
                allocated: 0,
                files: 0,
                folders: 0,
                mtime: 0,
                kind: kind::UNKNOWN,
                category: prism_types::ids::CATEGORY_UNKNOWN,
                ext_id: 0,
                attr_flags: 0,
                link_to: 0,
                err_code: 1,
            });
            arena.attach(root_id, uid);
            pending.push(0);
            dir_paths.push(Vec::new());
        }
    }

    arena.finalize_children();
    *phase.lock() = ScanPhase::IndexingExt;
    let aggregates = agg::compute(&arena, &ext_table);

    let root_allocated = arena.allocated(root_id as usize);
    let summary = ScanSummary {
        scan_id: id,
        root: roots[0].clone(),
        strategy: ScanStrategy::Standard,
        size_mode: options.size_mode,
        files: control.files_seen.load(Ordering::Relaxed),
        folders: control.dirs_seen.load(Ordering::Relaxed),
        logical: arena.logical(root_id),
        allocated: root_allocated,
        unique: root_allocated,
        unknown: 0,
        free: free_bytes,
        duration_ms: started.elapsed().as_millis() as u64,
        errors: errors.len() as u32,
        truncated,
    };

    Ok(Arc::new(CompletedScan {
        arena: Arc::new(arena),
        ext_table: Arc::new(ext_table),
        aggregates: Arc::new(aggregates),
        summary,
        errors,
    }))
}

/// Complete a directory and cascade upward, emitting refresh deltas
/// (live sizes, parity-SCN-02).
fn complete_cascade(
    arena: &mut Arena,
    pending: &mut [u32],
    node: NodeId,
    deltas: &mut Vec<NodeDelta>,
) {
    let mut cur = node;
    let mut guard = 0usize;
    let n = arena.len();
    loop {
        let own_logical = arena.logical(cur);
        let own_alloc = arena.allocated(cur as usize);
        let own_files = arena.files(cur);
        let own_folders = arena.folders(cur);
        deltas.push(NodeDelta {
            id: cur,
            parent: arena.parent(cur),
            name: arena.name_str(cur),
            depth: 0,
            logical: own_logical,
            allocated: own_alloc,
            kind: arena.kind(cur),
        });
        let parent = arena.parent(cur);
        if parent == cur {
            break;
        }
        arena.add_metrics(parent, own_logical, own_alloc, own_files, own_folders);
        let p = pending[parent as usize];
        if p > 0 {
            pending[parent as usize] = p - 1;
        }
        if pending[parent as usize] == 0 {
            cur = parent;
        } else {
            break;
        }
        guard += 1;
        if guard > n {
            break; // defensive; structure invariants are property-tested
        }
    }
}

/// Root-relative prefix for a directory path (ends with `/` or is empty).
fn rel_dir_prefix(roots: &[String], dir_abs: &[u16]) -> String {
    let dir = String::from_utf16_lossy(dir_abs);
    for r in roots {
        if let Some(stripped) = dir.strip_prefix(r.as_str()) {
            let s = stripped.trim_start_matches(['\\', '/']);
            return if s.is_empty() {
                String::new()
            } else {
                format!("{}/", s.replace('\\', "/"))
            };
        }
    }
    String::new()
}

/// Platform path separator (UTF-16): `\` on Windows, `/` elsewhere.
const SEP: u16 = if cfg!(windows) { 0x5C } else { 0x2F };

fn join_path(base: &[u16], name: &[u16]) -> Vec<u16> {
    let mut out = Vec::with_capacity(base.len() + name.len() + 1);
    out.extend_from_slice(base);
    if !out.is_empty()
        && !out.ends_with(&[SEP])
        && !out.ends_with(&[0x5C])
        && !out.ends_with(&[0x2F])
    {
        out.push(SEP);
    }
    out.extend_from_slice(name);
    out
}

fn root_name(path: &str) -> String {
    let p = path.trim_end_matches(['\\', '/']);
    if p.is_empty() {
        return path.to_string();
    }
    match p.rsplit(['\\', '/']).next() {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => p.to_string(),
    }
}

fn home_dir() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

/// Platform-selected enumerator factory (one per worker).
pub fn make_enumerator() -> Box<dyn DirEnumerator> {
    #[cfg(unix)]
    {
        Box::new(crate::scanner::posix::PosixDevEnumerator::new(0))
    }
    #[cfg(windows)]
    {
        Box::new(crate::scanner::win32::Win32NtEnumerator::new(0))
    }
    #[cfg(not(any(unix, windows)))]
    {
        compile_error!("PRISM engine supports windows (release) and unix (development) only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_name_extraction() {
        assert_eq!(root_name("C:\\"), "C:");
        assert_eq!(root_name("/home/user/data/"), "data");
        assert_eq!(root_name("/"), "/");
    }
}
