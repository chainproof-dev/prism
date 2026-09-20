// VizFrame binary decoder — mirrors crates/prism-core/src/viz/frame.rs
// (AMM-003 header layout: 36-byte header, 28-byte tiles, palette-indexed
// color slots resolved by the renderer's DataPalette).

export const VIZ_MAGIC = 0x3156_4650; // 'PVF1' LE
export const VIZ_VERSION = 1;
export const HEADER_LEN = 36;
export const TILE_LEN = 28;

/** Tile flag bits (mirror of Rust `tile_flags`). */
export const TileFlags = {
  COMPOSITE: 0x0001,
  DIR: 0x0002,
  FREE_SPACE: 0x0004,
  UNKNOWN: 0x0008,
} as const;

/** Frame flag: color slot carries a palette index. */
export const FRAME_FLAG_PALETTE_INDEXED = 0x01;

export interface DecodedTile {
  nodeId: number;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Palette index (when palette-indexed) or literal ARGB. */
  color: number;
  depth: number;
  flags: number;
}

export interface DecodedLabel {
  nodeId: number;
  x: number;
  y: number;
  text: string;
}

export interface DecodedFrame {
  scanId: number;
  rootId: number;
  mode: number;
  flags: number;
  w: number;
  h: number;
  dpr: number;
  tiles: DecodedTile[];
  labels: DecodedLabel[];
  paletteIndexed: boolean;
  /** Mode-specific tile semantics (see viz/mod.rs module docs). */
  polar: boolean;
}

class Cursor {
  private view: DataView;
  pos = 0;
  constructor(private buf: ArrayBuffer) {
    this.view = new DataView(buf);
  }
  u32(): number {
    const v = this.view.getUint32(this.pos, true);
    this.pos += 4;
    return v;
  }
  i32(): number {
    const v = this.view.getInt32(this.pos, true);
    this.pos += 4;
    return v;
  }
  u16(): number {
    const v = this.view.getUint16(this.pos, true);
    this.pos += 2;
    return v;
  }
  f32(): number {
    const v = this.view.getFloat32(this.pos, true);
    this.pos += 4;
    return v;
  }
  utf8(len: number): string {
    const bytes = new Uint8Array(this.buf, this.pos, len);
    this.pos += len;
    return new TextDecoder().decode(bytes);
  }
  get remaining(): number {
    return this.buf.byteLength - this.pos;
  }
}

/**
 * Decode + validate a VizFrame. Malformed frames throw — the caller surfaces
 * the "viz degraded" diagnostic (PRISM-IPC-030: hard error, never silent).
 */
export function decodeVizFrame(buffer: ArrayBuffer): DecodedFrame {
  if (buffer.byteLength < HEADER_LEN) {
    throw new Error(`viz-frame: too short (${buffer.byteLength} bytes)`);
  }
  const c = new Cursor(buffer);
  const magic = c.u32();
  if (magic !== VIZ_MAGIC) {
    throw new Error(`viz-frame: bad magic 0x${magic.toString(16)}`);
  }
  const version = c.u32();
  if (version !== VIZ_VERSION) {
    throw new Error(`viz-frame: unsupported version ${version}`);
  }
  const scanId = c.u32();
  const rootId = c.u32();
  const tileCount = c.u32();
  // header bytes 20..24: mode(u8) | flags(u8) | reserved(u16)
  const modeByte = new DataView(buffer, 20, 1).getUint8(0);
  const frameFlags = new DataView(buffer, 21, 1).getUint8(0);
  c.pos = 24;
  const w = c.u32();
  const h = c.u32();
  const dpr = c.f32();
  if (tileCount > 250_000) {
    throw new Error(`viz-frame: tileCount ${tileCount} exceeds cap (PRISM-IPC-052)`);
  }
  c.pos = HEADER_LEN;
  const tiles: DecodedTile[] = new Array(tileCount);
  for (let i = 0; i < tileCount; i++) {
    if (c.remaining < TILE_LEN) {
      throw new Error('viz-frame: truncated tile table');
    }
    const nodeId = c.i32();
    const x = c.f32();
    const y = c.f32();
    const tw = c.f32();
    const th = c.f32();
    const color = c.u32();
    const meta = c.u32();
    tiles[i] = { nodeId, x, y, w: tw, h: th, color, depth: meta >>> 16, flags: meta & 0xffff };
  }
  const labelCount = c.u32();
  const labels: DecodedLabel[] = new Array(labelCount);
  for (let i = 0; i < labelCount; i++) {
    if (c.remaining < 14) {
      throw new Error('viz-frame: truncated label');
    }
    const nodeId = c.i32();
    const x = c.f32();
    const y = c.f32();
    const len = c.u16();
    if (c.remaining < len) {
      throw new Error('viz-frame: truncated label text');
    }
    labels[i] = { nodeId, x, y, text: c.utf8(len) };
  }
  const polar = modeByte === 2 || modeByte === 4 || modeByte === 5; // sunburst/pack/mindmap
  return {
    scanId,
    rootId,
    mode: modeByte,
    flags: frameFlags,
    w,
    h,
    dpr,
    tiles,
    labels,
    paletteIndexed: (frameFlags & FRAME_FLAG_PALETTE_INDEXED) !== 0,
    polar,
  };
}

/** Palette-index slots (mirror of Rust `palette_index`). */
export const PaletteSlot = {
  CATEGORY: 0x1_0000,
  BRANCH: 0x2_0000,
  AGE: 0x3_0000,
} as const;

/** Extract the palette index kind + value from a color slot. */
export function splitPaletteSlot(color: number): { kind: 'category' | 'branch' | 'age'; index: number } {
  const kindBits = color & 0xffff_0000;
  const index = color & 0xffff;
  if (kindBits === PaletteSlot.CATEGORY) return { kind: 'category', index };
  if (kindBits === PaletteSlot.BRANCH) return { kind: 'branch', index };
  return { kind: 'age', index };
}
