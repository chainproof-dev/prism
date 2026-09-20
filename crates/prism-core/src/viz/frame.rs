//! VizFrame binary format (docs/05 § 5, PRISM-IPC-030). Little-endian.
//!
//! Layout (AMM-003 corrects the spec sketch arithmetic: TileRec is 28 bytes
//! — i32 + 4×f32 + 2×u32; the header carries w/h/dpr for validation):
//!
//! ```text
//! HEADER (36 bytes)
//!   u32 magic 'PVF1' | u32 version | u32 scanId | u32 rootId
//!   u32 tileCount | u8 mode | u8 flags | u16 reserved
//!   u32 viewportW | u32 viewportH | f32 dpr
//! TILES (28 bytes each × tileCount)
//!   i32 nodeId | f32 x | f32 y | f32 w | f32 h | u32 colorARGB | u32 meta
//!   meta = (depth:u16 << 16) | flags:u16
//! LABELS
//!   u32 labelCount, then per label: i32 nodeId | f32 x | f32 y | u16 len | u8[len] utf8
//! ```

/// Frame magic.
pub const MAGIC: u32 = u32::from_le_bytes(*b"PVF1");
/// Current format version.
pub const VERSION: u32 = 1;
/// Header length in bytes.
pub const HEADER_LEN: usize = 36;
/// Tile record length in bytes.
pub const TILE_LEN: usize = 28;

/// Tile flag bits (meta low u16 = flags, high u16 = depth).
pub mod tile_flags {
    /// Composite (`<small items>` aggregated node).
    pub const COMPOSITE: u16 = 0x0001;
    /// Directory (vs file leaf).
    pub const DIR: u16 = 0x0002;
    /// Free-space pseudo tile.
    pub const FREE_SPACE: u16 = 0x0004;
    /// Unknown pseudo tile (hatched).
    pub const UNKNOWN: u16 = 0x0008;
}

/// One tile record pending serialization.
pub struct Tile {
    /// Node id (-1 for synthetic composites).
    pub node_id: i32,
    /// x in device px.
    pub x: f32,
    /// y in device px.
    pub y: f32,
    /// width.
    pub w: f32,
    /// height.
    pub h: f32,
    /// ARGB color.
    pub argb: u32,
    /// depth.
    pub depth: u16,
    /// flags.
    pub flags: u16,
}

/// One label record.
pub struct Label {
    /// Node id.
    pub node_id: i32,
    /// x.
    pub x: f32,
    /// y.
    pub y: f32,
    /// utf8 text.
    pub text: String,
}

/// Frame builder (engine side).
pub struct FrameBuilder {
    buf: Vec<u8>,
    label_body: Vec<u8>,
    tile_count: u32,
    label_count: u32,
}

impl FrameBuilder {
    /// New frame for (scan, root, mode, frame-flags, viewport, dpr).
    pub fn new(
        scan_id: u32,
        root_id: u32,
        mode: u8,
        frame_flags: u8,
        w: u32,
        h: u32,
        dpr: f32,
    ) -> Self {
        let mut buf = Vec::with_capacity(64 * 1024);
        put_u32(&mut buf, MAGIC);
        put_u32(&mut buf, VERSION);
        put_u32(&mut buf, scan_id);
        put_u32(&mut buf, root_id);
        put_u32(&mut buf, 0); // tileCount placeholder @16
        buf.push(mode);
        buf.push(frame_flags);
        put_u16(&mut buf, 0); // reserved
        put_u32(&mut buf, w);
        put_u32(&mut buf, h);
        put_f32(&mut buf, dpr);
        debug_assert_eq!(buf.len(), HEADER_LEN);
        Self {
            buf,
            label_body: Vec::new(),
            tile_count: 0,
            label_count: 0,
        }
    }

    /// Append a tile.
    pub fn tile(&mut self, t: Tile) {
        put_i32(&mut self.buf, t.node_id);
        put_f32(&mut self.buf, t.x);
        put_f32(&mut self.buf, t.y);
        put_f32(&mut self.buf, t.w);
        put_f32(&mut self.buf, t.h);
        put_u32(&mut self.buf, t.argb);
        put_u32(
            &mut self.buf,
            (u32::from(t.depth) << 16) | u32::from(t.flags),
        );
        self.tile_count += 1;
    }

