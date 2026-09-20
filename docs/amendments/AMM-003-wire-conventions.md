# AMM-003 — Wire conventions for T1/T2 payloads

**Status:** Accepted · **Affects:** docs/05-IPC-PROTOCOL.md § 2/§ 5/§ 6 · **Phase:** 0–1

## Context

docs/05 sketches the wire with `bigint` sizes and a 24-byte TileRec. During
implementation three details needed concrete decisions:

## Decisions

1. **Transport**: commands cross T2 as `serde_json::Value` (napi serde-json
   feature), events as JSON-serialized batch strings over one TSFN (≤ 512
   events / ≤ 16 ms per the event pump contract). T1 (Electron IPC) passes
   the values through structured clone. One serialization boundary, one
   source of truth (the Rust structs + their serde attributes).
2. **BigInt end-to-end (as originally intended)**: napi's serde-json bridge
   converts `u64` → JS `BigInt`, so all byte counts arrive as real `bigint`
   in the renderer. The TS contracts generate `bigint` for `u64`/`i64` fields
   (NodeRow.logical, summary totals, volume sizes, …). Formatting utilities
   are bigint-aware.
3. **VizFrame corrections**: the spec sketch's "24 bytes each" TileRec
   arithmetic was wrong — 7 fields (i32 + 4×f32 + 2×u32) = **28 bytes**. The
   header carries `viewportW/H/dpr` (36-byte header) for validation. Color
   slots are **palette-indexed** (frame flag bit 0): the engine emits a
   category/branch/age index; the renderer resolves it through the theme
   `DataPalette` — palette edits re-tint cached frames without relayout
   (docs/11 § 5 semantics preserved).
4. **Mode-specific tile semantics** (documented in `viz/mod.rs`): treemap/
   icicle = rects; sunburst = (r0, a0, r1, a1) polar; pack/mindmap =
   center + radius. The renderer interprets by mode byte.
5. **Event name mapping**: engine event tags are kebab (`scan-progress`);
   T1 event names use the colon form (`scan:progress`) — the generated
   `EventsMap` maps both.

## Consequences

- The `emit-ts` codegen reads the same serde attributes that drive the
  actual wire — drift is a CI break, not a runtime surprise.
- Golden-frame hashes and VizFrame round-trip tests pin the binary layout.
