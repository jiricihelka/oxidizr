//! Stage 3 of the DSL pipeline: assemble the lossless [`Cst`] from logical
//! lines. The official tooling's parse loop runs its line-level checks IN
//! ORDER, and the order is reproduced exactly:
//!
//! * an all-whitespace line → blank line (never reaches the comment machine);
//! * a whole-line `//`/`#` comment (same);
//! * tokenize, then the block-comment state machine on the TRIMMED joined line
//!   and the FIRST token (after `x =` assignment stripping). The check order is
//!   load-bearing: a whole-line `/* … */` neither opens nor closes a region (so
//!   it does NOT close one that is open); a line whose first token starts with
//!   `/*` opens a region, closed by the next line whose trimmed text ends with
//!   `*/`. Hence `x = /* */` opens a region its own `*/` cannot close, and a
//!   `*/` on a `//`-comment line never closes (check 2 ate that line).
//!
//! Unclosed `/*` swallows to EOF. The first-token test uses the RAW token value —
//! `${VAR}` substitution lands with the parser and can only change this decision
//! for values expanding to `/*`, which no fixture relies on.
//!
//! The dropped EOF tail never reaches these checks (its buffer is discarded),
//! so it is marked [`NodeKind::DroppedTail`] outside any region.

use std::rc::Rc;

use oxidizr_ast::{Cst, Node, NodeKind, TextRange, u32_len};

use crate::compat;
use crate::lexer::{self, LexedLine};
use crate::preprocess::{Logical, preprocess};

/// Byte-offset line index for mapping offsets to 1-based (line, char-column)
/// positions — diagnostics spans and snapshot stability.
// test-only: the shipped parser never builds one (driven by unit tests)
#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct LineIndex {
    /// Byte offset of each line's first character (index 0 is always `0`).
    starts: Vec<u32>,
}

#[cfg(test)]
impl LineIndex {
    /// Index a source (lines split on `\n`, matching the physical-line splitter).
    #[must_use]
    pub(crate) fn new(source: &str) -> LineIndex {
        let mut starts = vec![0_u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                starts.push(u32_len(i + 1));
            }
        }
        LineIndex { starts }
    }

    /// 1-based line and character column of a byte offset (column counts UTF-8
    /// characters from the line start).
    #[must_use]
    pub(crate) fn line_col(&self, source: &str, byte: usize) -> (u32, u32) {
        let line = self
            .starts
            .partition_point(|&s| (s as usize) <= byte)
            .saturating_sub(1);
        let start = self.starts[line] as usize;
        let col = source[start..byte.min(source.len())].chars().count();
        (u32_len(line + 1), u32_len(col + 1))
    }
}

/// Build the lossless CST for a decoded (UTF-8) DSL source.
#[must_use]
pub fn build(source: &str) -> Cst {
    let logicals = preprocess(source);
    let mut children: Vec<Node> = Vec::new();
    let mut region: Vec<Node> = Vec::new();

    for logical in &logicals {
        if logical.dropped {
            // The EOF tail is never parsed; close any region it swallowed
            // first (the region simply ends at EOF), then cover the bytes.
            close_region(&mut children, &mut region);
            children.push(line_node(source, logical, NodeKind::DroppedTail));
            continue;
        }

        // A text-block line joins to `prefix + "\"${placeholder}\""` — never
        // blank, never comment-like — so it outranks the blank/comment checks
        // (its joined PREFIX alone may well be whitespace-only).
        if logical.text_block.is_none() && logical.is_empty_line(source) {
            // (1) blank: no state change, not even inside a region.
            push_line(
                &mut children,
                &mut region,
                line_node(source, logical, NodeKind::BlankLine),
            );
            continue;
        }
        if logical.text_block.is_none() && logical.is_comment_line(source) {
            // (2) whole-line comment: same.
            push_line(
                &mut children,
                &mut region,
                line_node(source, logical, NodeKind::CommentLine),
            );
            continue;
        }

        // (3) code line.
        let lexed = lexer::lex_line(source, logical);
        let node = node_from_leaves(NodeKind::LogicalLine, logical.span, &lexed.leaves);
        let joined = logical.joined(source);
        let trimmed = compat::java_trim(&joined);
        let first = first_token_value(&lexed, source);

        if trimmed.starts_with("/*") && trimmed.ends_with("*/") {
            // (3a) whole-line block comment: no state change.
            let wrapped = Node::new(NodeKind::BlockComment, node.range, vec![node]);
            push_line(&mut children, &mut region, wrapped);
        } else if first.starts_with("/*") {
            // (3b) open a region; this line is its first member.
            region.push(node);
        } else if !region.is_empty() && trimmed.ends_with("*/") {
            // (3c) this line closes the region.
            region.push(node);
            close_region(&mut children, &mut region);
        } else if !region.is_empty() {
            // (3d) swallowed by the open region.
            region.push(node);
        } else {
            children.push(node);
        }
    }
    close_region(&mut children, &mut region);

    let root = Node::new(
        NodeKind::Root,
        TextRange::new(0, u32_len(source.len())),
        children,
    );
    Cst::new(Rc::from(source), root)
}