    /// Append a label (serialized into the label region).
    pub fn label(&mut self, l: Label) {
        put_i32(&mut self.label_body, l.node_id);
        put_f32(&mut self.label_body, l.x);
        put_f32(&mut self.label_body, l.y);
        let bytes = l.text.as_bytes();
        put_u16(&mut self.label_body, bytes.len() as u16);
        self.label_body.extend_from_slice(bytes);
        self.label_count += 1;
    }

    /// Finish: patch tileCount, append label section.
    pub fn finish(mut self) -> Vec<u8> {
        self.buf[16..20].copy_from_slice(&self.tile_count.to_le_bytes());
        put_u32(&mut self.buf, self.label_count);
        self.buf.extend_from_slice(&self.label_body);
        self.buf
    }
}

fn put_u32(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_i32(buf: &mut Vec<u8>, v: i32) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_u16(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_le_bytes());
}
fn put_f32(buf: &mut Vec<u8>, v: f32) {
    buf.extend_from_slice(&v.to_le_bytes());
}

/// Decoded frame (engine mirror; the TS decoder lives in
/// `packages/shared/src/viz-frame.ts`).
pub struct DecodedFrame {
    /// Scan id.
    pub scan_id: u32,
    /// Root node.
    pub root_id: u32,
    /// Mode byte (matches `VizMode` wire order).
    pub mode: u8,
    /// Frame flags.
    pub flags: u8,
    /// Viewport width.
    pub w: u32,
    /// Viewport height.
    pub h: u32,
    /// DPR.
    pub dpr: f32,
    /// Tiles.
    pub tiles: Vec<DecodedTile>,
    /// Labels.
    pub labels: Vec<DecodedLabel>,
}

/// Decoded tile.
#[derive(Debug, Clone, Copy)]
pub struct DecodedTile {
    /// Node id.
    pub node_id: i32,
    /// x.
    pub x: f32,
    /// y.
    pub y: f32,
    /// w.
    pub w: f32,
    /// h.
    pub h: f32,
    /// ARGB.
    pub argb: u32,
    /// depth.
    pub depth: u16,
    /// flags.
    pub flags: u16,
}

/// Decoded label.
#[derive(Debug, Clone)]
pub struct DecodedLabel {
    /// Node id.
    pub node_id: i32,
    /// x.
    pub x: f32,
    /// y.
    pub y: f32,
    /// text.
    pub text: String,
}

