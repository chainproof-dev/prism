//! # Scheduler (PRISM-HG-080 — docs/12 § 9)
//!
//! Background scan scheduling that survives app closure. The spec list is
//! persisted in the engine DB (portable, testable); on Windows each enabled
//! spec is mirrored into a Task Scheduler entry named `PRISM\<id>` whose
//! action is — BY CONSTRUCTION — `"<runner_exe>" --background-scan
//! "<target>"`. There is no API surface that can schedule anything else:
//! **no scheduled cleanup, ever** ([01 § 8] scope boundary; the upsert query
//! carries no action field at all).
//!
//! Off-Windows the backend is an honest `dev-file` stub: specs persist and
//! list round-trips work (UI flows + tests), but nothing fires — `backend`
//! in the page response says so.
//!
//! Firing semantics on Windows are owned by Task Scheduler itself (local
//! time, DST, logon triggers). The `next_run_ms` computation here is for
//! display only and is computed from the trigger math (UTC-offset aware).

use std::path::Path;

use prism_types::commands::{ScheduleDigest, ScheduleSpec, ScheduleTrigger, SchedulerPage};

use crate::error::EngineError;
use crate::persistence::Db;

/// Engine-DB settings key holding the spec list (JSON array).
const SPECS_KEY: &str = "scheduler.specs";
/// Task Scheduler folder + prefix for mirrored entries.
const TASK_PREFIX: &str = "PRISM\\";

// ---------------------------------------------------------------------------
// validation
// ---------------------------------------------------------------------------

/// Validate a spec (label, target, trigger ranges). Returns a normalized
/// copy (weekly days deduped + sorted) or a validation error.
pub fn normalize_spec(spec: &ScheduleSpec) -> Result<ScheduleSpec, EngineError> {
    let label = spec.label.trim();
    if label.is_empty() {
        return Err(EngineError::Internal(
            "scheduler: label must be non-empty".into(),
        ));
    }
    if label.chars().count() > 120 {
        return Err(EngineError::Internal(
            "scheduler: label exceeds 120 chars".into(),
        ));
    }
    let target = spec.target.trim();
    if target.is_empty() {
        return Err(EngineError::Internal(
            "scheduler: target must be non-empty".into(),
        ));
    }
    if !is_absolute_path(target) {
        return Err(EngineError::Internal(format!(
            "scheduler: target must be absolute: {target}"
        )));
    }
    let trigger = normalize_trigger(&spec.trigger)?;
    Ok(ScheduleSpec {
        id: spec.id.clone(),
        label: label.to_string(),
        target: target.to_string(),
        trigger,
        enabled: spec.enabled,
        last_run_ms: spec.last_run_ms,
    })
}

fn normalize_trigger(t: &ScheduleTrigger) -> Result<ScheduleTrigger, EngineError> {
    match t {
        ScheduleTrigger::Daily { time_min } => {
            if *time_min > 1439 {
                return Err(EngineError::Internal(format!(
                    "scheduler: time_min {time_min} out of 0..=1439"
                )));
            }
            Ok(ScheduleTrigger::Daily {
                time_min: *time_min,
            })
        }
        ScheduleTrigger::Weekly { days, time_min } => {
            if *time_min > 1439 {
                return Err(EngineError::Internal(format!(
                    "scheduler: time_min {time_min} out of 0..=1439"
                )));
            }
            let mut days = days.clone();
            days.retain(|d| (1..=7).contains(d));
            if days.is_empty() {
                return Err(EngineError::Internal(
                    "scheduler: weekly needs >=1 day (ISO 1..=7)".into(),
                ));
            }
            days.sort_unstable();
            days.dedup();
            Ok(ScheduleTrigger::Weekly {
                days,
                time_min: *time_min,
            })
        }
        ScheduleTrigger::AtLogon {} => Ok(ScheduleTrigger::AtLogon {}),
    }
}

