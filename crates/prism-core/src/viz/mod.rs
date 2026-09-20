//! # Visualization layout engine (docs/11)
//!
//! All layouts are computed here, in Rust, deterministically, off the UI
//! thread. The squarified treemap is implemented from the published algorithm
//! (Bruls, Huizing & van Wijk, "Squarified Treemaps") — clean-room per
//! docs/03 § 0. Cushion shading is a *paint-time* technique (renderer), driven
//! by the depth metadata we emit per tile.
//!
//! **Palette-indexed colors:** frame flag bit 0 (`FRAME_FLAG_PALETTE_INDEXED`)
//! means the `argb` slot carries a palette index the renderer resolves via
//! the theme `DataPalette` — color edits re-tint without relayout (§ 5).
//!
//! **Per-mode tile semantics** (renderer interprets by mode byte):
//! - treemap/icicle: `x, y, w, h` = rect (device px)
//! - sunburst: `x, y` = (r_inner, r_outer), `w, h` = (angle_start, angle_end) rad
//! - pack: `x, y` = center, `w` = radius, `h` unused
//! - mindmap: `x, y` = center, `w` = radius, `h` unused (edges derived from depth)
//! - age timeline: rect columns

pub mod frame;

use prism_types::ids::NodeId;
use prism_types::viz::{ColorMode, VizMode, VizOptions};

use crate::arena::{Arena, kind};
use frame::{FrameBuilder, Label, Tile, tile_flags};

/// Frame flag: color slot carries a palette index (vs literal ARGB).
pub const FRAME_FLAG_PALETTE_INDEXED: u8 = 0x01;

/// Mode byte (wire order matches `VizMode`).
pub fn mode_byte(mode: VizMode) -> u8 {
    match mode {
        VizMode::Treemap => 1,
        VizMode::Sunburst => 2,
        VizMode::Icicle => 3,
        VizMode::Pack => 4,
        VizMode::Mindmap => 5,
        VizMode::Folders => 6,
        VizMode::Table => 7,
        VizMode::Bars => 8,
        VizMode::AgeTimeline => 9,
    }
}

/// Age timeline column count (docs/11 § 4.7 default 64).
pub const AGE_COLUMNS: usize = 64;

/// Compute a layout frame for (arena, root, mode, viewport). DOM modes
/// (Folders/Table/Bars) are served by typed queries, not frames.
pub fn layout(
    arena: &Arena,
    scan_id: u32,
    root: NodeId,
    mode: VizMode,
    viewport: (f32, f32, f32),
    options: &VizOptions,
) -> Result<Vec<u8>, String> {
    if !arena.is_finalized() {
        return Err("arena not finalized (query during walk is served by deltas)".to_string());
    }
    let (w, h, dpr) = viewport;
    if w <= 0.0 || h <= 0.0 {
        return Err(format!("invalid viewport {w}×{h}"));
    }
    if root as usize >= arena.len() {
        return Err(format!("root {root} out of range (len {})", arena.len()));
    }
    let mut ctx = Ctx {
        arena,
        options,
        mode,
        tiles: Vec::with_capacity(4096),
        labels: Vec::new(),
        budget: tile_budget(mode, options),
    };
    match mode {
        VizMode::Treemap => ctx.squarify(root, 0.0, 0.0, w, h, 0),
        VizMode::Icicle => ctx.icicle(root, 0.0, 0.0, w, 0),
        VizMode::Sunburst => {
            let total = arena.allocated(root as usize).max(1) as f32;
            let r_max = w.min(h) / 2.0;
            ctx.sunburst(
                root,
                r_max * 0.08,
                r_max,
                0.0,
                std::f32::consts::TAU,
                total,
                0,
            );
        }
        VizMode::Pack => ctx.pack(root, w / 2.0, h / 2.0, w.min(h) / 2.0, 0),
        VizMode::Mindmap => ctx.mindmap(root, w / 2.0, h / 2.0, 44.0, 0),
        VizMode::AgeTimeline => ctx.age_timeline(root, w, h),
        VizMode::Folders | VizMode::Table | VizMode::Bars => {
            return Err("DOM modes are served by typed queries, not binary frames".to_string());
        }
    }
    let mut builder = FrameBuilder::new(
        scan_id,
        root,
        mode_byte(mode),
        FRAME_FLAG_PALETTE_INDEXED,
        w as u32,
        h as u32,
        dpr,
    );
    for t in ctx.tiles {
        builder.tile(t);
    }
    for l in ctx.labels {
        builder.label(l);
    }
    Ok(builder.finish())
}

