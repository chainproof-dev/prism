//! Visualization DTOs (docs/05 § 3.4, docs/11).

use serde::{Deserialize, Serialize};

use crate::ids::{NodeId, ScanId};

/// The nine visualization modes (docs/11 § 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VizMode {
    /// Squarified cushion treemap (default; parity TMP rows).
    Treemap,
    /// Radial icicle (angle ∝ size, rings = depth).
    Sunburst,
    /// Width ∝ size, rows = depth.
    Icicle,
    /// Nested circle pack.
    Pack,
    /// Radial weighted tree.
    Mindmap,
    /// One level at a time, DOM cards.
    Folders,
    /// Ranked table (DOM).
    Table,
    /// Ranked bars (DOM).
    Bars,
    /// Byte-weighted mtime histogram.
    AgeTimeline,
}

/// Color encoding modes (viz rail).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ColorMode {
    /// Category spectrum (default).
    Type,
    /// Depth-stable hue hashing of top-level branches.
    Branch,
    /// Age ramp on mtime.
    Age,
}

/// Layout options (LOD + mode params, docs/11 § 3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VizOptions {
    /// Max tiles (treemap/icicle; default 60000).
    pub max_tiles: u32,
    /// Max arcs (sunburst; default 12000).
    pub max_arcs: u32,
    /// Max circles (pack; default 8000).
    pub max_circles: u32,
    /// Max graph nodes (mindmap; default 6000).
    pub max_graph_nodes: u32,
    /// Min tile size in px (default 3.0).
    pub min_tile_px: f32,
    /// Min share of parent (default 0.0004).
    pub min_share: f32,
    /// Drawn depth cutoff (default 6, slider 2–12).
    pub drawn_depth: u8,
    /// Tile gap px (0–3, parity-TMP-07).
    pub gap_px: u8,
    /// Cushion elevation E (default 0.55).
    pub cushion_elevation: f32,
    /// Cushion falloff f (default 0.66).
    pub cushion_falloff: f32,
    /// Label density 0–1 (default 0.5).
    pub label_density: f32,
    /// Color mode.
    pub color_mode: ColorMode,
    /// Active size mode.
    pub size_mode: crate::scan::SizeMode,
}

impl Default for VizOptions {
    fn default() -> Self {
        Self {
            max_tiles: 60_000,
            max_arcs: 12_000,
            max_circles: 8_000,
            max_graph_nodes: 6_000,
            min_tile_px: 3.0,
            min_share: 0.0004,
            drawn_depth: 6,
            gap_px: 1,
            cushion_elevation: 0.55,
            cushion_falloff: 0.66,
            label_density: 0.5,
            color_mode: ColorMode::Type,
            size_mode: crate::scan::SizeMode::Allocated,
        }
    }
}

/// Viewport in CSS pixels + device pixel ratio.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Viewport {
    /// Width CSS px.
    pub w: f32,
    /// Height CSS px.
    pub h: f32,
    /// Device pixel ratio (canvas backing = ×dpr, capped 2.0 renderer-side).
    pub dpr: f32,
}

/// `viz:layout` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VizLayoutQuery {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Mode.
    pub mode: VizMode,
    /// Root node for this frame (zoom root).
    pub root: NodeId,
    /// Viewport.
    pub viewport: Viewport,
    /// Options.
    pub options: VizOptions,
}

/// One legend row (type/branch/age depending on color mode).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegendItem {
    /// Category (type mode) or branch id or age bucket.
    pub key: String,
    /// Display label.
    pub label: String,
    /// Color as `#RRGGBB` (theme-rendered by token pipeline).
    pub color: String,
    /// Share of scan bytes (0–1) for legend ordering.
    pub share: f32,
}

/// `viz:color-mapping` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorMapping {
    /// Legend rows.
    pub legend: Vec<LegendItem>,
    /// Palette version (bumps on user color edits).
    pub version: u32,
}