/// The first token of the STATEMENT: `identifier = rest` is treated as
/// starting at token index 2 (after the `=`), everything else at index 0.
fn first_token_value(lexed: &LexedLine, source: &str) -> String {
    let idx = if lexed.tokens.len() >= 3 && lexed.tokens[1].value(source) == "=" {
        2
    } else {
        0
    };
    lexed
        .tokens
        .get(idx)
        .map_or_else(String::new, |t| t.value(source))
}

/// Wrap one logical line's leaves into a node of the given kind (lexing per kind).
fn line_node(source: &str, logical: &Logical, kind: NodeKind) -> Node {
    let lexed = match kind {
        NodeKind::BlankLine => lexer::lex_blank(source, logical),
        NodeKind::CommentLine => lexer::lex_comment(source, logical),
        _ => lexer::lex_line(source, logical),
    };
    node_from_leaves(kind, logical.span, &lexed.leaves)
}

fn node_from_leaves(kind: NodeKind, span: (usize, usize), leaves: &[lexer::Leaf]) -> Node {
    let children = leaves
        .iter()
        .map(|l| Node::leaf(l.kind, TextRange::new(l.range.start, l.range.end)))
        .collect();
    Node::new(
        kind,
        TextRange::new(u32_len(span.0), u32_len(span.1)),
        children,
    )
}

/// Append a finished line node: into the open block-comment region (order matters —
/// blank/comment lines between region members stay inside it) or the top level.
fn push_line(children: &mut Vec<Node>, region: &mut Vec<Node>, node: Node) {
    if region.is_empty() {
        children.push(node);
    } else {
        region.push(node);
    }
}