fn tile_budget(mode: VizMode, o: &VizOptions) -> u32 {
    match mode {
        VizMode::Treemap | VizMode::Icicle => o.max_tiles,
        VizMode::Sunburst => o.max_arcs,
        VizMode::Pack => o.max_circles,
        VizMode::Mindmap => o.max_graph_nodes,
        _ => o.max_tiles,
    }
}

/// Shared layout context.
struct Ctx<'a> {
    arena: &'a Arena,
    options: &'a VizOptions,
    mode: VizMode,
    tiles: Vec<Tile>,
    labels: Vec<Label>,
    budget: u32,
}

impl Ctx<'_> {
    fn over_budget(&self) -> bool {
        self.tiles.len() as u32 >= self.budget
    }

    fn push(&mut self, node: NodeId, x: f32, y: f32, w: f32, h: f32, depth: u16, extra: u16) {
        if self.over_budget() {
            return;
        }
        let a = self.arena;
        let mut flags = extra;
        match a.kind(node) {
            kind::DIR | kind::ROOT => flags |= tile_flags::DIR,
            kind::FREE_SPACE => flags |= tile_flags::FREE_SPACE,
            kind::UNKNOWN => flags |= tile_flags::UNKNOWN,
            _ => {}
        }
        let color = palette_index(a, node, self.options.color_mode);
        // labels: big-enough tiles only (docs/11 § 3.3)
        if self.mode == VizMode::Treemap || self.mode == VizMode::Icicle {
            if h >= 28.0 && w >= 42.0 && self.labels.len() < 512 {
                let name = a.name_str(node);
                let display = if name.chars().count() > 24 {
                    format!("{}…", name.chars().take(22).collect::<String>())
                } else {
                    name
                };
                self.labels.push(Label {
                    node_id: node as i32,
                    x: x + 4.0,
                    y: y + 14.0,
                    text: display,
                });
            }
        }
        self.tiles.push(Tile {
            node_id: node as i32,
            x,
            y,
            w,
            h,
            argb: color,
            depth,
            flags,
        });
    }

    /// Squarified treemap (Bruls–Huizing–van Wijk). Directories descend; files
    /// are leaf tiles; sub-threshold siblings aggregate into a composite tile
    /// flagged `COMPOSITE` and addressed by the parent (selectable + zoomable).
    fn squarify(&mut self, node: NodeId, x: f32, y: f32, w: f32, h: f32, depth: u32) {
        if w <= 0.5 || h <= 0.5 || depth > self.options.drawn_depth as u32 || self.over_budget() {
            return;
        }
        let a = self.arena;
        let children = a.children(node).to_vec();
        if children.is_empty() {
            self.push(node, x, y, w, h, depth as u16, 0);
            return;
        }
        let total: f64 = children
            .iter()
            .map(|&c| a.allocated(c as usize) as f64)
            .sum();
        if total <= 0.0 {
            self.push(node, x, y, w, h, depth as u16, tile_flags::COMPOSITE);
            return;
        }
        // LOD split
        let area = f64::from(w * h);
        let mut items: Vec<(NodeId, f64)> = Vec::with_capacity(children.len());
        let mut tail = false;
        for &c in &children {
            let s = a.allocated(c as usize) as f64;
            let share = s / total;
            let est_px = (share * area).sqrt();
            if s <= 0.0
                || share < self.options.min_share as f64
                || est_px < self.options.min_tile_px as f64
            {
                tail = true;
            } else {
                items.push((c, s));
            }
        }
        if items.is_empty() {
            self.push(node, x, y, w, h, depth as u16, tile_flags::COMPOSITE);
            return;
        }
        items.sort_by(|p, q| q.1.partial_cmp(&p.1).unwrap_or(std::cmp::Ordering::Equal));

        // Squarified strip placement (paper's rule). Items carry BYTE sizes;
        // the rect carries PIXELS — the thickness scale converts. The
        // remaining rect always corresponds to the remaining item sum.
        let mut rx = x;
        let mut ry = y;
        let mut rw = w;
        let mut rh = h;
        let mut remaining_sum: f64 = items.iter().map(|(_, s)| *s).sum();
        let mut i = 0usize;
        while i < items.len() && !self.over_budget() {
            let along = f64::from(rw.min(rh)); // side the row partitions
            let across = f64::from(rw.max(rh)); // side the strip thickness eats
            // grow the row while the worst aspect ratio improves
            let mut row_sum = 0.0f64;
            let mut best_worst = f64::MAX;
            let row_start = i;
            while i < items.len() {
                let s = items[i].1;
                let cand_sum = row_sum + s;
                let cand_worst = worst_of(
                    &items[row_start..i],
                    s,
                    along,
                    across,
                    cand_sum,
                    remaining_sum,
                );
                if i == row_start || cand_worst <= best_worst {
                    best_worst = cand_worst;
                    row_sum = cand_sum;
                    i += 1;
                } else {
                    break;
                }
            }
            // strip thickness from the byte→pixel share of the across side
            let thickness = ((row_sum / remaining_sum) * across) as f32;
            let vertical_band = rw >= rh; // wider than tall → strip is a band on the left
            let scale = (along / row_sum) as f32;
            let mut cursor = if vertical_band { ry } else { rx };
            for &(c, s) in &items[row_start..i] {
                let len = (s as f32) * scale;
                if vertical_band {
                    self.descend_or_tile(c, rx, cursor, thickness, len, depth);
                    cursor += len;
                } else {
                    self.descend_or_tile(c, cursor, ry, len, thickness, depth);
                    cursor += len;
                }
            }
            if vertical_band {
                rx += thickness;
                rw -= thickness;
            } else {
                ry += thickness;
                rh -= thickness;
            }
            remaining_sum -= row_sum;
        }
        // tail composite into the remaining rect
        if tail {
            let (fx, fy, fw, fh) = if rw > 1.0 && rh > 1.0 {
                (rx, ry, rw, rh)
            } else {
                (x, y, w.max(1.0), h.max(1.0))
            };
            self.push(node, fx, fy, fw, fh, depth as u16, tile_flags::COMPOSITE);
        }
    }

    fn descend_or_tile(&mut self, node: NodeId, x: f32, y: f32, w: f32, h: f32, depth: u32) {
        let k = self.arena.kind(node);
        if k == kind::DIR || k == kind::ROOT {
            self.squarify(node, x, y, w, h, depth + 1);
        } else {
            self.push(node, x, y, w, h, (depth + 1) as u16, 0);
        }
    }

    /// Icicle: rows = depth, width ∝ size.
    fn icicle(&mut self, node: NodeId, x: f32, y: f32, w: f32, depth: u32) {
        const ROW_H: f32 = 26.0;
        if w < 1.0 || depth > self.options.drawn_depth as u32 || self.over_budget() {
            return;
        }
        self.push(node, x, y, w, ROW_H - 1.0, depth as u16, 0);
        let a = self.arena;
        let children = a.children(node).to_vec();
        let total: f64 = children
            .iter()
            .map(|&c| a.allocated(c as usize) as f64)
            .sum::<f64>()
            .max(1.0);
        let mut cx = x;
        for &c in &children {
            let share = a.allocated(c as usize) as f64 / total;
            let cw = (share * f64::from(w)) as f32;
            self.icicle(c, cx, y + ROW_H, cw, depth + 1);
            cx += cw;
        }
    }

    /// Sunburst: polar tiles (r0, a0, r1, a1).
    #[allow(clippy::too_many_arguments)]
    fn sunburst(
        &mut self,
        node: NodeId,
        r0: f32,
        r1: f32,
        a0: f32,
        a1: f32,
        total: f32,
        depth: u32,
    ) {
        if depth > self.options.drawn_depth as u32 || self.over_budget() || a1 - a0 < 0.0005 {
            return;
        }
        let a = self.arena;
        let ring = (r1 - r0).max(6.0);
        if depth > 0 {
            // polar tile: x=r0, y=r0+ring, w=a0, h=a1 (see module docs)
            self.push(node, r0, r0 + ring, a0, a1, depth as u16, 0);
        }
        let children = a.children(node).to_vec();
        if children.is_empty() {
            return;
        }
        let mut angle = a0;
        for &c in &children {
            let share = a.allocated(c as usize) as f32 / total.max(1.0);
            let span = share * (a1 - a0);
            self.sunburst(c, r0 + ring, r1, angle, angle + span, total, depth + 1);
            angle += span;
        }
    }

    /// Circle pack: (x, y) = center, w = radius.
    fn pack(&mut self, node: NodeId, cx: f32, cy: f32, r: f32, depth: u32) {
        if r < 2.0 || depth > self.options.drawn_depth as u32 || self.over_budget() {
            return;
        }
        self.push(node, cx, cy, r, 0.0, depth as u16, 0);
        let a = self.arena;
        let children = a.children(node).to_vec();
        if children.is_empty() || children.len() > 64 {
            return; // deep crowding handled by LOD budget
        }
        let total: f64 = children
            .iter()
            .map(|&c| a.allocated(c as usize) as f64)
            .sum::<f64>()
            .max(1.0);
        // deterministic rings: 4, 8, 12… slots at shrinking radii
        let mut idx = 0usize;
        let mut ring_idx = 0u32;
        let mut per_ring = 4usize;
        let mut rr = r * 0.55;
        while idx < children.len() && rr < r {
            let base = -std::f32::consts::FRAC_PI_2 + ring_idx as f32 * 0.35;
            for k in 0..per_ring {
                if idx >= children.len() {
                    break;
                }
                let c = children[idx];
                let share = a.allocated(c as usize) as f64 / total;
                let cr = ((share as f32).sqrt() * r * 0.42).clamp(2.0, (r * 0.45).max(2.5));
                let ang = base + k as f32 * (std::f32::consts::TAU / per_ring as f32);
                self.pack(c, cx + ang.cos() * rr, cy + ang.sin() * rr, cr, depth + 1);
                idx += 1;
            }
            ring_idx += 1;
            per_ring += 4;
            rr += r * 0.16;
        }
    }

    /// Mindmap: root center, children on rings (x, y) = center, w = radius.
    fn mindmap(&mut self, node: NodeId, cx: f32, cy: f32, r_ring: f32, depth: u32) {
        if depth > self.options.drawn_depth as u32 || self.over_budget() {
            return;
        }
        let a = self.arena;
        if depth == 0 {
            let root_r = 24.0f32;
            self.push(node, cx, cy, root_r, 0.0, 0, 0);
        }
        let children = a.children(node).to_vec();
        if children.is_empty() {
            return;
        }
        let total: f64 = children
            .iter()
            .map(|&c| a.allocated(c as usize) as f64)
            .sum::<f64>()
            .max(1.0);
        let mut angle = -std::f32::consts::FRAC_PI_2;
        let full = std::f32::consts::TAU;
        for &c in &children {
            let share = a.allocated(c as usize) as f64 / total;
            let span = (share as f32) * full;
            let mid = angle + span / 2.0;
            let px = cx + mid.cos() * r_ring;
            let py = cy + mid.sin() * r_ring;
            let node_r = ((share as f32).sqrt() * 60.0).clamp(3.0, 120.0);
            self.push(c, px, py, node_r, 0.0, (depth + 1) as u16, 0);
            self.mindmap(c, cx, cy, r_ring + 46.0, depth + 1);
            angle += span;
        }
    }

    /// Age timeline: 64 byte-weighted mtime columns.
    fn age_timeline(&mut self, root: NodeId, w: f32, h: f32) {
        let a = self.arena;
        let mut sums = vec![0u64; AGE_COLUMNS];
        let mut min_t = i64::MAX;
        let mut max_t = i64::MIN;
        for i in 0..a.len() {
            let t = a.mtime(i as NodeId);
            if t > 0 {
                min_t = min_t.min(t);
                max_t = max_t.max(t);
            }
        }
        if min_t == i64::MAX {
            return; // no timestamps — renderer shows the honest empty state
        }
        let span = (max_t - min_t).max(1);
        for i in 0..a.len() {
            let t = a.mtime(i as NodeId);
            if t > 0 {
                let col = (((t - min_t) as f64 / span as f64) * (AGE_COLUMNS as f64 - 1.0)).round()
                    as usize;
                if let Some(slot) = sums.get_mut(col.min(AGE_COLUMNS - 1)) {
                    *slot += a.allocated(i);
                }
            }
        }
        let peak = sums.iter().copied().max().unwrap_or(1).max(1) as f32;
        let cw = w / AGE_COLUMNS as f32;
        for (i, &s) in sums.iter().enumerate() {
            if s == 0 {
                continue;
            }
            let bh = (s as f32 / peak * (h - 20.0)).max(2.0);
            self.push(
                root,
                i as f32 * cw,
                h - bh,
                cw.max(1.0) - 1.0,
                bh,
                0,
                tile_flags::COMPOSITE,
            );
        }
    }
}