fn take_u32(buf: &[u8], pos: usize) -> Result<u32, String> {
    buf.get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| "truncated u32".to_string())
}
fn take_i32(buf: &[u8], pos: usize) -> Result<i32, String> {
    buf.get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .map(i32::from_le_bytes)
        .ok_or_else(|| "truncated i32".to_string())
}
fn take_u16(buf: &[u8], pos: usize) -> Result<u16, String> {
    buf.get(pos..pos + 2)
        .and_then(|s| s.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or_else(|| "truncated u16".to_string())
}
fn take_f32(buf: &[u8], pos: usize) -> Result<f32, String> {
    buf.get(pos..pos + 4)
        .and_then(|s| s.try_into().ok())
        .map(f32::from_le_bytes)
        .ok_or_else(|| "truncated f32".to_string())
}

/// Decode + validate a frame. Malformed frames are hard errors (the renderer
/// shows a "viz degraded" diagnostic — never a silent drop, PRISM-IPC-030).
pub fn decode(buf: &[u8]) -> Result<DecodedFrame, String> {
    if buf.len() < HEADER_LEN {
        return Err(format!("frame too short: {} bytes", buf.len()));
    }
    let magic = take_u32(buf, 0)?;
    if magic != MAGIC {
        return Err(format!("bad magic: {magic:#010x}"));
    }
    let version = take_u32(buf, 4)?;
    if version != VERSION {
        return Err(format!("unsupported frame version {version}"));
    }
    let scan_id = take_u32(buf, 8)?;
    let root_id = take_u32(buf, 12)?;
    let tile_count = take_u32(buf, 16)? as usize;
    let mode = buf[20];
    let flags = buf[21];
    let w = take_u32(buf, 24)?;
    let h = take_u32(buf, 28)?;
    let dpr = take_f32(buf, 32)?;
    if tile_count > 250_000 {
        return Err(format!(
            "tileCount {tile_count} exceeds the PRISM-IPC-052 cap"
        ));
    }
    let mut pos = HEADER_LEN;
    let mut tiles = Vec::with_capacity(tile_count);
    for _ in 0..tile_count {
        if pos + TILE_LEN > buf.len() {
            return Err("truncated tile table".into());
        }
        let node_id = take_i32(buf, pos)?;
        let x = take_f32(buf, pos + 4)?;
        let y = take_f32(buf, pos + 8)?;
        let tw = take_f32(buf, pos + 12)?;
        let th = take_f32(buf, pos + 16)?;
        let argb = take_u32(buf, pos + 20)?;
        let meta = take_u32(buf, pos + 24)?;
        tiles.push(DecodedTile {
            node_id,
            x,
            y,
            w: tw,
            h: th,
            argb,
            depth: (meta >> 16) as u16,
            flags: (meta & 0xFFFF) as u16,
        });
        pos += TILE_LEN;
    }
    let label_count = take_u32(buf, pos)? as usize;
    pos += 4;
    let mut labels = Vec::with_capacity(label_count.min(10_000));
    for _ in 0..label_count {
        if pos + 14 > buf.len() {
            return Err("truncated label".into());
        }
        let node_id = take_i32(buf, pos)?;
        let x = take_f32(buf, pos + 4)?;
        let y = take_f32(buf, pos + 8)?;
        let len = take_u16(buf, pos + 12)? as usize;
        pos += 14;
        if pos + len > buf.len() {
            return Err("truncated label text".into());
        }
        let text = String::from_utf8(buf[pos..pos + len].to_vec()).map_err(|e| e.to_string())?;
        pos += len;
        labels.push(DecodedLabel {
            node_id,
            x,
            y,
            text,
        });
    }
    Ok(DecodedFrame {
        scan_id,
        root_id,
        mode,
        flags,
        w,
        h,
        dpr,
        tiles,
        labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut b = FrameBuilder::new(7, 3, 1, 0, 800, 600, 2.0);
        b.tile(Tile {
            node_id: 42,
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
            argb: 0xFF_53_D7_F0,
            depth: 1,
            flags: tile_flags::DIR,
        });
        b.tile(Tile {
            node_id: -1,
            x: 400.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
            argb: 0xFF_88_88_88,
            depth: 2,
            flags: tile_flags::COMPOSITE,
        });
        b.label(Label {
            node_id: 42,
            x: 4.0,
            y: 16.0,
            text: "Users".into(),
        });
        let frame = b.finish();
        let d = decode(&frame).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(d.scan_id, 7);
        assert_eq!(d.root_id, 3);
        assert_eq!(d.w, 800);
        assert_eq!(d.dpr, 2.0);
        assert_eq!(d.tiles.len(), 2);
        assert_eq!(d.tiles[1].flags, tile_flags::COMPOSITE);
        assert_eq!(d.tiles[1].depth, 2);
        assert_eq!(d.labels.len(), 1);
        assert_eq!(d.labels[0].text, "Users");
    }

    #[test]
    fn malformed_rejected() {
        assert!(decode(&[0u8; 8]).is_err());
        let mut b = FrameBuilder::new(1, 1, 1, 0, 10, 10, 1.0);
        b.tile(Tile {
            node_id: 1,
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            argb: 0,
            depth: 0,
            flags: 0,
        });
        let mut frame = b.finish();
        frame[0] = b'X'; // corrupt magic
        assert!(decode(&frame).is_err());
        let frame2 = FrameBuilder::new(1, 1, 1, 0, 10, 10, 1.0).finish();
        assert!(decode(&frame2).is_ok());
    }
}
