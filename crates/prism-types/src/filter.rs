//! Filter queries (docs/05 § 3.6, docs/10 § Search & filter).

use serde::{Deserialize, Serialize};

use crate::ids::{CategoryId, ScanId};

/// Inclusive numeric range facet.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    /// Lower bound (inclusive).
    pub min: f64,
    /// Upper bound (inclusive).
    pub max: f64,
}

/// What kind of nodes a filter matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterKind {
    /// Files only.
    Files,
    /// Directories only.
    Dirs,
    /// Both.
    Both,
}

/// Filter query (search field + facets, docs/10 § Search).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterQuery {
    /// Name pattern: glob chars `*?[]` auto-detected vs regex; engine reports
    /// which interpretation was used (labeled honestly, never silent).
    pub name: String,
    /// Category facet (empty = all).
    pub categories: Vec<CategoryId>,
    /// Size facet in bytes (optional).
    pub size: Option<Range>,
    /// Age facet in days (optional).
    pub age: Option<Range>,
    /// Node kind facet.
    pub kind: FilterKind,
}

/// How the engine interpreted the name pattern (surfaced in the filter chip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterPatternKind {
    /// Glob interpretation.
    Glob,
    /// Regex interpretation.
    Regex,
    /// Literal substring (no meta chars).
    Literal,
}

/// Filter result descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterResult {
    /// Filter id for paging.
    pub filter_id: u32,
    /// Matched node count.
    pub matched: u64,
    /// Pattern interpretation actually used.
    pub pattern_kind: FilterPatternKind,
    /// Name pattern compiled (for the chip label).
    pub pattern_display: String,
}

/// Filter apply request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterApplyQuery {
    /// Scan lease.
    pub scan_id: ScanId,
    /// Query body.
    pub query: FilterQuery,
    /// Scope: node id or whole scan.
    pub scope: FilterScope,
}

/// Filter scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterScope {
    /// Whole scan.
    All,
    /// Subtree of one node.
    Node(u32),
}

impl From<FilterScope> for Option<u32> {
    fn from(value: FilterScope) -> Self {
        match value {
            FilterScope::All => None,
            FilterScope::Node(id) => Some(id),
        }
    }
}