/// Worst aspect ratio for a candidate row (paper's rule). `along` = the rect
/// side the row partitions; `across` = the side the strip thickness consumes;
/// `cand_sum` = row + candidate bytes; `remaining` = bytes still to place in
/// this rect (including the candidate row).
fn worst_of(
    row: &[(NodeId, f64)],
    next: f64,
    along: f64,
    across: f64,
    cand_sum: f64,
    remaining: f64,
) -> f64 {
    let mut worst = 0.0f64;
    for &(_, s) in row.iter().chain(std::iter::once(&(NodeId::MAX, next))) {
        if s <= 0.0 || cand_sum <= 0.0 || remaining <= 0.0 {
            continue;
        }
        let thickness = cand_sum / remaining * across;
        let item_len = s / cand_sum * along;
        let aspect = (thickness / item_len).max(item_len / thickness);
        worst = worst.max(aspect);
    }
    worst
}

/// Palette index for a node under the active color mode (see module docs).
fn palette_index(arena: &Arena, node: NodeId, mode: ColorMode) -> u32 {
    match mode {
        ColorMode::Type => (arena.category(node) as u32 & 0xFFFF) | 0x1_0000,
        ColorMode::Branch => {
            // depth-stable hash of the top-level branch (docs/08 § 4.2)
            let mut cur = node;
            loop {
                let p = arena.parent(cur);
                if p == cur {
                    break;
                }
                let pp = arena.parent(p);
                if pp == p {
                    break; // cur is a depth-1 branch
                }
                cur = p;
            }
            let mut h: u32 = 2166136261;
            for &u in arena.name_utf16(cur) {
                h ^= u32::from(u);
                h = h.wrapping_mul(16777619);
            }
            (h % 12) | 0x2_0000
        }
        ColorMode::Age => {
            let now_ms = crate::agg::now_unix_ms();
            let mtime_ms = crate::scanner::filetime_ticks_to_unix_ms(arena.mtime(node));
            let bucket = crate::agg::age_bucket(mtime_ms, now_ms) as u32;
            bucket | 0x3_0000
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NodeInput;

    fn build_tree() -> Arena {
        let mut a = Arena::with_capacity(32);
        let root_name: Vec<u16> = "r".encode_utf16().collect();
        let root = a.push(NodeInput {
            parent: 0,
            name_utf16: &root_name,
            logical: 0,
            allocated: 0,
            files: 0,
            folders: 1,
            mtime: 0,
            kind: kind::ROOT,
            category: 1,
            ext_id: 0,
            attr_flags: 0,
            link_to: 0,
            err_code: 0,
        });
        let mut kids = Vec::new();
        for (i, sz) in [500u64, 300, 150, 40, 10].iter().enumerate() {
            let nm: Vec<u16> = format!("f{i}.bin").encode_utf16().collect();
            let n = a.push(NodeInput {
                parent: root,
                name_utf16: &nm,
                logical: *sz,
                allocated: *sz,
                files: 1,
                folders: 0,
                mtime: 0,
                kind: kind::FILE,
                category: 1,
                ext_id: 1,
                attr_flags: 0,
                link_to: 0,
                err_code: 0,
            });
            kids.push(n);
            a.attach(root, n);
        }
        // subtree sums (the completion cascade does this in real scans)
        for sz in [500u64, 300, 150, 40, 10] {
            a.add_metrics(root, sz, sz, 1, 0);
        }
        a.set_subtree(root, 5, 1);
        // set root sizes manually for the test
        // (in real scans the completion cascade does this)
        a.finalize_children();
        a
    }

    #[test]
    fn treemap_layout_produces_tiles() {
        let arena = build_tree();
        let opts = VizOptions::default();
        let frame = layout(&arena, 1, 0, VizMode::Treemap, (800.0, 600.0, 1.0), &opts)
            .unwrap_or_else(|e| panic!("{e}"));
        let d = frame::decode(&frame).unwrap_or_else(|e| panic!("{e}"));
        assert!(!d.tiles.is_empty(), "treemap must emit tiles");
        // root total 1000 units over 800×600 → biggest file ≈ half area
        let biggest = d.tiles.iter().map(|t| t.w * t.h).fold(0.0f32, f32::max);
        assert!(
            biggest > 0.4 * 800.0 * 600.0 * 0.5,
            "largest tile should be ~50% of area, got {biggest}"
        );
    }

    #[test]
    fn sunburst_polar_tiles() {
        let arena = build_tree();
        let opts = VizOptions::default();
        let frame = layout(&arena, 1, 0, VizMode::Sunburst, (800.0, 800.0, 1.0), &opts)
            .unwrap_or_else(|e| panic!("{e}"));
        let d = frame::decode(&frame).unwrap_or_else(|e| panic!("{e}"));
        assert!(!d.tiles.is_empty());
    }

    #[test]
    fn dom_modes_rejected() {
        let arena = build_tree();
        let opts = VizOptions::default();
        assert!(layout(&arena, 1, 0, VizMode::Table, (800.0, 600.0, 1.0), &opts).is_err());
    }

    #[test]
    fn invalid_viewport_rejected() {
        let arena = build_tree();
        let opts = VizOptions::default();
        assert!(layout(&arena, 1, 0, VizMode::Treemap, (0.0, 0.0, 1.0), &opts).is_err());
    }
}
