//! `oxidizr-ast` — the DSL concrete syntax tree: kind-tagged nodes with children,
//! trivia (comments/whitespace preserved for `fmt`) and spans. The losslessness
//! invariant (`Cst` leaf text reconstructs the source byte-for-byte, P1-D3) is defined
//! here and exercised by `oxidizr-dsl`.
//!
//! LSP (P4) consumes this crate + `oxidizr-diag` without pulling in the parser.
//!
//! Shape: an owned tree ([`Node`]) over a source buffer the caller keeps alive
//! separately (nodes address it with [`TextRange`] byte offsets, `u32` — DSL files
//! over 4 GiB are out of scope). Trivia are leaves like everything else, so
//! [`Cst::verify_lossless`] is a simple contiguity check over in-order leaves.

use std::fmt;
use std::rc::Rc;

/// Byte offset range into the source ([`Cst::text`] buffer). `u32` bounds sources at
/// 4 GiB — DSL workspaces are text files far below that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextRange {
    /// Inclusive start byte.
    pub start: u32,
    /// Exclusive end byte.
    pub end: u32,
}

impl TextRange {
    /// Construct a range.
    #[must_use]
    pub const fn new(start: u32, end: u32) -> TextRange {
        TextRange { start, end }
    }

    /// Length in bytes.
    #[must_use]
    pub const fn len(self) -> u32 {
        self.end - self.start
    }

    /// Whether the range covers no bytes.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// Narrow a byte length to `u32` by saturation (sources above 4 GiB are out of
/// scope, [`TextRange`]) — the one place the `usize` → `u32` narrowing lives, so
/// callers stay free of raw casts.
#[must_use]
pub fn u32_len(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// Node taxonomy. Leaves carry no children; composites group leaves for the parser
/// and formatter. Kinds added by the M4 parser (statements, scopes) extend this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// Tree root; spans the whole source.
    Root,
    /// One reference "logical line" (after `\`-continuation joins and text-block
    /// folding); may cover several physical lines.
    LogicalLine,
    /// A logical line matching the reference `EMPTY_LINE_PATTERN` (blank).
    BlankLine,
    /// A logical line matching the reference `COMMENT_PATTERN` (whole-line `//`/`#`).
    CommentLine,
    /// A `/* … */` region per the reference parse-loop state machine: one whole-line
    /// comment, or the line that opens the region through the line that closes it.
    BlockComment,
    /// The silently-dropped EOF tail (unfinished continuation / unterminated text
    /// block): covered for losslessness, never parsed (the reference discards it).
    DroppedTail,
    /// Run of spaces/tabs (indentation and inter-token gaps).
    Whitespace,
    /// A line ending: `\n` or `\r\n` (kept so `text()` is byte-exact).
    Newline,
    /// A `// …` or `# …` line comment (trailing or whole-line).
    Comment,
    /// The trailing `\` of a continued line (trivia: it vanishes from the logical
    /// line but must survive `text()`).
    Continuation,
    /// A UTF-8 BOM (`U+FEFF`) the reference strips at parse time (trivia).
    Bom,
    /// A bare token (unquoted, whitespace-delimited).
    TokenBare,
    /// A quoted token, range including the `"` delimiters.
    TokenQuoted,
    /// A line containing a `"""` text block: prefix tokens + [`NodeKind::TextBlockRaw`].
    TextBlockLine,
    /// Raw `""" … """` region (opener through the closing line's end) as one leaf.
    TextBlockRaw,
}

/// One CST node.
#[derive(Debug, Clone)]
pub struct Node {
    /// Taxonomy tag.
    pub kind: NodeKind,
    /// Byte span in the source; composites span their children's extent.
    pub range: TextRange,
    /// Children in source order (leaf kinds have none).
    pub children: Vec<Node>,
}

impl Node {
    /// Leaf node of the given kind and span.
    #[must_use]
    pub fn leaf(kind: NodeKind, range: TextRange) -> Node {
        Node {
            kind,
            range,
            children: Vec::new(),
        }
    }

    /// Composite node over `children` (all same source).
    #[must_use]
    pub fn new(kind: NodeKind, range: TextRange, children: Vec<Node>) -> Node {
        Node {
            kind,
            range,
            children,
        }
    }

    /// Walk to the first node satisfying `predicate`, depth-first in source order.
    #[must_use]
    pub fn find(&self, predicate: &dyn Fn(&Node) -> bool) -> Option<&Node> {
        if predicate(self) {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(predicate))
    }
}

/// A lossless CST over `source`. Shared via `Rc` (single-threaded clones are cheap;
/// Send/Sync not needed pre-LSP — revisit at P4 if the LSP needs cross-thread moves).
#[derive(Debug, Clone)]
pub struct Cst {
    /// Full source text.
    pub source: Rc<str>,
    /// Root node ([`NodeKind::Root`]).
    pub root: Node,
}

/// Violation of the losslessness invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LosslessnessError {
    /// A byte gap between consecutive in-order leaves.
    Gap {
        /// First uncovered byte offset.
        at: u32,
    },
    /// Leaves overlap or go backwards.
    Overlap {
        /// Offset where monotonicity broke.
        at: u32,
    },
    /// Leaves do not cover the full source.
    LengthMismatch {
        /// Total covered bytes.
        covered: u32,
        /// Source length in bytes.
        source_len: u32,
    },
    /// The root kind is not [`NodeKind::Root`].
    BadRoot,
}

