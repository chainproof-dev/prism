# AMM-004 — Turbo scan scope v1

**Date:** Session 2 · **Supersedes:** nothing · **Docs touched:** 06 § 3, 12 § 8

## Decision
Turbo scan v1 reads the full MFT and builds the arena from all in-use
records. `ScanOptions.excludePatterns` are NOT applied engine-side on the
turbo path (the MFT is read wholesale; filtering a flat record stream by
glob requires path materialization for every record, which forfeits the
throughput contract). Exclusion filtering on turbo scans is applied at the
viz/query layer.

## Rationale
ADR-06 makes turbo a first-class strategy with its own contract. The
honest tradeoff (documented here) is: exclusions cost per-record path
assembly; skipping them keeps the ≥1M files/s budget. The UI surfaces this
in the preflight explainer ("Turbo reads the whole index").

## Review
Revisit if exclusion-before-arena becomes a correctness requirement (e.g.
compliance-driven scans) — the alternative is glob matching on parent FRN
prefix sets, estimated 15-20% throughput cost.
