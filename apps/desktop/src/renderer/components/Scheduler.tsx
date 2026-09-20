// Scheduler settings section (PRISM-HG-080 — docs/12 § 9). Premium feature:
// shows the locked state honestly when unlicensed (no fake schedules).
// Scope guard mirrored in copy: schedules only ever run background STANDARD
// scans — no scheduled cleanup exists.
import { useCallback, useEffect, useState } from 'react';
import type { ScheduleSpec, SchedulerPage } from '@prism/shared/generated';
import { useLicenseStore } from '../stores/ui';

// Client-side next-run display (mirrors engine scheduler::next_run_ms; the
// engine owns firing on Windows, this is a local-time preview only).
function nextRun(spec: ScheduleSpec): string {
  if (!spec.enabled) return 'paused';
  if (spec.trigger.kind === 'at-logon') return 'at next logon';
  const now = new Date();
  const hh = Math.floor(spec.trigger.timeMin / 60);
  const mm = spec.trigger.timeMin % 60;
  const fire = (d: Date): Date => {
    const c = new Date(d);
    c.setHours(hh, mm, 0, 0);
    return c;
  };
  let candidate = fire(now);
  if (spec.trigger.kind === 'weekly') {
    // ISO 1..7 → JS 0..6 (Sun..Sat)
    const jsDays = new Set<number>(spec.trigger.days.map((iso) => (iso === 7 ? 0 : iso)));
    let probe = new Date(candidate);
    for (let i = 0; i < 8; i += 1) {
      if (jsDays.has(probe.getDay()) && probe.getTime() > now.getTime()) {
        candidate = probe;
        break;
      }
      probe = new Date(fire(new Date(probe.getFullYear(), probe.getMonth(), probe.getDate() + 1)));
    }
  }
  if (candidate.getTime() <= now.getTime()) {
    candidate = new Date(candidate.getFullYear(), candidate.getMonth(), candidate.getDate() + 1, hh, mm);
  }
  return candidate.toLocaleString(undefined, { weekday: 'short', hour: '2-digit', minute: '2-digit' });
}

function triggerLabel(spec: ScheduleSpec): string {
  const hhmm = (m: number): string => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
  switch (spec.trigger.kind) {
    case 'daily':
      return `Daily · ${hhmm(spec.trigger.timeMin)}`;
    case 'weekly': {
      const names = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
      return `Weekly · ${spec.trigger.days.map((d) => names[d - 1]).join(', ')} · ${hhmm(spec.trigger.timeMin)}`;
    }
    case 'at-logon':
      return 'At logon';
  }
}

interface Draft {
  label: string;
  target: string;
  kind: 'daily' | 'weekly' | 'at-logon';
  time: string;
  days: number[];
}

const EMPTY_DRAFT: Draft = { label: '', target: '', kind: 'daily', time: '09:00', days: [1] };

