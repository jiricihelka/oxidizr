//! `textDocument/formatting` + `textDocument/rangeFormatting`
//! .
//!
//! Both requests print through the canonical formatter
//! ([`oxidizr_dsl::fmt::format`]): lossless over the CST and NEVER
//! validating (the lexer is infallible), so buffers WITH parse errors
//! format — `parse(fmt(x)) == parse(x)` holds for every source.
//!
//! * **Whole document** — ONE [`TextEdit`] replacing the entire buffer
//!   with the canonical print; the array is EMPTY when the buffer is
//!   already canonical (no spurious edits — clients mark buffers dirty
//!   on every applied edit).
//! * **Range** — widened to the top-level statement(s) the request
//!   touches: canonical style is a GLOBAL re-print (indent = brace
//!   depth from column 0, blank runs compacted), so an edit that
//!   stopped mid-statement would disagree with the very next full
//!   format. A request sitting in trivia (blank/comment runs between
//!   statements) formats the NEXT statement; a trailing one the
//!   PREVIOUS.
//! * **JSON buffers are never formatted** — `.json` is the byte
//!   contract and the DSL printer would destroy it (the sibling route
//!   of [`crate::diagnostics::collect`]).

use lsp_types::{Range, TextEdit, Uri};
use oxidizr_ast::{Cst, Node, NodeKind};
use oxidizr_dsl::{cst, fmt};

use crate::completion::position_to_offset;
use crate::diagnostics::{is_json, position};

/// Whole-document formatting: the canonical print as ONE whole-buffer
/// [`TextEdit`], or the EMPTY array (JSON route, canonical buffer).
#[must_use]
pub(crate) fn full_document(uri: &Uri, text: &str) -> Vec<TextEdit> {
    if is_json(uri) {
        return Vec::new();
    }
    edit(text, 0, text.len())
}

/// Range formatting: the requested [`Range`] widened to the top-level
/// statements it covers; EMPTY array when the document holds no
/// statement at all, or the selected statements already are canonical.
#[must_use]
pub fn range(uri: &Uri, text: &str, request: Range) -> Vec<TextEdit> {
    if is_json(uri) {
        return Vec::new();
    }
    let from = position_to_offset(text, request.start);
    let to = position_to_offset(text, request.end).max(from);
    let statements = statements(text);
    selection(&statements, from, to).map_or_else(Vec::new, |(start, end)| edit(text, start, end))
}

/// The print of `text[start..end]` as one [`TextEdit`] spanning it —
/// EMPTY array when that slice already is canonical.
fn edit(text: &str, start: usize, end: usize) -> Vec<TextEdit> {
    let slice = &text[start..end];
    let formatted = fmt::format(slice);
    if formatted == slice {
        return Vec::new();
    }
    vec![TextEdit {
        range: Range::new(position(text, start), position(text, end)),
        new_text: formatted,
    }]
}

/// Top-level statements: maximal runs of CST lines from brace depth 0
/// back to depth 0 (a `workspace { … }` is ONE statement; a directive
/// or metadata line is one of its own). Top-level blanks/comments are
/// trivia and bind to NEITHER; braces still open at EOF end the open
/// statement at the buffer end (a broken buffer formats too — the
/// printer never validates). Spans end on a line-terminator boundary so
/// a statement edit can never leave a half line behind.
fn statements(text: &str) -> Vec<(usize, usize)> {
    let cst = cst::build(text);
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut open: Option<usize> = None;
    for line in &cst.root.children {
        if depth == 0
            && matches!(
                line.kind,
                NodeKind::BlankLine
                    | NodeKind::CommentLine
                    | NodeKind::BlockComment
                    | NodeKind::DroppedTail
            )
        {
            continue; // top-level trivia (blank / comment / dropped tail)
        }
        let delta = brace_delta(&cst, line);
        if open.is_none() {
            open = Some(usize::try_from(line.range.start).unwrap_or(usize::MAX));
        }
        depth = (depth + delta).max(0); // stray closers cannot go negative
        if depth == 0
            && let Some(begin) = open.take()
        {
            let end = terminator_end(text, usize::try_from(line.range.end).unwrap_or(usize::MAX));
            out.push((begin, end));
        }
    }
    if let Some(begin) = open {
        out.push((begin, text.len()));
    }
    out
}

/// `end` pulled through the line's terminating newline when it sits at
/// the CST node's edge (the printer counts a statement as INCLUDING its
/// last line break — the edit boundary must match).
fn terminator_end(text: &str, end: usize) -> usize {
    let rest = &text[end..];
    if rest.starts_with("\r\n") {
        end + 2
    } else if rest.starts_with('\n') {
        end + 1
    } else {
        end
    }
}

/// `{`/`}` count of one CST line — BARE tokens only, so braces inside
/// quoted values, `"""` text blocks or comments never move the depth.
fn brace_delta(cst: &Cst, node: &Node) -> i32 {
    let mut delta = 0i32;
    if node.kind == NodeKind::TokenBare {
        let start = usize::try_from(node.range.start).unwrap_or(usize::MAX);
        let end = usize::try_from(node.range.end).unwrap_or(usize::MAX);
        let token = &cst.source[start..end];
        if token == "{" {
            delta += 1;
        } else if token == "}" {
            delta -= 1;
        }
    }
    for child in &node.children {
        delta += brace_delta(cst, child);
    }
    delta
}