/// Close the open block-comment region (no-op when none).
fn close_region(children: &mut Vec<Node>, region: &mut Vec<Node>) {
    let members = std::mem::take(region);
    if members.is_empty() {
        return;
    }
    let start = members.first().expect("non-empty").range.start;
    let end = members.last().expect("non-empty").range.end;
    children.push(Node::new(
        NodeKind::BlockComment,
        TextRange::new(start, end),
        members,
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Compact leaf-kinds-per-line dump for assertions.
    fn sketch(source: &str) -> Vec<(&'static str, Vec<&'static str>)> {
        fn kinds(node: &Node, out: &mut Vec<&'static str>) {
            if node.children.is_empty() {
                out.push(kind_name(node.kind));
            } else {
                for c in &node.children {
                    kinds(c, out);
                }
            }
        }
        let cst = build(source);
        assert_eq!(cst.verify_lossless(), Ok(()), "lossless for {source:?}");
        cst.root
            .children
            .iter()
            .map(|line| {
                let mut out = Vec::new();
                kinds(line, &mut out);
                (kind_name(line.kind), out)
            })
            .collect()
    }

    fn kind_name(kind: NodeKind) -> &'static str {
        match kind {
            NodeKind::Root => "Root",
            NodeKind::LogicalLine => "Code",
            NodeKind::BlankLine => "Blank",
            NodeKind::CommentLine => "Comment-line",
            NodeKind::BlockComment => "BlockComment",
            NodeKind::DroppedTail => "Dropped",
            NodeKind::Whitespace => "ws",
            NodeKind::Newline => "nl",
            NodeKind::Comment => "comment",
            NodeKind::Continuation => "cont",
            NodeKind::Bom => "bom",
            NodeKind::TokenBare => "bare",
            NodeKind::TokenQuoted => "quoted",
            NodeKind::TextBlockLine => "tb-line",
            NodeKind::TextBlockRaw => "tb-raw",
        }
    }

    #[test]
    fn line_classification_and_tiling() {
        use NodeKind::*;
        let src = "\n// c\n# c\nname = \"x\" // trailing\n";
        let cst = build(src);
        assert_eq!(cst.verify_lossless(), Ok(()));
        let kinds: Vec<_> = cst.root.children.iter().map(|n| n.kind).collect();
        assert_eq!(
            kinds,
            vec![BlankLine, CommentLine, CommentLine, LogicalLine]
        );
        let _ = sketch(src);
    }

    #[test]
    fn whole_line_block_comment_no_state_change() {
        let src = "a\n/* one comment */\nb\n";
        let s = sketch(src);
        assert_eq!(s[1].0, "BlockComment");
        assert_eq!(s[2].0, "Code");
    }

    #[test]
    fn region_opens_on_first_token_and_closes_on_trim_end() {
        let src = "/* start\nmiddle\n} still comment\nend */\ncode\n";
        let s = sketch(src);
        assert_eq!(s[0].0, "BlockComment");
        // one region covering lines 1–4
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].0, "Code");
    }

    #[test]
    fn assignment_stripping_exposes_the_comment_token() {
        // `x = /* */` → check (3a) fails (line starts with `x`), so the first token after
        // strip is `/*` → region opens; the same line's trailing `*/` cannot close it
        // (check order), `next` is swallowed, line `*/` finally closes.
        let src = "x = /* */\nnext\n*/\nafter\n";
        let s = sketch(src);
        assert_eq!(s.len(), 2, "region covers lines 1–3, then code");
        assert_eq!(s[0].0, "BlockComment");
        assert_eq!(s[1].0, "Code");
    }

    #[test]
    fn comment_line_with_star_slash_does_not_close_region() {
        let src = "/* open\n// has */ inside\nreal */\nx\n";
        let s = sketch(src);
        assert_eq!(s.len(), 2, "region = lines 1–3, then code");
        assert_eq!(s[0].0, "BlockComment");
        assert_eq!(s[1].0, "Code");
    }

    #[test]
    fn whole_line_comment_inside_region_does_not_close_it() {
        let src = "/* open\n/* solo */\nkeep */\nx\n";
        let s = sketch(src);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn unclosed_region_swallows_to_eof() {
        let src = "a\n/* never closed\nb\n";
        let s = sketch(src);
        assert_eq!(s.len(), 2);
        assert_eq!(s[1].0, "BlockComment");
    }

    #[test]
    fn dropped_tail_outside_region() {
        let src = "a\nb \\\n";
        let s = sketch(src);
        assert_eq!(s[1].0, "Dropped");
    }

    #[test]
    fn line_index_positions() {
        let src = "ab\u{e9}\nsecond\r\nthird";
        let idx = LineIndex::new(src);
        assert_eq!(idx.line_col(src, 0), (1, 1));
        assert_eq!(idx.line_col(src, 2), (1, 3)); // start of the 2-byte é
        let second = src.find('s').unwrap();
        assert_eq!(idx.line_col(src, second), (2, 1));
        assert_eq!(idx.line_col(src, second + 6), (2, 7));
        let third = src.rfind("third").unwrap();
        assert_eq!(idx.line_col(src, third), (3, 1));
    }
}
