//! Monitor (PRISM-HG-060, docs/12 § 7): 1 Hz system + process sampling.
//!
//! Windows: `NtQuerySystemInformation` (SystemProcessInformation +
//! SystemPerformanceInformation) — the plan's mechanism. Dev platform
//! (AMM-002): `/proc` — same DTOs, same delta math, so the renderer and the
//! event contract are exercised end-to-end on the dev host.
//!
//! Informational only (no kill/affinity — docs/10 § 8: no destructive
//! surface on a diagnostics pane).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use prism_types::events::{EngineEvent, MonitorSample, ProcessSample};

use crate::ipc::EngineEventSink;

/// A running monitor session (stop drops the thread).
pub struct MonitorHandle {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl MonitorHandle {
    /// Stop and join (≤ period + 50 ms).
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Start sampling every `period_ms` (min 250), emitting `monitor:sample`.
pub fn start(sink: Arc<EngineEventSink>, period_ms: u32) -> MonitorHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let period = Duration::from_millis(period_ms.max(250) as u64);
    let stop2 = Arc::clone(&stop);
    let join = std::thread::Builder::new()
        .name("prism-monitor".into())
        .spawn(move || {
            let mut state = SampleState::default();
            while !stop2.load(Ordering::Relaxed) {
                let t0 = Instant::now();
                let sample = state.sample();
                let _ = sink.emit(EngineEvent::MonitorSample { sample });
                let elapsed = t0.elapsed();
                if period > elapsed {
                    std::thread::sleep(period - elapsed);
                }
            }
        })
        .ok();
    MonitorHandle { stop, join }
}

// ---------------------------------------------------------------------------
// delta state shared by both backends
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SampleState {
    prev: Option<PrevSample>,
}

/// Per-pid previous counters (delta base).
struct PrevProc {
    cpu_ticks: u64,
    read: u64,
    write: u64,
}

struct PrevSample {
    at: Instant,
    procs: HashMap<u32, PrevProc>,
    cpu_idle: u64,
    cpu_total: u64,
}

impl SampleState {
    fn sample(&mut self) -> MonitorSample {
        let now_wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let raw = sample_raw();
        let now = Instant::now();

        let (cpu_total, procs) = match (&self.prev, raw) {
            (
                Some(prev),
                RawSample::Win {
                    procs,
                    cpu_idle,
                    cpu_total_ticks,
                },
            ) => {
                let dt = now.duration_since(prev.at).as_millis().max(1) as u64;
                let d_idle = cpu_idle.saturating_sub(prev.cpu_idle);
                let d_total = cpu_total_ticks.saturating_sub(prev.cpu_total);
                let cpu = if d_total > 0 {
                    1.0 - (d_idle as f32 / d_total as f32)
                } else {
                    0.0
                };
                (cpu.clamp(0.0, 1.0), join_procs(&prev.procs, procs, dt))
            }
            (
                Some(prev),
                RawSample::Proc {
                    procs,
                    cpu_idle,
                    cpu_total_ticks,
                },
            ) => {
                let dt = now.duration_since(prev.at).as_millis().max(1) as u64;
                let d_idle = cpu_idle.saturating_sub(prev.cpu_idle);
                let d_total = cpu_total_ticks.saturating_sub(prev.cpu_total);
                let cpu = if d_total > 0 {
                    1.0 - (d_idle as f32 / d_total as f32)
                } else {
                    0.0
                };
                (cpu.clamp(0.0, 1.0), join_procs(&prev.procs, procs, dt))
            }
            (None, _) => (0.0, Vec::new()), // first sample: no deltas yet
        };

        self.prev = Some(PrevSample {
            at: now,
            procs: raw_prev_procs(),
            cpu_idle: raw_idle(),
            cpu_total: raw_total(),
        });

        let mut procs = procs;
        procs.sort_by(|a, b| {
            b.cpu
                .partial_cmp(&a.cpu)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        procs.truncate(40);

        MonitorSample {
            ts: now_wall,
            cpu_total,
            mem_total: mem_stats().0,
            mem_used: mem_stats().1,
            disk_read_bps: procs.iter().map(|p| p.read_bps).sum(),
            disk_write_bps: procs.iter().map(|p| p.write_bps).sum(),
            processes: procs,
        }
    }
}

/// Raw per-process counters before delta computation.
struct RawProc {
    /// PID.
    pid: u32,
    /// Process name.
    name: String,
    /// CPU time ms.
    cpu_ticks: u64,
    /// Working set bytes.
    working_set: u64,
    /// Cumulative read bytes.
    read: u64,
    /// Cumulative write bytes.
    write: u64,
    /// Thread count.
    threads: u32,
}

#[allow(dead_code)] // Win/Proc variants are per-platform (cfg) constructed
enum RawSample {
    Win {
        procs: Vec<RawProc>,
        cpu_idle: u64,
        cpu_total_ticks: u64,
    },
    Proc {
        procs: Vec<RawProc>,
        cpu_idle: u64,
        cpu_total_ticks: u64,
    },
}

fn join_procs(prev: &HashMap<u32, PrevProc>, cur: Vec<RawProc>, dt_ms: u64) -> Vec<ProcessSample> {
    let secs = dt_ms as f32 / 1000.0;
    cur.into_iter()
        .filter_map(|p| {
            let base = prev.get(&p.pid)?;
            let d_cpu = p.cpu_ticks.saturating_sub(base.cpu_ticks);
            let d_read = p.read.saturating_sub(base.read);
            let d_write = p.write.saturating_sub(base.write);
            Some(ProcessSample {
                pid: p.pid,
                name: p.name,
                // cpu_ticks are ms in both backends → fraction of one core.
                cpu: (d_cpu as f32 / 1000.0 / secs).max(0.0),
                working_set: p.working_set,
                read_bps: (d_read as f64 / secs as f64) as u64,
                write_bps: (d_write as f64 / secs as f64) as u64,
                threads: p.threads,
            })
        })
        .collect()
}

// The prev-side accessors exist because borrowck wants `raw` consumed once;
// we re-derive the prev bookkeeping from the same sample instead of cloning.
fn raw_prev_procs() -> HashMap<u32, PrevProc> {
    LAST_RAW.with(|c| {
        c.borrow()
            .iter()
            .map(|p| {
                (
                    p.pid,
                    PrevProc {
                        cpu_ticks: p.cpu_ticks,
                        read: p.read,
                        write: p.write,
                    },
                )
            })
            .collect()
    })
}

thread_local! {
    static LAST_RAW: std::cell::RefCell<Vec<RawProc>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn raw_idle() -> u64 {
    LAST_IDLE.with(|c| c.get())
}

fn raw_total() -> u64 {
    LAST_TOTAL.with(|c| c.get())
}

thread_local! {
    static LAST_IDLE: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static LAST_TOTAL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

// ---------------------------------------------------------------------------
// Windows backend — NtQuerySystemInformation (docs/06 § 5)
// ---------------------------------------------------------------------------

#[cfg(windows)]
fn sample_raw() -> RawSample {
    use windows_sys::Win32::Foundation::NTSTATUS;
    use windows_sys::Win32::System::SystemInformation::{
        NtQuerySystemInformation, SYSTEM_PROCESS_INFORMATION, SystemProcessInformation,
    };

    #[repr(C)]
    #[allow(non_snake_case, non_camel_case_types)]
    struct SYSTEM_PROCESS_INFORMATION_W {
        NextEntryOffset: u32,
        ThreadCount: u32,
        WorkingSetPrivateSize: usize,
        HardFaultCount: u32,
        NumberOfThreadsHighWatermark: u32,
        CycleTime: u64,
        CreateTime: i64,
        UserTime: i64,
        KernelTime: i64,
        ImageName: windows_sys::core::UNICODE_STRING,
        BasePriority: i32,
        UniqueProcessId: *mut core::ffi::c_void,
        InheritedFromUniqueProcessId: *mut core::ffi::c_void,
        HandleCount: u32,
        SessionId: u32,
        UniqueProcessKey: *mut core::ffi::c_void,
        PeakVirtualSize: usize,
        VirtualSize: usize,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
        PrivatePageCount: usize,
        ReadOperationCount: u64,
        WriteOperationCount: u64,
        OtherOperationCount: u64,
        ReadTransferCount: u64,
        WriteTransferCount: u64,
        OtherTransferCount: u64,
    }

    // Buffer loop: the API returns STATUS_INFO_LENGTH_MISMATCH until big enough.
    let mut len = 1 << 20u32;
    loop {
        let mut buf = vec![0u8; len as usize];
        let mut ret = 0u32;
        let st: NTSTATUS = unsafe {
            NtQuerySystemInformation(
                SystemProcessInformation,
                buf.as_mut_ptr().cast(),
                len,
                &mut ret,
            )
        };
        if st >= 0 {
            // Walk entries.
            let mut procs = Vec::new();
            let mut off = 0usize;
            loop {
                let p = unsafe { &*(buf.as_ptr().add(off).cast::<SYSTEM_PROCESS_INFORMATION_W>()) };
                let pid = p.UniqueProcessId as u32;
                if pid != 0 {
                    let name_len = p.ImageName.Length as usize / 2;
                    let name = if name_len > 0 && !p.ImageName.Buffer.is_null() {
                        let slice =
                            unsafe { std::slice::from_raw_parts(p.ImageName.Buffer, name_len) };
                        String::from_utf16_lossy(slice)
                    } else {
                        "System".to_string()
                    };
                    procs.push(RawProc {
                        pid,
                        name,
                        cpu_ticks: (p.UserTime + p.KernelTime) as u64 / 100, // 100ns → ms ticks
                        working_set: p.WorkingSetSize as u64,
                        read: p.ReadTransferCount,
                        write: p.WriteTransferCount,
                        threads: p.ThreadCount,
                    });
                }
                let next = p.NextEntryOffset as usize;
                if next == 0 {
                    break;
                }
                off += next;
            }
            let (idle, total) = cpu_times_win();
            let raw = RawSample::Win {
                procs,
                cpu_idle: idle,
                cpu_total_ticks: total,
            };
            stash_raw(&raw);
            return raw;
        }
        if ret > len {
            len = ret + 65536;
        } else {
            len = len.saturating_mul(2);
        }
        if len > (64 << 20) {
            return RawSample::Win {
                procs: Vec::new(),
                cpu_idle: 0,
                cpu_total_ticks: 0,
            };
        }
    }
}

#[cfg(windows)]
fn stash_raw(r: &RawSample) {
    match r {
        RawSample::Win {
            procs,
            cpu_idle,
            cpu_total_ticks,
        } => {
            LAST_RAW.with(|c| {
                *c.borrow_mut() = procs
                    .iter()
                    .map(|p| RawProc {
                        pid: p.pid,
                        name: p.name.clone(),
                        cpu_ticks: p.cpu_ticks,
                        working_set: p.working_set,
                        read: p.read,
                        write: p.write,
                        threads: p.threads,
                    })
                    .collect()
            });
            LAST_IDLE.with(|c| c.set(*cpu_idle));
            LAST_TOTAL.with(|c| c.set(*cpu_total_ticks));
        }
        RawSample::Proc { .. } => unreachable!("windows backend only produces Win"),
    }
}

#[cfg(windows)]
fn cpu_times_win() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{FILETIME, GetSystemTimes};
    let mut idle = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut kern = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut user = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let ok = unsafe { GetSystemTimes(&mut idle, &mut kern, &mut user) };
    if ok == 0 {
        return (0, 0);
    }
    let to_u64 = |f: &FILETIME| (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64;
    let idle_t = to_u64(&idle);
    // Kernel FILETIME includes idle; total = user + kernel.
    let total = to_u64(&user) + to_u64(&kern);
    (idle_t / 10_000, total / 10_000) // → ms
}

#[cfg(windows)]
fn mem_stats() -> (u64, u64) {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut ms = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        dwMemoryLoad: 0,
        ullTotalPhys: 0,
        ullAvailPhys: 0,
        ullTotalPageFile: 0,
        ullAvailPageFile: 0,
        ullTotalVirtual: 0,
        ullAvailVirtual: 0,
        ullAvailExtendedVirtual: 0,
    };
    if unsafe { GlobalMemoryStatusEx(&mut ms) } == 0 {
        return (0, 0);
    }
    (ms.ullTotalPhys, ms.ullTotalPhys - ms.ullAvailPhys)
}

// ---------------------------------------------------------------------------
// Dev backend (AMM-002): /proc — Linux; empty sample on other non-Windows.
// ---------------------------------------------------------------------------

#[cfg(not(windows))]
fn sample_raw() -> RawSample {
    #[cfg(target_os = "linux")]
    {
        let procs = proc_processes();
        let (cpu_idle, total) = proc_cpu_times();
        let raw = RawSample::Proc {
            procs,
            cpu_idle,
            cpu_total_ticks: total,
        };
        stash_raw(&raw);
        raw
    }
    #[cfg(not(target_os = "linux"))]
    {
        RawSample::Proc {
            procs: Vec::new(),
            cpu_idle: 0,
            cpu_total_ticks: 0,
        }
    }
}

#[cfg(not(windows))]
fn stash_raw(r: &RawSample) {
    match r {
        RawSample::Proc {
            procs,
            cpu_idle,
            cpu_total_ticks,
        } => {
            LAST_RAW.with(|c| {
                *c.borrow_mut() = procs
                    .iter()
                    .map(|p| RawProc {
                        pid: p.pid,
                        name: p.name.clone(),
                        cpu_ticks: p.cpu_ticks,
                        working_set: p.working_set,
                        read: p.read,
                        write: p.write,
                        threads: p.threads,
                    })
                    .collect()
            });
            LAST_IDLE.with(|c| c.set(*cpu_idle));
            LAST_TOTAL.with(|c| c.set(*cpu_total_ticks));
        }
        RawSample::Win { .. } => unreachable!("dev backend only produces Proc"),
    }
}

#[cfg(target_os = "linux")]
fn proc_cpu_times() -> (u64, u64) {
    // /proc/stat: "cpu  user nice system idle iowait irq softirq steal …"
    let Ok(text) = std::fs::read_to_string("/proc/stat") else {
        return (0, 0);
    };
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            let fields: Vec<u64> = rest
                .split_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect();
            let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0); // idle + iowait
            let total: u64 = fields.iter().sum();
            return (idle, total);
        }
    }
    (0, 0)
}

#[cfg(target_os = "linux")]
fn proc_processes() -> Vec<RawProc> {
    let mut out = Vec::new();
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return out;
    };
    let clk_tck: u64 = 100; // USER_HZ on every modern Linux
    for ent in dir.flatten() {
        let name = ent.file_name();
        let name = name.to_string_lossy();
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
            continue;
        };
        // comm is in parens and may contain spaces — split after the last ')'.
        let Some(comm_end) = stat.rfind(')') else {
            continue;
        };
        let after = &stat[comm_end + 1..];
        let f: Vec<&str> = after.split_whitespace().collect();
        // fields[11] utime, [12] stime (1-indexed from state) → after ')' the
        // first token is `state`; utime = index 11, stime = 12.
        let utime: u64 = f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
        let stime: u64 = f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
        let threads: u32 = f.get(17).and_then(|v| v.parse().ok()).unwrap_or(1);
        let rss_pages: u64 = f.get(20).and_then(|v| v.parse().ok()).unwrap_or(0);
        let page = 4096u64;

