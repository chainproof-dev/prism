//! Exclusion/preset glob matcher (docs/06 § 2.4). Zero-dependency subset:
//! `*`, `?`, `**`, `[class]`. Case-insensitive (NTFS semantics), matched on
//! UTF-16 names with our own fold (no ICU).

/// A compiled glob. Patterns match against **relative path from scan root**
/// with `/` separators (normalized from `\`), e.g. `**/node_modules/**`.
pub struct Glob {
    segments: Vec<Seg>,
    original: String,
}

enum Seg {
    /// `**` — any number of path segments.
    DoubleStar,
    /// One path segment pattern (may contain `*`, `?`, `[...]`).
    One(Vec<Piece>),
}

enum Piece {
    /// Literal char (already case-folded).
    Lit(u16),
    /// `*` within a segment.
    Star,
    /// `?` single char.
    Question,
    /// `[...]` class: (chars, negated).
    Class(Vec<u16>, bool),
}

/// Case-fold a UTF-16 code unit (ASCII fast path; Latin-1 supplement slow
/// path; other planes pass through — NTFS simple fold subset).
fn fold(u: u16) -> u16 {
    if (0x41..=0x5A).contains(&u) {
        u + 32 // A-Z → a-z
    } else if u < 128 {
        u
    } else {
        match char::from_u32(u32::from(u)) {
            Some(c) => c
                .to_lowercase()
                .next()
                .and_then(|c| {
                    let mut buf = [0u16; 2];
                    c.encode_utf16(&mut buf).first().copied()
                })
                .unwrap_or(u),
            None => u,
        }
    }
}

impl Glob {
    /// Compile a glob pattern. Returns `None` when the pattern is empty.
    pub fn compile(pattern: &str) -> Option<Self> {
        if pattern.is_empty() {
            return None;
        }
        let normalized: String = pattern.replace('\\', "/");
        let mut segments = Vec::new();
        for seg in normalized.split('/') {
            if seg == "**" {
                segments.push(Seg::DoubleStar);
            } else {
                segments.push(Seg::One(compile_seg(seg)?));
            }
        }
        Some(Self {
            segments,
            original: pattern.to_string(),
        })
    }

    /// Original pattern (display).
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Match a relative path (segments pre-split, case-folded by us).
    pub fn matches_segments(&self, path_segs: &[&str]) -> bool {
        let mut segs: Vec<Vec<u16>> = Vec::with_capacity(path_segs.len());
        for s in path_segs {
            segs.push(s.encode_utf16().map(fold).collect());
        }
        match_segs(&self.segments, &segs)
    }

    /// Match a relative path string.
    pub fn matches(&self, rel_path: &str) -> bool {
        let normalized = rel_path.replace('\\', "/");
        let parts: Vec<&str> = normalized.split('/').collect();
        self.matches_segments(&parts)
    }
}

fn compile_seg(seg: &str) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut chars = seg
        .encode_utf16()
        .collect::<Vec<u16>>()
        .into_iter()
        .peekable();
    while let Some(c) = chars.next() {
        match c {
            0x2A /* * */ => pieces.push(Piece::Star),
            0x3F /* ? */ => pieces.push(Piece::Question),
            0x5B /* [ */ => {
                let mut cls = Vec::new();
                let negated = chars.peek() == Some(&0x5E /* ^ */);
                if negated {
                    chars.next();
                }
                let mut closed = false;
                for c2 in chars.by_ref() {
                    if c2 == 0x5D /* ] */ {
                        closed = true;
                        break;
                    }
                    cls.push(fold(c2));
                }
                if !closed {
                    return None; // malformed class — compile fails loudly
                }
                pieces.push(Piece::Class(cls, negated));
            }
            _ => pieces.push(Piece::Lit(fold(c))),
        }
    }
    Some(pieces)
}

fn match_segs(pat: &[Seg], path: &[Vec<u16>]) -> bool {
    match pat.first() {
        None => path.is_empty(),
        Some(Seg::DoubleStar) => {
            // `**` consumes zero or more segments
            for skip in 0..=path.len() {
                if match_segs(&pat[1..], &path[skip..]) {
                    return true;
                }
            }
            false
        }
        Some(Seg::One(pieces)) => {
            if path.is_empty() {
                return false;
            }
            match_pieces(pieces, &path[0]) && match_segs(&pat[1..], &path[1..])
        }
    }
}