/// Absolute = starts with `/` (posix) or `X:\`/`X:/` (windows drive).
fn is_absolute_path(p: &str) -> bool {
    if p.starts_with('/') || p.starts_with("\\\\") {
        return true;
    }
    let b = p.as_bytes();
    b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
}

/// Random-looking id (no uuid dep): time + counter + pid hash, v4-shaped.
fn new_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let c = CTR.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id() as u64;
    let mut h = t ^ (c << 32) ^ (pid.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    // xorshift-mix to spread bits, then format as 8-4-4-4-12 hex.
    let mut out = String::with_capacity(36);
    for i in 0..36 {
        let ch = match i {
            8 | 13 | 18 | 23 => '-',
            14 => '4', // version nibble
            _ => {
                h ^= h << 13;
                h ^= h >> 7;
                h ^= h << 17;
                const HEX: &[u8; 16] = b"0123456789abcdef";
                HEX[(h >> 60) as usize] as char
            }
        };
        out.push(ch);
    }
    out
}

// ---------------------------------------------------------------------------
// store (engine DB settings row — source of truth on every platform)
// ---------------------------------------------------------------------------

/// Load all specs (creation order).
pub fn load_specs(db: &Db) -> Vec<ScheduleSpec> {
    let Some(raw) = db.get_setting(SPECS_KEY) else {
        return Vec::new();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn store_specs(db: &Db, specs: &[ScheduleSpec]) -> Result<(), EngineError> {
    let json = serde_json::to_string(specs)
        .map_err(|e| EngineError::Internal(format!("scheduler serialize: {e}")))?;
    db.set_setting(SPECS_KEY, &json)
}

/// `scheduler:list`.
pub fn list(db: &Db) -> SchedulerPage {
    SchedulerPage {
        schedules: load_specs(db),
        backend: backend_name().to_string(),
    }
}

/// Which registration backend is live.
pub fn backend_name() -> &'static str {
    if cfg!(windows) {
        "task-scheduler"
    } else {
        "dev-file"
    }
}

/// `scheduler:upsert` — validate, persist, mirror into Task Scheduler
/// (Windows). `spec.id` empty = create (id assigned here). Returns the
/// final stored spec.
pub fn upsert(db: &Db, spec: &ScheduleSpec, runner_exe: &str) -> Result<ScheduleSpec, EngineError> {
    let mut norm = normalize_spec(spec)?;
    let creating = norm.id.is_empty();
    if creating {
        norm.id = new_id();
    }
    if cfg!(windows) && norm.enabled {
        register_task(&norm, runner_exe)?;
    } else if cfg!(windows) && !norm.enabled {
        // Disabled spec: make sure no live task lingers.
        let _ = unregister_task(&norm.id);
    }
    let mut specs = load_specs(db);
    if let Some(slot) = specs.iter_mut().find(|s| s.id == norm.id) {
        *slot = norm.clone();
    } else {
        specs.push(norm.clone());
    }
    store_specs(db, &specs)?;
    Ok(norm)
}

/// `scheduler:delete`.
pub fn delete(db: &Db, id: &str) -> Result<(), EngineError> {
    if cfg!(windows) {
        unregister_task(id)?;
    }
    let mut specs = load_specs(db);
    let before = specs.len();
    specs.retain(|s| s.id != id);
    if specs.len() == before {
        return Err(EngineError::Internal(format!(
            "scheduler: no schedule {id}"
        )));
    }
    store_specs(db, &specs)
}

// ---------------------------------------------------------------------------
// Windows Task Scheduler mirroring (schtasks.exe — no COM surface needed)
// ---------------------------------------------------------------------------

/// Build the exact `/TR` action argument. Scope guard anchor: this is the
/// ONLY place a command line is ever constructed, and it is always a
/// background STANDARD scan of the spec target.
pub fn task_action(runner_exe: &str, target: &str) -> String {
    format!("\"{runner_exe}\" --background-scan \"{target}\"")
}

/// schtasks argument list for one spec (pure builder — unit-tested on all
/// platforms; only executed on Windows).
pub fn schtasks_create_args(spec: &ScheduleSpec, runner_exe: &str) -> Vec<String> {
    let mut args = vec![
        "/Create".into(),
        "/F".into(),
        "/TN".into(),
        format!("{TASK_PREFIX}{}", spec.id),
        "/TR".into(),
        task_action(runner_exe, &spec.target),
    ];
    match &spec.trigger {
        ScheduleTrigger::Daily { time_min } => {
            args.push("/SC".into());
            args.push("DAILY".into());
            args.push("/ST".into());
            args.push(hhmm(*time_min));
        }
        ScheduleTrigger::Weekly { days, time_min } => {
            args.push("/SC".into());
            args.push("WEEKLY".into());
            args.push("/D".into());
            args.push(
                days.iter()
                    .map(|d| iso_day_name(*d))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            args.push("/ST".into());
            args.push(hhmm(*time_min));
        }
        ScheduleTrigger::AtLogon {} => {
            args.push("/SC".into());
            args.push("ONLOGON".into());
        }
    }
    args
}

/// schtasks `/ST` value. Integer division is exact (minute-of-day → HH:MM).
#[allow(clippy::integer_division)]
fn hhmm(time_min: u16) -> String {
    format!("{:02}:{:02}", time_min / 60, time_min % 60)
}

/// ISO weekday number → schtasks English day token (locale-stable: the
/// `/D` tokens are fixed strings on all schtasks locales).
fn iso_day_name(d: u8) -> &'static str {
    match d {
        1 => "MON",
        2 => "TUE",
        3 => "WED",
        4 => "THU",
        5 => "FRI",
        6 => "SAT",
        _ => "SUN",
    }
}

#[cfg(windows)]
fn register_task(spec: &ScheduleSpec, runner_exe: &str) -> Result<(), EngineError> {
    let out = std::process::Command::new("schtasks.exe")
        .args(schtasks_create_args(spec, runner_exe))
        .output()
        .map_err(|e| EngineError::Internal {
            msg: format!("schtasks spawn: {e}"),
        })?;
    if !out.status.success() {
        return Err(EngineError::Internal {
            msg: format!(
                "schtasks create failed ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn unregister_task(id: &str) -> Result<(), EngineError> {
    let out = std::process::Command::new("schtasks.exe")
        .args(["/Delete", "/F", "/TN", &format!("{TASK_PREFIX}{id}")])
        .output()
        .map_err(|e| EngineError::Internal {
            msg: format!("schtasks spawn: {e}"),
        })?;
    // Deleting a missing task is success for our idempotency contract.
    if !out.status.success() && !String::from_utf8_lossy(&out.stderr).contains("cannot find") {
        return Err(EngineError::Internal {
            msg: format!(
                "schtasks delete failed ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    Ok(())
}

#[cfg(not(windows))]
fn register_task(_spec: &ScheduleSpec, _runner_exe: &str) -> Result<(), EngineError> {
    Ok(()) // dev-file backend: nothing fires off-Windows (honest stub)
}

#[cfg(not(windows))]
fn unregister_task(_id: &str) -> Result<(), EngineError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// next-run math (display only; Task Scheduler owns actual firing)
// ---------------------------------------------------------------------------

/// Current local-time UTC offset in minutes (DST-aware on Windows; UTC
/// off-Windows where the backend is a dev stub anyway).
pub fn utc_offset_minutes() -> i32 {
    #[cfg(windows)]
    {
        unsafe {
            use windows_sys::Win32::System::Time::{
                GetTimeZoneInformation, TIME_ZONE_ID_DAYLIGHT, TIME_ZONE_ID_STANDARD,
                TIME_ZONE_INFORMATION,
            };
            let mut tzi: TIME_ZONE_INFORMATION = std::mem::zeroed();
            let rc = GetTimeZoneInformation(&mut tzi);
            // Win32 bias: UTC = local + Bias → local offset = -Bias.
            // During daylight, add DaylightBias (typically -60).
            let extra = if rc == TIME_ZONE_ID_DAYLIGHT {
                tzi.DaylightBias
            } else if rc == TIME_ZONE_ID_STANDARD {
                tzi.StandardBias
            } else {
                0
            };
            -(tzi.Bias + extra)
        }
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Days since 1970-01-01 → (year, month, day). Howard Hinnant's civil_from_days.
/// Integer division is exact — the algorithm is defined in integer terms.
#[allow(clippy::integer_division)]
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// (year, month, day) → days since epoch (days_from_civil).
/// Integer division is exact — the algorithm is defined in integer terms.
#[allow(clippy::integer_division)]
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Weekday (0=Sunday .. 6=Saturday) of a days-since-epoch value.
fn weekday(days: i64) -> u32 {
    // 1970-01-01 was a Thursday (4).
    (days + 4).rem_euclid(7) as u32
}

/// ISO weekday (1=Mon..7=Sun) of a days-since-epoch value.
fn iso_weekday(days: i64) -> u8 {
    let w = weekday(days); // 0=Sun
    if w == 0 { 7 } else { w as u8 }
}

/// Next fire time (unix ms) for a trigger after `now_ms`; `None` for
/// AtLogon (fires at an unknowable future logon). Integer division is
/// exact (ms-of-day → minute-of-day).
#[allow(clippy::integer_division)]
pub fn next_run_ms(trigger: &ScheduleTrigger, now_ms: i64) -> Option<i64> {
    let off = utc_offset_minutes() as i64;
    let local_now = now_ms + off * 60_000;
    let days = local_now.div_euclid(86_400_000);
    let today_min = local_now.rem_euclid(86_400_000) / 60_000;
    let (fire_day, fire_min) = match trigger {
        ScheduleTrigger::AtLogon {} => return None,
        ScheduleTrigger::Daily { time_min } => {
            if *time_min as i64 > today_min {
                (days, *time_min as i64)
            } else {
                (days + 1, *time_min as i64)
            }
        }
        ScheduleTrigger::Weekly { days: wd, time_min } => {
            for probe in (days..).take(8) {
                let iso = iso_weekday(probe);
                if wd.contains(&iso) && (probe > days || *time_min as i64 > today_min) {
                    return Some((probe * 86_400_000 + *time_min as i64 * 60_000) - off * 60_000);
                }
            }
            return None; // unreachable for valid specs
        }
    };
    Some(fire_day * 86_400_000 + fire_min * 60_000 - off * 60_000)
}

// ---------------------------------------------------------------------------
// digest ("what changed since the last capture")
// ---------------------------------------------------------------------------

/// `scheduler:digest` — diff the two newest snapshots for one target.
/// Significance floor mirrors the snapshots feature (10 MB, docs/12 § 10).
pub fn digest(db: &Db, target: &str, limit: u8) -> Result<ScheduleDigest, EngineError> {
    let limit = limit.clamp(1, 100) as usize;
    let snaps = db.list_snapshots(target);
    if snaps.len() < 2 {
        let only = snaps.first();
        return Ok(ScheduleDigest {
            target: target.to_string(),
            before_ms: 0,
            after_ms: only.map(|s| s.created_at).unwrap_or(0),
            bytes_delta: 0,
            files_delta: 0,
            top: Vec::new(),
        });
    }
    let after = &snaps[0];
    let before = &snaps[1];
    let (deltas, net) = db.diff_snapshots(before.id, after.id, 10 * 1024 * 1024)?;
    let files_delta = deltas.iter().map(|d| d.delta_files).sum::<i64>();
    let mut top = deltas;
    // Biggest absolute byte delta first (significance ranking).
    top.sort_by_key(|d| std::cmp::Reverse(d.delta_bytes.abs()));
    top.truncate(limit);
    Ok(ScheduleDigest {
        target: target.to_string(),
        before_ms: before.created_at,
        after_ms: after.created_at,
        bytes_delta: net,
        files_delta,
        top,
    })
}

/// Open (or reuse) an engine DB at `path` — convenience for the background
/// runner entry (main passes the user-data db path).
pub fn open_db(path: &Path) -> Result<Db, EngineError> {
    Db::open(path)
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(id: &str, label: &str, target: &str, trigger: ScheduleTrigger) -> ScheduleSpec {
        ScheduleSpec {
            id: id.to_string(),
            label: label.to_string(),
            target: target.to_string(),
            trigger,
            enabled: true,
            last_run_ms: 0,
        }
    }

    #[test]
    fn validation_rejects_bad_specs() {
        let daily = ScheduleTrigger::Daily { time_min: 600 };
        assert!(normalize_spec(&spec("", "Weekly C:", "C:\\", daily.clone())).is_ok());
        assert!(normalize_spec(&spec("", "  ", "C:\\", daily.clone())).is_err());
        let too_long = "x".repeat(121);
        assert!(normalize_spec(&spec("", &too_long, "C:\\", daily.clone())).is_err());
        assert!(normalize_spec(&spec("", "l", "", daily.clone())).is_err());
        assert!(normalize_spec(&spec("", "l", "relative\\path", daily.clone())).is_err());
        assert!(
            normalize_spec(&spec(
                "",
                "l",
                "C:\\",
                ScheduleTrigger::Daily { time_min: 1440 }
            ))
            .is_err()
        );
        assert!(
            normalize_spec(&spec(
                "",
                "l",
                "C:\\",
                ScheduleTrigger::Weekly {
                    days: vec![],
                    time_min: 0
                }
            ))
            .is_err()
        );
        assert!(
            normalize_spec(&spec(
                "",
                "l",
                "C:\\",
                ScheduleTrigger::Weekly {
                    days: vec![0, 8],
                    time_min: 0
                }
            ))
            .is_err()
        );
    }

    #[test]
    fn weekly_days_are_normalized() {
        let n = normalize_spec(&spec(
            "",
            "l",
            "C:\\",
            ScheduleTrigger::Weekly {
                days: vec![5, 1, 5, 3],
                time_min: 30,
            },
        ))
        .unwrap_or_else(|e| panic!("normalize: {e:?}"));
        match n.trigger {
            ScheduleTrigger::Weekly { days, time_min } => {
                assert_eq!(days, vec![1, 3, 5], "deduped + sorted ISO days");
                assert_eq!(time_min, 30);
            }
            other => panic!("wrong trigger {other:?}"),
        }
    }

    #[test]
    fn store_round_trip_create_update_delete() {
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let db = Db::open(&dir.path().join("t.db")).unwrap_or_else(|e| panic!("{e}"));
        assert!(load_specs(&db).is_empty(), "fresh db has no specs");
        assert_eq!(
            backend_name(),
            if cfg!(windows) {
                "task-scheduler"
            } else {
                "dev-file"
            }
        );

        // create
        let s1 = upsert(
            &db,
            &spec(
                "",
                "Nightly C:",
                "C:\\",
                ScheduleTrigger::Daily { time_min: 540 },
            ),
            "/nonexistent/runner.exe",
        )
        .unwrap_or_else(|e| panic!("create: {e:?}"));
        assert!(!s1.id.is_empty(), "engine assigns the id");
        assert_eq!(load_specs(&db).len(), 1);

        // update by id (same id replaced, not appended)
        let s1b = upsert(
            &db,
            &spec(
                &s1.id,
                "Nightly C: v2",
                "C:\\",
                ScheduleTrigger::Daily { time_min: 600 },
            ),
            "/nonexistent/runner.exe",
        )
        .unwrap_or_else(|e| panic!("update: {e:?}"));
        assert_eq!(s1b.id, s1.id);
        let all = load_specs(&db);
        assert_eq!(all.len(), 1, "update replaced in place");
        assert_eq!(all[0].label, "Nightly C: v2");

        // delete + delete-missing
        delete(&db, &s1.id).unwrap_or_else(|e| panic!("delete: {e:?}"));
        assert!(load_specs(&db).is_empty());
        assert!(delete(&db, &s1.id).is_err(), "deleting a missing id fails");
    }

    #[test]
    fn schtasks_args_are_exact() {
        let s = spec(
            "abc",
            "l",
            "C:\\Users\\me",
            ScheduleTrigger::Daily { time_min: 540 },
        );
        assert_eq!(
            schtasks_create_args(&s, "C:\\Program Files\\Prism\\Prism.exe"),
            vec![
                "/Create",
                "/F",
                "/TN",
                "PRISM\\abc",
                "/TR",
                "\"C:\\Program Files\\Prism\\Prism.exe\" --background-scan \"C:\\Users\\me\"",
                "/SC",
                "DAILY",
                "/ST",
                "09:00",
            ]
        );
        let w = spec(
            "w1",
            "l",
            "D:\\data",
            ScheduleTrigger::Weekly {
                days: vec![1, 5],
                time_min: 1439,
            },
        );
        assert_eq!(
            schtasks_create_args(&w, "r.exe"),
            vec![
                "/Create",
                "/F",
                "/TN",
                "PRISM\\w1",
                "/TR",
                "\"r.exe\" --background-scan \"D:\\data\"",
                "/SC",
                "WEEKLY",
                "/D",
                "MON,FRI",
                "/ST",
                "23:59",
            ]
        );
        let l = spec("l1", "l", "C:\\", ScheduleTrigger::AtLogon {});
        assert_eq!(
            schtasks_create_args(&l, "r.exe"),
            vec![
                "/Create",
                "/F",
                "/TN",
                "PRISM\\l1",
                "/TR",
                "\"r.exe\" --background-scan \"C:\\\"",
                "/SC",
                "ONLOGON",
            ]
        );
    }

    #[test]
    fn action_is_always_a_scan() {
        // PRISM-HG-080 scope guard, pinned: the constructed action can ONLY
        // be a background standard scan — there is no other builder.
        let a = task_action("r.exe", "C:\\x");
        assert!(a.contains("--background-scan"));
        assert!(!a.contains("cleanup"), "no scheduled cleanup exists");
        assert!(!a.contains("delete"));
        assert!(!a.contains("execute"));
    }

    #[test]
    fn ids_are_v4_shaped_and_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        assert_eq!(a.as_bytes()[14], b'4', "uuid v4 version nibble");
        assert_eq!(a.matches('-').count(), 4);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-'));
    }

    #[test]
    fn civil_round_trip_epoch() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        for z in [0i64, 1, -1, 19_000, 20_000, -500] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z, "round trip {z}");
        }
        // 2026-09-20 is a Sunday.
        let d = days_from_civil(2026, 9, 20);
        assert_eq!(weekday(d), 0);
        assert_eq!(iso_weekday(d), 7);
    }

    #[test]
    fn next_run_daily_math() {
        // UTC (off-Windows dev backend): 2026-09-20 12:00:00 UTC.
        let now = days_from_civil(2026, 9, 20) * 86_400_000 + 12 * 3_600_000;
        let t = ScheduleTrigger::Daily { time_min: 540 }; // 09:00
        let nxt = next_run_ms(&t, now).unwrap_or_else(|| panic!("daily next"));
        // 09:00 already passed today → tomorrow 09:00 UTC.
        assert_eq!(
            nxt,
            days_from_civil(2026, 9, 21) * 86_400_000 + 540 * 60_000
        );
        // Before the slot today → today.
        let early = days_from_civil(2026, 9, 20) * 86_400_000 + 8 * 3_600_000;
        assert_eq!(
            next_run_ms(&t, early).unwrap_or_else(|| panic!("today slot")),
            days_from_civil(2026, 9, 20) * 86_400_000 + 540 * 60_000
        );
        // Exactly at the slot → tomorrow (a slot at now doesn't fire now).
        let at = days_from_civil(2026, 9, 20) * 86_400_000 + 540 * 60_000;
        assert_eq!(
            next_run_ms(&t, at).unwrap_or_else(|| panic!("tomorrow slot")),
            days_from_civil(2026, 9, 21) * 86_400_000 + 540 * 60_000
        );
    }

    #[test]
    fn next_run_weekly_and_logon() {
        // Sunday 2026-09-20, 12:00 UTC. Weekly MON(1)+FRI(5) at 09:00.
        let now = days_from_civil(2026, 9, 20) * 86_400_000 + 12 * 3_600_000;
        let t = ScheduleTrigger::Weekly {
            days: vec![1, 5],
            time_min: 540,
        };
        let nxt = next_run_ms(&t, now).unwrap_or_else(|| panic!("daily next"));
        // Next Monday = 2026-09-21.
        assert_eq!(
            nxt,
            days_from_civil(2026, 9, 21) * 86_400_000 + 540 * 60_000
        );
        // Friday 2026-09-25 morning, still before Friday 09:00? Friday 07:00 → today.
        let fri_morning = days_from_civil(2026, 9, 25) * 86_400_000 + 7 * 3_600_000;
        assert_eq!(
            next_run_ms(&t, fri_morning).unwrap_or_else(|| panic!("fri slot")),
            days_from_civil(2026, 9, 25) * 86_400_000 + 540 * 60_000
        );
        // AtLogon never predicts.
        assert_eq!(next_run_ms(&ScheduleTrigger::AtLogon {}, now), None);
    }

    #[test]
    fn digest_uses_two_newest_snapshots() {
        use crate::arena::{Arena, NodeInput, kind};
        let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        let db = Db::open(&dir.path().join("t.db")).unwrap_or_else(|e| panic!("{e}"));

        // Zero/one snapshot → honest empty digest.
        let d0 = digest(&db, "C:\\", 10).unwrap_or_else(|e| panic!("digest0: {e:?}"));
        assert_eq!(d0.before_ms, 0);
        assert!(d0.top.is_empty());

        let mk = |proj_bytes: u64| {
            let mut a = Arena::with_capacity(8);
            let root = a.push(NodeInput {
                parent: 0,
                name_utf16: &"C:".encode_utf16().collect::<Vec<_>>(),
                logical: 0,
                allocated: 0,
                files: 3,
                folders: 2,
                mtime: 0,
                kind: kind::ROOT,
                category: 0,
                ext_id: 0,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            let d1 = a.push(NodeInput {
                parent: root,
                name_utf16: &"proj".encode_utf16().collect::<Vec<_>>(),
                logical: proj_bytes,
                allocated: proj_bytes,
                files: 3,
                folders: 0,
                mtime: 0,
                kind: kind::DIR,
                category: 0,
                ext_id: 0,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            a.attach(root, d1);
            a.finalize_children();
            a
        };
        db.save_snapshot(&mk(1_000), "C:\\", 4)
            .unwrap_or_else(|e| panic!("snap1: {e}"));
        db.save_snapshot(&mk(60_000_000), "C:\\", 4)
            .unwrap_or_else(|e| panic!("snap2: {e}"));
        let d = digest(&db, "C:\\", 10).unwrap_or_else(|e| panic!("digest: {e:?}"));
        assert!(d.before_ms > 0 && d.after_ms >= d.before_ms);
        assert!(d.bytes_delta >= 59_999_000, "net {d:?}");
        assert!(!d.top.is_empty(), "grew row surfaces");
    }
}