/// The span covering EVERY top-level statement overlapping `[from, to)`
/// (a cursor — `from == to` — overlaps the statement containing it or
/// the one it touches at its left edge, and several selected statements
/// join into one contiguous edit). Nothing overlaps: the NEXT statement,
/// else the PREVIOUS, else `None` (a document with no statements).
fn selection(statements: &[(usize, usize)], from: usize, to: usize) -> Option<(usize, usize)> {
    let probe = to.max(from + 1); // a cursor still overlaps the statement containing it
    let mut selected: Option<(usize, usize)> = None;
    for &(start, end) in statements {
        if start < probe && end > from {
            let entry = selected.get_or_insert((start, end));
            entry.0 = entry.0.min(start);
            entry.1 = entry.1.max(end);
        }
    }
    selected.or_else(|| {
        statements
            .iter()
            .find(|(start, _)| *start > from)
            .copied()
            .or_else(|| {
                statements
                    .iter()
                    .filter(|(_, end)| *end <= from)
                    .max_by_key(|(_, end)| *end)
                    .copied()
            })
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use lsp_types::Position;

    fn dsl() -> Uri {
        "file:///w.dsl".parse().unwrap()
    }

    fn json() -> Uri {
        "file:///w.json".parse().unwrap()
    }

    const SLOPPY: &str = "workspace {\nname \"x\"\n}\n";

    /// The one edit of a non-empty answer.
    fn single(edits: &[TextEdit]) -> TextEdit {
        let [edit] = edits else {
            panic!("exactly one edit: {edits:?}");
        };
        edit.clone()
    }

    #[test]
    fn full_document_prints_one_whole_buffer_edit() {
        let edits = full_document(&dsl(), SLOPPY);
        let edit = single(&edits);
        assert_eq!(edit.range.start, Position::new(0, 0));
        assert_eq!(edit.range.end, Position::new(3, 0));
        // Canonical style: 4-space indent, and the printer's conservative
        // unquoting drops the quotes the tokenizer does not need.
        assert_eq!(edit.new_text, "workspace {\n    name x\n}\n");
    }

    #[test]
    fn canonical_and_empty_buffers_answer_empty() {
        let edits = full_document(&dsl(), SLOPPY);
        let printed = single(&edits).new_text;
        let again = full_document(&dsl(), &printed);
        assert!(again.is_empty(), "canonical buffer needs no edit");
        let empty = full_document(&dsl(), "");
        assert!(empty.is_empty(), "empty buffer needs no edit");
    }

    #[test]
    fn json_buffers_are_never_touched() {
        let json_text = "{\n\"views\":{\n}}";
        let full = full_document(&json(), json_text);
        assert!(full.is_empty(), "the byte contract never formats");
        let selected = range(
            &json(),
            json_text,
            Range::new(Position::new(1, 0), Position::new(1, 1)),
        );
        assert!(selected.is_empty(), "the byte contract never formats");
    }

    #[test]
    fn broken_buffers_still_format_and_formatting_is_idempotent() {
        // Unbalanced braces + junk: the lexer is infallible, the printer
        // re-indents what it can (the L-6 contract).
        let broken = "workspace {\n   name \"x\"\n oops ~~~\n";
        let edits = full_document(&dsl(), broken);
        let printed = single(&edits).new_text;
        assert_ne!(printed, broken);
        let again = full_document(&dsl(), &printed);
        assert!(again.is_empty(), "fmt(fmt(x)) == fmt(x)");
    }

    #[test]
    fn range_widens_to_the_whole_statement() {
        // The `name` line belongs to the ONE workspace statement — the
        // edit spans the ENTIRE statement, not the requested line.
        let edits = range(
            &dsl(),
            SLOPPY,
            Range::new(Position::new(1, 1), Position::new(1, 3)),
        );
        let edit = single(&edits);
        assert_eq!(edit.range.start, Position::new(0, 0));
        assert_eq!(edit.range.end, Position::new(3, 0));
    }

    #[test]
    fn range_cuts_at_the_statement_boundary() {
        // Line 0 is its own top-level statement: the edit stops there.
        // (The directive carries a DOUBLE space, so the print differs.)
        let text = "!var  A 1\nworkspace {\nname \"x\"\n}\n";
        let edits = range(
            &dsl(),
            text,
            Range::new(Position::new(0, 0), Position::new(0, 2)),
        );
        let edit = single(&edits);
        assert_eq!(edit.range.end, Position::new(1, 0), "{:?}", edit.range);
        assert_eq!(edit.new_text, "!var A 1\n");
    }

    #[test]
    fn trivia_walks_to_the_next_or_previous_statement() {
        // Cursor on the leading blank line → the workspace statement.
        let text = "\n\nworkspace {\nname \"x\"\n}\n";
        let edits = range(
            &dsl(),
            text,
            Range::new(Position::new(0, 0), Position::new(0, 0)),
        );
        let edit = single(&edits);
        assert_eq!(edit.range.start, Position::new(2, 0));
        // Cursor at EOF (past the final newline): the PREVIOUS statement.
        let edits = range(
            &dsl(),
            SLOPPY,
            Range::new(Position::new(3, 0), Position::new(3, 0)),
        );
        assert_eq!(edits.len(), 1, "a trailing cursor formats backwards");
    }

    #[test]
    fn a_statementless_document_answers_empty() {
        let comments = "// just a comment\n\n# and another\n";
        let edits = range(
            &dsl(),
            comments,
            Range::new(Position::new(0, 0), Position::new(2, 5)),
        );
        assert!(edits.is_empty(), "no statements, nothing to format");
    }
}
