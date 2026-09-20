// Test-only VizFrame builder — mirrors the Rust FrameBuilder byte-for-byte
// so the TS decoder is validated against the exact wire format.
const MAGIC = 0x3156_4650;

export function buildTestFrame(): ArrayBuffer {
  const buf = new Uint8Array(36 + 28 * 2 + 4 + 20);
  const v = new DataView(buf.buffer);
  let p = 0;
  const u32 = (x: number) => { v.setUint32(p, x, true); p += 4; };
  const i32 = (x: number) => { v.setInt32(p, x, true); p += 4; };
  const f32 = (x: number) => { v.setFloat32(p, x, true); p += 4; };
  const u16 = (x: number) => { v.setUint16(p, x, true); p += 2; };
  u32(MAGIC); u32(1); u32(7); u32(3); u32(2); // magic ver scan root tileCount
  buf[20] = 1;  // mode = treemap
  buf[21] = 0x01; // frame flags = PALETTE_INDEXED
  buf[22] = 0; buf[23] = 0; // reserved
  p = 24;
  u32(800); u32(600); f32(2.0);
  // tile 1
  i32(42); f32(0); f32(0); f32(400); f32(300); u32(0x1_0002); u32((1 << 16) | 2);
  // tile 2 (composite)
  i32(-1); f32(400); f32(0); f32(400); f32(300); u32(0x2_0005); u32((2 << 16) | 1);
  // labels
  u32(1);
  i32(42); f32(4); f32(16); u16(5);
  buf.set([0x55, 0x73, 0x65, 0x72, 0x73], p); // "Users"
  return buf.buffer;
}
