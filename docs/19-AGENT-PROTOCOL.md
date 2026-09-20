# 19 — Agent Protocol (Rules of Engagement for Build Agents)

> Owner doc: [00-INDEX.md](00-INDEX.md) · Upstream: [18-PHASE-PLAN](18-PHASE-PLAN.md) · Downstream: all.
>
> **This document governs every agent session.** It is written as rules, not suggestions. If a rule here conflicts with intuition, memory of "how it's usually done", or a stale doc — this doc and the current phase gate win; then file a doc fix.

---

## Session Bootstrap (mandatory, every session)

1. Read this document fully.
2. Read [18-PHASE-PLAN](18-PHASE-PLAN.md) — current phase entry criteria + your task's gate context.
3. Read the tail of `docs/worklog.md` (last 3 sessions minimum) — know what the previous agents did, deferred, and broke.
4. Read the spec docs your task touches (map in [00 § 2](00-INDEX.md#2-document-map--reading-order)).
5. Confirm: branch from `main`, task ID in branch name (`task/P3-004-viz-treemap`), one task = one branch = one PR.
6. **If any spec is ambiguous or contradictory for your task: STOP.** Open `docs/amendments/question-<slug>.md` describing the conflict, then take a different task. Never resolve ambiguity by guessing — a wrong guess costs a gate review cycle; a question costs a minute.

## A1 — Latest dependencies, pinned

- All new dependencies **MUST** be the latest stable release at the time of introduction, and the pinned version list [20-DEPENDENCY-MANIFEST](20-DEPENDENCY-MANIFEST.md) is updated in the same PR with a rationale line.
- Major-version upgrades are never bundled silently into feature PRs — separate upgrade PR with CI + perf diff.
- No deprecated APIs: if the docs mark an API deprecated, you don't use it. No beta/RC dependencies in prod paths without an owner amendment.
- Zero-installing: lockfiles frozen in CI; `pnpm approve-builds` allowlist for anything with native postinstall.

## A2 — No fallbacks (the rule the owner cares about most)

**Definition.** A "fallback" is any code path that silently substitutes a degraded/incomplete/alternate behavior when the primary path fails or is unavailable. They are **forbidden**:

- ❌ `try { primary } catch { doSomethingCheaper() }` without surfacing the failure to the user AND the logs.
- ❌ Feature detection that quietly disables a feature ("if WebGL missing, render nothing").
- ❌ Partial implementations shipped behind a default-off flag "for later".
- ❌ Catching an error to return `null`/`[]`/default value that the caller then treats as truth.
- ❌ Replacing a failed scan strategy with another strategy without the user explicitly choosing it.
- ❌ "Temporarily" hardcoding a value with a `// TODO`.

**The required pattern instead:** **fail loud, degrade visibly, or block honestly.**
1. Recoverable? → retry with backoff *only* for transient network/IO classes, then surface the typed error.
2. Degrades UX? → the UI states exactly what's unavailable and why (e.g., "No preview available for this type", "Turbo scan needs admin approval — approve or use Standard").
3. Not implemented? → don't ship the surface at all (no dead buttons, no stub tabs).

Every error path must end in: a typed error (Rust `EngineError` / TS discriminated union, [05 § 2.1](05-IPC-PROTOCOL.md#21-command-t1-request)), a log line with context, and (if user-visible) an actionable message. `catch {}` swallowing is a review-blocking offense.

## A3 — Production-grade quality baseline

- **Rust:** `Result` everywhere; no `unwrap`/`expect`/`panic!`/`todo!`/`unimplemented!` outside `#[cfg(test)]` (CI denies the lints). Unsafe blocks need `// SAFETY:` + invariant ([06 § 11](06-RUST-CORE.md#11-unsafe-policy--ffi-checklist)). Errors: `thiserror` enums; `anyhow` only in dev tooling. No `unsafe` in hot loops without a benchmark justifying it. Public APIs documented with examples. Bounds/overflow: prefer checked ops on user-derived numbers.
- **TypeScript:** `strict` + `noUncheckedIndexedAccess` + `exactOptionalPropertyTypes`; `any` forbidden (use `unknown` + narrow); `as` casts forbidden except generated code and DOM augmentation; exhaustive `switch` on discriminated unions (lint rule on). No `@ts-ignore`; `@ts-expect-error` requires an issue link comment.
- **React:** function components; hooks rules; no derived state in `useEffect` when it can be computed in render; stores per [10 § 15](10-SCREENS-AND-FLOWS.md#15-stores-state-architecture); virtualization rule ([09 § 3](09-UI-COMPONENTS.md#3-component-engineering-rules)) — long lists virtualize, always.
- **All code:** dead code deleted (not commented); commented-out code blocks rejected in review; magic numbers must be named constants or tokens; every non-obvious branch has a `why` comment (not a `what` comment).

## A4 — Senior-Rust engineering patterns (expected idioms)

- Arena/columnar data over node-pointer graphs ([07](07-DATA-MODEL.md)); indices over `Rc<RefCell<>>`; no `Box<dyn Error>` in library paths.
- Concurrency: bounded channels (`crossbeam` with explicit capacity); workers check cancellation between batches; single-writer arenas; `Arc` only where sharing is real; `Send`/`Sync` boundaries documented for new types.
- Zero-copy discipline in the scan path: `Vec<u16>` name buffers reused; no `String` allocations per file ([06 § 2.1](06-RUST-CORE.md#21-enumeration-primitive)); allocations amortized via `Vec::with_capacity` from pre-passes.
- Determinism: iteration orders that reach output are defined (size-desc, then name-ordinal); `HashMap` iteration never leaks into user-visible ordering (use `BTreeMap` or sorted vectors where order matters).
- FFI: `#[napi]` signatures minimal & typed; big payloads as `Buffer` ([05 § 5](05-IPC-PROTOCOL.md#5-binary-payloads-vizframe)); never call blocking JS from engine threads; TSFN only from the drain thread.
- Testing co-located; benches for anything on a budget row ([16](16-PERFORMANCE-BUDGETS.md)).

## A5 — Never panic across FFI; contain every failure

- Every `#[napi]` entry: `catch_unwind` wrapper (provided by `ipc/mod.rs` helper — use it, don't hand-roll); converts panics to `EngineError::Internal` + `tracing::error!`.
- Renderer crash: engine leases survive; renderer reattaches (`scan:reattach`, [04 § 6](04-SYSTEM-ARCHITECTURE.md#6-failure-domains--containment)).
- A failed sub-operation must never poison the session (scan continues past access-denied dirs — WDS-SCN-05 semantics) *and* never hide (errors drawer).

## A6 — Scope discipline

- Implement exactly the task's scope. Adjacent bugs you notice → file them (worklog + issue), don't fix them in the same PR unless trivial and explicitly listed in the PR description.
- Refactors need their own task ID. "Drive-by refactors" are reverted on review.
- Docs-first for contracts: if your task changes any message, token, theme key, or persistence schema, the spec doc updates **in the same PR** (docs are code — CI link-checks them).

## A7 — UI smoothness rules (the "no laggy feel" contract)

- **60 fps is the floor, 120 where the display allows** — long tasks > 8 ms on the main/renderer thread are bugs; measure, don't guess (`--perf-hud`, [16 § 7](16-PERFORMANCE-BUDGETS.md#7-measurement--enforcement)).
- All engine calls off the UI thread by construction ([04 § 3](04-SYSTEM-ARCHITECTURE.md#3-process--thread-topology)); the renderer never blocks on IPC for paint (frames cached, interactions local: hover hit-testing is client-side, [11 § 6](11-VISUALIZATION-ENGINE.md#6-interaction-contracts)).
- Animations: transform/opacity only; no layout-thrashing animations; stagger caps per [08 § 8](08-UI-DESIGN-SYSTEM.md#8-motion); `prefers-reduced-motion` honored.
- Search/filter typing never janks: debounce per contract ([10 § Search](10-SCREENS-AND-FLOWS.md#search--filter)), results applied in rAF transactions, spinner only ≥ 250 ms.
- Lists: virtualize; skeletons for async panes ≤ 300 ms; never blank-flash.
- Startup: Welcome interactive ≤ 1.5 s on R2 ([16 § 4](16-PERFORMANCE-BUDGETS.md#4-ui-budgets-renderer-r2-fix-l-active-scan)) — no network calls on the critical path (license check is async, non-blocking).

## A8 — Verification before hand-off (Definition of Done)

A task is **done** when all of these are true and evidenced in the PR body:

1. ✅ Code complete per spec; requirement IDs covered cited in PR.
2. ✅ Tests: unit + (integration/e2e/visual/perf as applicable) added and green locally and in CI.
3. ✅ `pnpm typecheck && pnpm lint && pnpm vitest run` + `cargo fmt --check && cargo clippy -- -D warnings && cargo test` all green.
4. ✅ Performance: no hot-path change without bench numbers in the PR (before/after).
5. ✅ Screenshots (UI) or golden frames (viz) attached; visual regression updated **only** with an explanation of the intended change.
6. ✅ Docs/spec updated for any contract change; worklog session entry appended (`Done` bullets must reference evidence: test names, commands, screenshots).
7. ✅ No TODOs, no commented-out code, no `console.log`/`dbg!` left.
8. ✅ Self-review performed (read your own diff as a reviewer; list what you'd ask).
9. ✅ If the task completes a phase-checklist box → updated `docs/phases/phase-<n>.md` (task boxes only — gate boxes are gate-review territory).

## A9 — Forbidden actions (instant review rejection)

- Committing secrets, keys, tokens, or `.env` files.
- Disabling lint rules/tests/CI jobs to make a pass (`eslint-disable` without issue link; `#[allow(clippy::…)]` without justification comment).
- Using forbidden vocabulary in product surfaces ([00 § 4.3](00-INDEX.md#43-forbidden-vocabulary-in-shipped-artifacts)) — including internal codename leaks into UI copy.
- Copying code from the GPL reference project or from the DiskBuddy artifacts (clean-room rules, [03 § 0](03-PORTING-MATRIX-WINDIRSTAT.md#0-clean-room-protocol)).
- Shipping a feature behind a flag that is not finished (A2).
- Modifying perf budgets or this protocol doc without an owner-approved amendment.
- Force-pushing shared branches; merging own PR without review.

## A10 — Context continuity (protect progress across sessions)

- **Worklog is sacred:** every session appends; nothing is "too small" to log — future you is a stranger.
- **Save state incrementally:** commit to your branch at every coherent stopping point; never leave > 400 lines uncommitted.
- **Hand-off notes:** if you stop mid-task, the worklog `Deferred` section must say exactly where you stopped, what compiles, what doesn't, and the next concrete step.
- **Read before write:** before touching a file, `git log -3 -- <file>` to see recent intent; before touching a spec, check `docs/amendments/` for pending changes.

## Worklog

Format per [18 § Session Records](18-PHASE-PLAN.md#session-records). The worklog is the recovery plane for the whole project: if the repository and the worklog disagree about *intent*, the worklog + specs win the argument, and the repo gets fixed to match.
