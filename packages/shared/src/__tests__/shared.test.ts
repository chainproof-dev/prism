import { describe, expect, it } from 'vitest';
import { formatBytes, formatDuration, formatPercent, formatRate } from '../client';
import { buildTestFrame } from './viz-frame-test-helper';
import { decodeVizFrame, TileFlags } from '../viz-frame';

describe('formatBytes (docs/08 § 11)', () => {
  it('uses SI decimals with adaptive digits', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(999)).toBe('999 B');
    expect(formatBytes(4_200_000)).toBe('4.2 MB');
    expect(formatBytes(118_000_000_000)).toBe('118 GB');
  });
  it('binary toggle', () => {
    expect(formatBytes(1024 ** 3, { binary: true })).toBe('1.0 GiB');
  });
  it('exact mode', () => {
    expect(formatBytes(1_234_567_890, { exact: true })).toBe('1,234,567,890 bytes');
  });
});

describe('percent + duration + rate', () => {
  it('percent', () => {
    expect(formatPercent(0.0004)).toBe('< 0.1%');
    expect(formatPercent(0.004)).toBe('0.4%');
    expect(formatPercent(0.12)).toBe('12%');
  });
  it('duration', () => {
    expect(formatDuration(9_800)).toBe('9s');
    expect(formatDuration(243_000)).toBe('4m 03s');
  });
  it('rate', () => {
    expect(formatRate(1_240_000)).toBe('1.24M files/s');
  });
});

describe('viz-frame decoder (round-trips the Rust encoder wire format)', () => {
  it('decodes a golden frame', () => {
    const d = decodeVizFrame(buildTestFrame());
    expect(d.scanId).toBe(7);
    expect(d.rootId).toBe(3);
    expect(d.w).toBe(800);
    expect(d.dpr).toBe(2);
    expect(d.tiles).toHaveLength(2);
    const tile1 = d.tiles[1]!;
    expect((tile1.flags & TileFlags.COMPOSITE) !== 0).toBe(true);
    expect(tile1.depth).toBe(2);
    expect(d.labels[0]?.text).toBe('Users');
    expect(d.paletteIndexed).toBe(true);
  });
  it('rejects corrupted frames loudly', () => {
    const frame = new Uint8Array(buildTestFrame());
    frame[0] = 0x58; // corrupt magic
    expect(() => decodeVizFrame(frame.buffer)).toThrow(/magic/);
  });
});