impl fmt::Display for LosslessnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LosslessnessError::Gap { at } => write!(f, "lossless CST: gap at byte {at}"),
            LosslessnessError::Overlap { at } => write!(f, "lossless CST: overlap at byte {at}"),
            LosslessnessError::LengthMismatch {
                covered,
                source_len,
            } => write!(
                f,
                "lossless CST: leaves cover {covered} bytes, source is {source_len}"
            ),
            LosslessnessError::BadRoot => write!(f, "lossless CST: root is not NodeKind::Root"),
        }
    }
}

impl std::error::Error for LosslessnessError {}

impl Cst {
    /// Assemble a CST; checks the root kind eagerly (cheap).
    #[must_use]
    pub fn new(source: Rc<str>, root: Node) -> Cst {
        debug_assert_eq!(root.kind, NodeKind::Root);
        Cst { source, root }
    }

    /// Collect leaf ranges in source order.
    #[must_use]
    pub fn leaves(&self) -> Vec<TextRange> {
        fn walk(node: &Node, out: &mut Vec<TextRange>) {
            if node.children.is_empty() {
                out.push(node.range);
            } else {
                for child in &node.children {
                    walk(child, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    /// Source text of a node (zero-copy slice).
    #[must_use]
    pub fn text(&self, node: &Node) -> &str {
        &self.source[node.range.start as usize..node.range.end as usize]
    }

    /// The losslessness invariant: in-order leaves tile the source exactly, so
    /// concatenating leaf text reproduces every byte (whitespace, comments, `\`,
    /// CRLF endings, and text blocks included).
    ///
    /// # Errors
    ///
    /// [`LosslessnessError`] on the first gap / overlap / coverage mismatch found.
    pub fn verify_lossless(&self) -> Result<(), LosslessnessError> {
        if self.root.kind != NodeKind::Root {
            return Err(LosslessnessError::BadRoot);
        }
        let mut cursor: u32 = 0;
        for leaf in self.leaves() {
            if leaf.start > cursor {
                return Err(LosslessnessError::Gap { at: cursor });
            }
            if leaf.start < cursor {
                return Err(LosslessnessError::Overlap { at: cursor });
            }
            cursor = leaf.end;
        }
        let len = u32::try_from(self.source.len()).unwrap_or(u32::MAX);
        if cursor != len {
            return Err(LosslessnessError::LengthMismatch {
                covered: cursor,
                source_len: len,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn cst(src: &str, leaves: Vec<(NodeKind, u32, u32)>) -> Cst {
        let root = Node::new(
            NodeKind::Root,
            TextRange::new(0, u32_len(src.len())),
            leaves
                .into_iter()
                .map(|(k, s, e)| Node::leaf(k, TextRange::new(s, e)))
                .collect(),
        );
        Cst::new(Rc::from(src), root)
    }

    #[test]
    fn tiles_pass() {
        let c = cst(
            "ab c",
            vec![
                (NodeKind::TokenBare, 0, 2),
                (NodeKind::Whitespace, 2, 3),
                (NodeKind::TokenBare, 3, 4),
            ],
        );
        assert_eq!(c.verify_lossless(), Ok(()));
    }

    #[test]
    fn gap_detected() {
        let c = cst(
            "ab c",
            vec![(NodeKind::TokenBare, 0, 2), (NodeKind::TokenBare, 3, 4)],
        );
        assert_eq!(c.verify_lossless(), Err(LosslessnessError::Gap { at: 2 }));
    }

    #[test]
    fn overlap_and_length_detected() {
        let c = cst(
            "abcd",
            vec![(NodeKind::TokenBare, 0, 3), (NodeKind::TokenBare, 2, 4)],
        );
        assert_eq!(
            c.verify_lossless(),
            Err(LosslessnessError::Overlap { at: 3 })
        );
        let c = cst("abcd", vec![(NodeKind::TokenBare, 0, 3)]);
        assert_eq!(
            c.verify_lossless(),
            Err(LosslessnessError::LengthMismatch {
                covered: 3,
                source_len: 4
            })
        );
    }

    #[test]
    fn nested_leaves_order() {
        let inner = vec![
            Node::leaf(NodeKind::Whitespace, TextRange::new(0, 1)),
            Node::leaf(NodeKind::TokenBare, TextRange::new(1, 3)),
        ];
        let line = Node::new(NodeKind::LogicalLine, TextRange::new(0, 3), inner);
        let c = Cst::new(
            Rc::from(" ab"),
            Node::new(NodeKind::Root, TextRange::new(0, 3), vec![line]),
        );
        assert_eq!(c.verify_lossless(), Ok(()));
        assert_eq!(c.leaves().len(), 2);
        assert_eq!(c.text(&c.root), " ab");
    }
}