fn match_pieces(pieces: &[Piece], text: &[u16]) -> bool {
    match pieces.first() {
        None => text.is_empty(),
        Some(Piece::Star) => {
            for skip in 0..=text.len() {
                if match_pieces(&pieces[1..], &text[skip..]) {
                    return true;
                }
            }
            false
        }
        Some(Piece::Lit(l)) => {
            !text.is_empty() && fold(text[0]) == *l && match_pieces(&pieces[1..], &text[1..])
        }
        Some(Piece::Question) => !text.is_empty() && match_pieces(&pieces[1..], &text[1..]),
        Some(Piece::Class(cls, negated)) => {
            if text.is_empty() {
                return false;
            }
            let hit = cls.contains(&fold(text[0]));
            hit != *negated && match_pieces(&pieces[1..], &text[1..])
        }
    }
}

/// A compiled exclusion set (all patterns must fail for a path to be included).
pub struct ExclusionSet {
    globs: Vec<Glob>,
    /// Segment names from patterns like `**/name/**` or `**/name` — a
    /// directory whose *name* matches gets its whole subtree pruned before
    /// enumeration (the preset pattern class).
    sticky_segments: Vec<String>,
}

impl ExclusionSet {
    /// Compile a set of patterns; malformed patterns are surfaced as errors
    /// (fail loud — never silently dropped, A2).
    pub fn compile(patterns: &[String]) -> Result<Self, String> {
        let mut globs = Vec::with_capacity(patterns.len());
        let mut sticky_segments = Vec::new();
        for p in patterns {
            let g =
                Glob::compile(p).ok_or_else(|| format!("malformed exclusion pattern: {p:?}"))?;
            // Derive sticky segments: `**/seg/**` / `**/seg` / `seg/**` / `seg`
            let normalized = p.replace('\\', "/");
            let segs: Vec<&str> = normalized.split('/').collect();
            let is_sticky = segs
                .iter()
                .all(|s| *s == "**" || !s.contains(['*', '?', '[', ']']));
            if is_sticky {
                for s in &segs {
                    if *s != "**" && !s.is_empty() {
                        sticky_segments.push(s.to_lowercase());
                    }
                }
            }
            globs.push(g);
        }
        Ok(Self {
            globs,
            sticky_segments,
        })
    }

    /// Empty set.
    pub fn none() -> Self {
        Self {
            globs: Vec::new(),
            sticky_segments: Vec::new(),
        }
    }

    /// True when the relative path is excluded (full-path match).
    pub fn is_excluded(&self, rel_path: &str) -> bool {
        self.globs.iter().any(|g| g.matches(rel_path))
    }

    /// True when a directory *name* triggers subtree pruning (preset class).
    pub fn is_sticky_dir(&self, name: &str) -> bool {
        !self.sticky_segments.is_empty()
            && self
                .sticky_segments
                .iter()
                .any(|s| s.eq_ignore_ascii_case(name))
    }

    /// Pattern count.
    pub fn len(&self) -> usize {
        self.globs.len()
    }

    /// Empty?
    pub fn is_empty(&self) -> bool {
        self.globs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)] // test policy (docs/06 § 12)

    use super::*;

    #[test]
    fn star_within_segment() {
        let g = Glob::compile("*.log").unwrap();
        assert!(g.matches("a.log"));
        assert!(g.matches("A.LOG")); // case-insensitive
        assert!(!g.matches("LOG")); // no extension — no match
        assert!(!g.matches("a.log.txt"));
        assert!(!g.matches("dir/a.log")); // one segment only
    }

    #[test]
    fn double_star_spans_segments() {
        let g = Glob::compile("**/node_modules/**").unwrap();
        assert!(g.matches("node_modules/foo"));
        assert!(g.matches("a/b/node_modules/pkg/file.js"));
        // trailing `**` matches zero segments: the dir itself is covered
        assert!(g.matches("a/node_modules"));
        assert!(!g.matches("node_modules-extra/file"));
    }

    #[test]
    fn question_and_class() {
        let g = Glob::compile("te?t[0-9].txt").unwrap();
        assert!(g.matches("test0.txt"));
        assert!(g.matches("text9.txt"));
        assert!(!g.matches("testa.txt"));
    }

    #[test]
    fn backslash_normalized() {
        let g = Glob::compile("**\\Temp\\**").unwrap();
        assert!(g.matches("a/Temp/x"));
    }

    #[test]
    fn malformed_class_fails_loud() {
        assert!(Glob::compile("a[b").is_none());
    }

    #[test]
    fn exclusion_set() {
        let set = ExclusionSet::compile(&["**/.git/**".into(), "**/*.tmp".into()]).unwrap();
        assert!(set.is_excluded(".git/objects/ab"));
        assert!(set.is_excluded("root/thing.tmp"));
        assert!(!set.is_excluded("src/main.rs"));
        // sticky dir names prune subtrees
        let set2 = ExclusionSet::compile(&["**/node_modules/**".into()]).unwrap();
        assert!(set2.is_sticky_dir("Node_Modules"));
        assert!(!set2.is_sticky_dir("src"));
    }
}