        let pcomm = stat[stat.find('(').map(|i| i + 1).unwrap_or(0)..comm_end].to_string();
        let proc_name = if pcomm.len() > 15 {
            std::fs::read_link(format!("/proc/{pid}/exe"))
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .unwrap_or(pcomm)
        } else {
            pcomm
        };

        let read = std::fs::read_to_string(format!("/proc/{pid}/io"))
            .ok()
            .and_then(|io| {
                io.lines()
                    .find(|l| l.starts_with("read_bytes:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse().ok())
            })
            .unwrap_or(0u64);
        let write = std::fs::read_to_string(format!("/proc/{pid}/io"))
            .ok()
            .and_then(|io| {
                io.lines()
                    .find(|l| l.starts_with("write_bytes:"))
                    .and_then(|l| l.split(':').nth(1))
                    .and_then(|v| v.trim().parse().ok())
            })
            .unwrap_or(0u64);

        out.push(RawProc {
            pid,
            name: proc_name,
            // Integer division: tick→ms truncation is the correct grain.
            #[allow(clippy::integer_division)]
            cpu_ticks: (utime + stime) * 1000 / clk_tck,
            working_set: rss_pages * page,
            read,
            write,
            threads,
        });
    }
    out
}

#[cfg(not(windows))]
fn mem_stats() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
            return (0, 0);
        };
        let mut total = 0u64;
        let mut available = 0u64;
        for line in text.lines() {
            if let Some(v) = line.strip_prefix("MemTotal:") {
                total = v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0)
                    * 1024;
            } else if let Some(v) = line.strip_prefix("MemAvailable:") {
                available = v
                    .split_whitespace()
                    .next()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0)
                    * 1024;
            }
        }
        (total, total.saturating_sub(available))
    }
    #[cfg(not(target_os = "linux"))]
    {
        (0, 0)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn proc_backend_produces_samples() {
        // Two samples on the dev host: the second must carry delta-based
        // rows (possibly empty if nothing changed — but never panics and
        // always has sane memory numbers).
        use super::*;
        let mut st = SampleState::default();
        let _first = st.sample();
        std::thread::sleep(Duration::from_millis(300));
        let second = st.sample();
        assert!(second.mem_total > 0, "meminfo parsed on Linux");
        assert!(second.cpu_total >= 0.0 && second.cpu_total <= 1.0);
    }
}