export function SchedulerSection(): React.ReactElement {
  const lic = useLicenseStore();
  const licensed = lic.phase === 'licensed' || lic.phase === 'trial' || lic.phase === 'grace';
  const [page, setPage] = useState<SchedulerPage | null>(null);
  const [draft, setDraft] = useState<Draft>(EMPTY_DRAFT);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async (): Promise<void> => {
    setError(null);
    try {
      const res = await window.prism?.invoke('scheduler:list', {});
      if (res && typeof res === 'object' && 'schedules' in res) {
        setPage(res as SchedulerPage);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, []);

  useEffect(() => {
    if (licensed) void reload();
  }, [licensed, reload]);

  if (!licensed) {
    return (
      <div className="rounded-md border border-hairline bg-surface-app p-3 text-xs text-text-secondary">
        <div className="mb-1 font-medium text-text-primary">Scheduler</div>
        Scheduled background scans are a Pro feature. Upgrade to keep watching a
        volume while the app is closed — each run auto-captures a snapshot and
        toasts a &ldquo;what changed&rdquo; digest on your next launch.
      </div>
    );
  }

  const upsert = async (): Promise<void> => {
    setBusy(true);
    setError(null);
    try {
      const timeMin = Number(draft.time.slice(0, 2)) * 60 + Number(draft.time.slice(3, 5));
      const trigger =
        draft.kind === 'daily'
          ? { kind: 'daily', timeMin }
          : draft.kind === 'weekly'
            ? { kind: 'weekly', days: draft.days, timeMin }
            : { kind: 'at-logon' };
      const spec = {
        id: '',
        label: draft.label || 'Scheduled scan',
        target: draft.target,
        trigger,
        enabled: true,
        lastRunMs: 0n,
      };
      await window.prism?.invoke('scheduler:upsert', { spec, runnerExe: '' });
      setDraft(EMPTY_DRAFT);
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const toggle = async (spec: ScheduleSpec): Promise<void> => {
    setBusy(true);
    try {
      await window.prism?.invoke('scheduler:upsert', {
        spec: { ...spec, enabled: !spec.enabled },
        runnerExe: '',
      });
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string): Promise<void> => {
    setBusy(true);
    try {
      await window.prism?.invoke('scheduler:delete', { id });
      await reload();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  const targetValid = /^[A-Za-z]:[\\/]/.test(draft.target) || draft.target.startsWith('/') || draft.target.startsWith('\\\\');

  return (
    <div>
      <div className="mb-2 text-[11px] text-text-faint">
        Schedules run a background standard scan and auto-capture a snapshot
        {page?.backend === 'dev-file' && ' (dev backend: entries persist but do not fire off-Windows)'}.
        Scheduled cleanup does not exist by design.
      </div>
      {error && <div className="mb-2 rounded border border-danger/40 bg-danger/10 p-2 text-[11px] text-danger">{error}</div>}
      <div className="space-y-2">
        {(page?.schedules ?? []).map((spec) => (
          <div key={spec.id} className="rounded-md border border-hairline bg-surface-app p-2.5 text-xs">
            <div className="flex items-center justify-between gap-2">
              <div className="min-w-0">
                <div className="truncate font-medium text-text-primary">{spec.label}</div>
                <div className="truncate text-text-faint">{spec.target}</div>
                <div className="mt-0.5 text-text-secondary">
                  {triggerLabel(spec)} · next: {nextRun(spec)}
                </div>
              </div>
              <div className="flex shrink-0 items-center gap-1.5">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void toggle(spec)}
                  className="rounded border border-hairline px-2 py-0.5 text-[11px] text-text-secondary hover:text-accent disabled:opacity-50"
                >
                  {spec.enabled ? 'Pause' : 'Resume'}
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void remove(spec.id)}
                  className="rounded border border-hairline px-2 py-0.5 text-[11px] text-text-secondary hover:text-danger disabled:opacity-50"
                >
                  Remove
                </button>
              </div>
            </div>
          </div>
        ))}
        {page?.schedules.length === 0 && (
          <div className="rounded-md border border-dashed border-hairline p-3 text-center text-[11px] text-text-faint">
            No schedules yet — add one below.
          </div>
        )}
      </div>

      {/* add form */}
      <div className="mt-3 space-y-2 rounded-md border border-hairline bg-surface-app p-2.5">
        <div className="grid grid-cols-2 gap-2">
          <input
            value={draft.label}
            onChange={(e) => setDraft({ ...draft, label: e.target.value })}
            placeholder="Label (e.g. Nightly C:)"
            className="rounded border border-hairline bg-surface-panel px-2 py-1 text-xs"
          />
          <input
            value={draft.target}
            onChange={(e) => setDraft({ ...draft, target: e.target.value })}
            placeholder="Target (C:\ or D:\data)"
            className="rounded border border-hairline bg-surface-panel px-2 py-1 text-xs"
          />
        </div>
        <div className="flex items-center gap-2">
          <select
            value={draft.kind}
            onChange={(e) => setDraft({ ...draft, kind: e.target.value as Draft['kind'] })}
            className="rounded border border-hairline bg-surface-panel px-2 py-1 text-xs"
          >
            <option value="daily">Daily</option>
            <option value="weekly">Weekly</option>
            <option value="at-logon">At logon</option>
          </select>
          {draft.kind !== 'at-logon' && (
            <input
              type="time"
              value={draft.time}
              onChange={(e) => setDraft({ ...draft, time: e.target.value })}
              className="rounded border border-hairline bg-surface-panel px-2 py-1 text-xs"
            />
          )}
          {draft.kind === 'weekly' && (
            <div className="flex gap-1">
              {[1, 2, 3, 4, 5, 6, 7].map((d) => (
                <button
                  key={d}
                  type="button"
                  onClick={() => {
                    const has = draft.days.includes(d);
                    setDraft({
                      ...draft,
                      days: has ? draft.days.filter((x) => x !== d) : [...draft.days, d],
                    });
                  }}
                  className={`h-6 w-6 rounded text-[10px] ${draft.days.includes(d) ? 'bg-accent text-surface-app' : 'border border-hairline text-text-faint'}`}
                >
                  {['M', 'T', 'W', 'T', 'F', 'S', 'S'][d - 1]}
                </button>
              ))}
            </div>
          )}
        </div>
        <button
          type="button"
          disabled={busy || !targetValid || (draft.kind === 'weekly' && draft.days.length === 0)}
          onClick={() => void upsert()}
          className="w-full rounded bg-accent px-3 py-1.5 text-xs font-medium text-surface-app disabled:opacity-40"
        >
          {busy ? 'Saving…' : 'Add schedule'}
        </button>
      </div>
    </div>
  );
}
