# Rollback runbook (docs/15 § 7.5)

## When to run
- A released build shows a crash spike, engine-load failures, or a licensing
  break AFTER the feed started serving it.

## Procedure
1. **Freeze**: set the channel directory to read-only (stops new update
   checks from seeing a bad `latest.yml` mid-swap).
2. **Swap back**: restore the previous `latest.yml` object (feed keeps N-3
   versions). Blue-green object swap — no partial states.
   ```sh
   rclone copy feed/stable/v0.1.1/latest.yml r2:prism-feed/stable/latest.yml
   ```
3. **In-app banner**: push an engine-warning through the update metadata
   (`releaseNotes` carries the notice; the app renders it post-update-check).
4. **Verify**: install the previous version fresh on Win10 21H2 + Win11
   24H2 x64/arm64, run the health check (engine loads, hello passes, viz
   frame renders headless — the staged-apply gate, docs/15 § 6.5).
5. **Postmortem**: file `docs/amendments/incident-<id>.md` within 48h.

## Downgrade attacks
The updater rejects version < current (version policy, docs/15 § 6.4) —
rollback via the feed is a SERVER-side operation (trusted channel), never a
client-accepted downgrade. Log lines carry `update.reject-downgrade`.

## Never do
- Do not hand-edit `latest.yml` hashes — regenerating via electron-builder
  is the only correct path (the sha512 must match the signed installer).
- Do not serve an unsigned "hotfix" — SmartScreen and the updater's
  Authenticode pre-execution check will both reject it (by design).
