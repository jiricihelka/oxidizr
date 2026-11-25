//! `fmt` — the canonical formatter over the lossless CST.
//!
//! Prints one formatted line per logical line of [`crate::cst::build`], so the
//! token stream per logical line is preserved exactly and the official parser parse
//! loop (which sees only logical lines) reproduces the same workspace:
//! `parse(fmt(x)) == parse(x)`. Canonical style: [`INDENT`]-per-brace-depth,
//! one statement per line, blank-line runs compacted to at most one, whole-line
//! comments re-indented, conservative unquoting (quote only when the tokenizer
//! needs it — see `unquote_value`).
//!
//! Byte-verbatim zones (semantics live in their exact bytes, so they are never
//! re-printed): `/* … */` block-comment regions after the opener line, `"""`
//! text-block raw regions, and the dropped EOF tail.
//!
//! Two line-end guards mirror the official preprocessing:
//! a rendered line whose content would END with `\` or `"""` gets
//! one protective space (printed bare, the re-parse would see a continuation /
//! text-block opener the original did not, swallowing following lines), the same
//! space guarding a trimmed continuation-joined comment whose last line ends `\`;
//! and a content-final `\r` (only reachable inside a quoted value at EOF) is doubled,
//! so it cannot fuse with the emitted LF into a `\r\n` terminator. Trailing
//! whitespace is otherwise dropped — the official parser splits on `\r?\n`, so a CR
//! before `\n` is a delimiter, never content, and terminators print as LF.
//!
//! The formatter never validates: the lexer is infallible, so any source
//! formats (invalid statements re-print token-identically and still fail to
//! parse, exactly as before formatting).

use oxidizr_ast::{Cst, Node, NodeKind};

use crate::compat;
use crate::preprocess::{self, Logical};

/// Canonical indentation unit — 4 spaces, matching the official parser docs' examples
/// (single source of truth for `fmt`).
pub const INDENT: &str = "    ";

/// Reformat DSL source to the canonical style. Invariants (tested in
/// `tests/m6_fmt.rs`): `parse(fmt(x))` builds the identical workspace to
/// `parse(x)`; `fmt(fmt(x)) == fmt(x)`.
#[must_use]
pub fn format(source: &str) -> String {
    let cst = crate::cst::build(source);
    let mut out = String::with_capacity(source.len() + 16);
    let mut depth: usize = 0;
    let mut blank_pending = false;
    let mut wrote_any = false;

    // The CST line nodes correspond 1:1 (in order) to the logical lines the
    // CST was built from; the cursor pairs each code line with its `Logical`
    // so the printer can tell in-segment whitespace (real gaps) from
    // between-segment gap whitespace (indent the official parser `stripLeading`
    // dropped at continuation joins).
    let logicals = preprocess::preprocess(source);
    let mut cursor = 0_usize;

    for line in &cst.root.children {
        match line.kind {
            NodeKind::BlankLine => {
                // Compaction: at most one blank, none before the first line.
                cursor += 1;
                blank_pending |= wrote_any;
            }
            NodeKind::CommentLine => {
                flush_blank(&mut out, &mut blank_pending);
                comment_line(&cst, line, depth, &mut out);
                cursor += 1;
                wrote_any = true;
            }
            NodeKind::BlockComment => {
                flush_blank(&mut out, &mut blank_pending);
                // First member is always the `/*`-opening LogicalLine (a region
                // opens only on a code line), i.e. the next logical.
                block_comment(&cst, line, &logicals[cursor], depth, &mut out);
                cursor += line.children.len();
                wrote_any = true;
            }
            NodeKind::LogicalLine | NodeKind::TextBlockLine => {
                flush_blank(&mut out, &mut blank_pending);
                let logical = &logicals[cursor];
                debug_assert_eq!(
                    (line.range.start as usize, line.range.end as usize),
                    logical.span,
                    "CST line and logical line desynchronized"
                );
                let code = code_line(&cst, line, logical);
                cursor += 1;
                if let Some(text) = code.verbatim {
                    out.push_str(text);
                } else {
                    let shown = depth.saturating_sub(usize::from(code.dedent));
                    out.push_str(&INDENT.repeat(shown));
                    if shown == 0 && code.body.starts_with('\u{FEFF}') {
                        // Line-START guard (mirrors the line-end guards): a
                        // BOM-led token re-indented to column 0 would sit at
                        // byte 0 of the rendered line, where the official parser
                        // parse loop strips it (and re-print drops it) —
                        // `fmt` would not be idempotent. A protective space
                        // keeps byte 0 a space; the official parser `trim` never
                        // reaches the BOM, so the token survives re-parses.
                        out.push(' ');
                    }
                    out.push_str(&code.body);
                    out.push_str(&code.tail_ws);
                    if code.needs_nl {
                        // A content-final `\r` must not fuse with the emitted LF
                        // into a `\r\n` terminator (the official parser `split("\\r?\\n")`
                        // would drop it from the line) — double it: `X\r` + `\r\n`
                        // re-splits to content `X\r`.
                        if code.body.ends_with('\r') && code.tail_ws.is_empty() {
                            out.push('\r');
                        }
                        out.push('\n');
                    }
                }
                depth = depth.saturating_add(code.opens).saturating_sub(code.closes);
                wrote_any = true;
            }
            // The dropped EOF tail: verbatim (the official parser never parses it).
            NodeKind::DroppedTail => {
                flush_blank(&mut out, &mut blank_pending);
                out.push_str(cst.text(line));
                cursor += 1;
                wrote_any = true;
            }
            // Root children are always line nodes; any other kind is a CST bug
            // — emit it verbatim (tiling beats a panic) and flag it.
            other => {
                debug_assert!(false, "unexpected root child kind {other:?}");
                flush_blank(&mut out, &mut blank_pending);
                out.push_str(cst.text(line));
                cursor += 1;
                wrote_any = true;
            }
        }
    }
    debug_assert_eq!(cursor, logicals.len(), "logical lines not fully consumed");
    out
}

/// Emit one deferred blank line, if pending.
fn flush_blank(out: &mut String, pending: &mut bool) {
    if *pending {
        out.push('\n');
        *pending = false;
    }
}

/// Trailing-end trim semantics: strip chars `≤ ' '`.
fn trim_java_end(s: &str) -> &str {
    s.trim_end_matches(|c: char| c <= ' ')
}

/// Would re-printing `s` without its trailing whitespace open a line-join
/// (`"\\\\"` suffix) or text-block (`\"\"\"\"\` suffix) that the
/// original did not have? Then the trailing whitespace must be kept verbatim.
fn ends_in_magic(s: &str) -> bool {
    s.ends_with('\\') || s.ends_with("\"\"\"")
}

/// One re-printed code line (indent and line terminator added by the caller).
struct CodeLine<'a> {
    /// Items joined by single spaces (or the text-block raw region).
    body: String,
    /// Protective space for a `"""`-ending body (else empty).
    tail_ws: String,
    /// Scope tokens `{` seen (token values, quoted included — cosmetic depth).
    opens: usize,
    /// Scope tokens `}` seen.
    closes: usize,
    /// The line's first token is `}` → print one level out.
    dedent: bool,
    /// Append `\n` after the body (false when a raw region ended with one).
    needs_nl: bool,
    /// `\`-ending, unrepresentable body: the line's original bytes instead.
    verbatim: Option<&'a str>,
}

/// Re-print one logical code line: items (token chunks, text-block raw) joined
/// by single spaces; trivia (indent, continuation, newline, BOM) dropped.
fn code_line<'a>(cst: &'a Cst, line: &Node, logical: &Logical) -> CodeLine<'a> {
    let mut chunks: Vec<String> = Vec::new();
    let mut raw: Option<&str> = None;
    let mut trailing_ws = String::new();
    let mut prev: Option<NodeKind> = None;

    for leaf in &line.children {
        match leaf.kind {
            NodeKind::Whitespace => {
                // Whitespace inside a segment is real gap whitespace (it joins
                // into the logical line, mid-chain indents included). Only
                // between-segment gap whitespace after a terminator is the
                // joiner line's indent the official parser `stripLeading`s away — no
                // gap may be printed there, or a fused token would split.
                let start = leaf.range.start as usize;
                let end = leaf.range.end as usize;
                let in_segment = logical
                    .segments
                    .iter()
                    .any(|s| s.start <= start && end <= s.end);
                let joiner_indent = !in_segment
                    && !matches!(
                        prev,
                        Some(
                            NodeKind::TokenBare
                                | NodeKind::TokenQuoted
                                | NodeKind::Whitespace
                                | NodeKind::Comment
                        )
                    );
                if !chunks.is_empty() && !joiner_indent {
                    trailing_ws.push_str(cst.text(leaf));
                }
            }
            // BOM vanishes (the official parser strips it); continuation and newline
            // trivia join — no space is inserted across them, so a token split
            // by a `\`-continuation stays one token.
            NodeKind::TokenBare | NodeKind::TokenQuoted | NodeKind::Comment => {
                let text = cst.text(leaf);
                if chunks.is_empty() || !trailing_ws.is_empty() {
                    chunks.push(text.to_string());
                } else {
                    // No whitespace between: the same fold continued (a `\` join
                    // inside one token) — append, do not space.
                    chunks.last_mut().expect("non-empty").push_str(text);
                }
                trailing_ws.clear();
            }
            NodeKind::TextBlockRaw => raw = Some(cst.text(leaf)),
            // Every other leaf is trivia: BOM (the official parser strips it),
            // continuation and newline (the join leaves no trace), terminators.
            _ => {}
        }
        prev = Some(leaf.kind);
    }

    let mut opens = 0_usize;
    let mut closes = 0_usize;
    let mut dedent = false;
    let mut body = String::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let (text, brace) = render_chunk(chunk);
        if i > 0 {
            body.push(' ');
        }
        body.push_str(text);
        if let Some(open) = brace {
            if open {
                opens += 1;
            } else {
                closes += 1;
                if i == 0 {
                    dedent = true;
                }
            }
        }
    }

    let mut needs_nl = true;
    let mut tail_ws = String::new();
    let mut verbatim: Option<&str> = None;
    match raw {
        Some(r) => {
            // The raw region goes out byte-verbatim; the gap before it becomes
            // a single space only where the source had one.
            if !chunks.is_empty() && !trailing_ws.is_empty() {
                body.push(' ');
            }
            body.push_str(r);
            needs_nl = !r.ends_with('\n');
        }
        None => {
            if body.ends_with('\\') {
                // The value cannot be re-parsed from one physical line — the
                // the official parser would always re-join the trailing `\` (it only
                // survived in x because a comment-guarded joiner stopped the
                // chain). Print the logical line's ORIGINAL bytes verbatim:
                // the same join replays, reproducing x exactly.
                verbatim = Some(&cst.source[line.range.start as usize..line.range.end as usize]);
            } else if ends_in_magic(&body) {
                if ends_in_magic(&logical.joined(&cst.source)) {
                    // The source's OWN joined line ends with `"""` too (the
                    // magic was hidden behind a `\`-join, e.g. `\"""` + a
                    // whitespace joiner). Bare would open a text block the
                    // source never opened; a protective space invents a
                    // whitespace token the source did not have —
                    // UNREPRESENTABLE, so the original bytes replay verbatim.
                    verbatim =
                        Some(&cst.source[line.range.start as usize..line.range.end as usize]);
                } else {
                    // `"""` body end, magic NOT in the source line (a hiding
                    // trailing space was trimmed, or the token juxtaposition
                    // created it): one protective space keeps the text-block
                    // trigger silent (the last chunk is necessarily a bare
                    // token, so the space stays out of every value).
                    tail_ws = " ".to_string();
                }
            }
        }
    }
    CodeLine {
        body,
        tail_ws,
        opens,
        closes,
        dedent,
        needs_nl,
        verbatim,
    }
}

/// Render one token chunk: the text to print, plus `Some(true)`/`Some(false)`
/// when the token VALUE is `{`/`}` (the official tokenizer strips quotes
/// before its scope checks, so `"{"` opens a context too).
fn render_chunk(chunk: &str) -> (&str, Option<bool>) {
    let text = unquote_value(chunk).unwrap_or(chunk);
    let brace = match text {
        "{" => Some(true),
        "}" => Some(false),
        _ => None,
    };
    (text, brace)
}

/// The unquoted form of a quoted token chunk, if re-printing it bare is safe:
/// terminated by a real closing quote, non-empty, whitespace-free, no `"`
/// inside (the value keeps its escape backslashes verbatim, so a bare `"`
/// could not be re-escaped byte-exactly), not ending in `\` (it would eat the
/// closing quote on re-read), and not touching the official parser's line-level
/// patterns (`#`/`//` comment, `/* … */` region) from any position.
/// `${VAR}` is deliberately fine bare — substitution ignores quoting.
fn unquote_value(chunk: &str) -> Option<&str> {
    let inner = chunk
        .strip_prefix('"')?
        .strip_suffix('"')
        .filter(|inner| !inner.is_empty())?;
    let safe = !inner.contains('"')
        && !inner.ends_with('\\')
        && !inner.chars().any(compat::is_java_whitespace)
        && !inner.starts_with('#')
        && !inner.starts_with("//")
        && !inner.starts_with("/*")
        && !inner.ends_with("*/");
    safe.then_some(inner)
}

/// Re-indent a whole-line comment; its content is emitted verbatim.
fn comment_line(cst: &Cst, line: &Node, depth: usize, out: &mut String) {
    out.push_str(&INDENT.repeat(depth));
    let content = line
        .children
        .iter()
        .find(|l| matches!(l.kind, NodeKind::Comment))
        .map_or_else(
            || trim_java_end(cst.text(line)),
            |l| trim_java_end(cst.text(l)),
        );
    out.push_str(content);
    // The line-end guard mirrored into continuation-JOINED comments: the
    // trim can expose the last physical line's `\`, and bare that is a
    // continuation on re-parse (the parser's comment exemption is per
    // physical line — this line is not comment text on its own), swallowing
    // following lines. One protective space keeps the structure; a comment-
    // starting last line stays bare (the exemption already covers it).
    if content.ends_with('\\') && content.contains('\n') {
        let last = content.rsplit('\n').next().unwrap_or(content);
        let rest = last.trim_start_matches(compat::is_java_regex_s);
        if !(rest.starts_with("//") || rest.starts_with('#')) {
            out.push(' ');
        }
    }
    out.push('\n');
}

/// Re-print the opener line of a `/* … */` region as a code line (token
/// identity keeps the region machine's first-token / trim-based open-close
/// decisions); every following member line stays byte-verbatim (interior bytes
/// may be comment text whose content must not move, and the close check is
/// trim-based).
fn block_comment(cst: &Cst, line: &Node, logical: &Logical, depth: usize, out: &mut String) {
    let Some(first) = line.children.first() else {
        out.push_str(trim_java_end(cst.text(line)));
        out.push('\n');
        return;
    };
    debug_assert_eq!(
        (first.range.start as usize, first.range.end as usize),
        logical.span,
        "block-comment opener and logical line desynchronized"
    );
    let code = code_line(cst, first, logical);
    if let Some(text) = code.verbatim {
        out.push_str(text);
    } else {
        out.push_str(&INDENT.repeat(depth));
        if depth == 0 && code.body.starts_with('\u{FEFF}') {
            // The line-START BOM guard (see `format`): the opener re-prints as
            // a code line, so a BOM-led first token needs the same protective
            // space at column 0.
            out.push(' ');
        }
        out.push_str(&code.body);
        out.push_str(&code.tail_ws);
        if code.needs_nl {
            if code.body.ends_with('\r') && code.tail_ws.is_empty() {
                out.push('\r');
            }
            out.push('\n');
        }
    }
    if first.range.end < line.range.end {
        let rest = &cst.source[first.range.end as usize..line.range.end as usize];
        out.push_str(rest);
        if !rest.ends_with('\n') {
            out.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_indent_and_idempotence() {
        let src = "workspace \"W\" {\n        model {\n  people {\n   person p \"P\"\n  }\n }\n}\n";
        // `"W"`/`"P"` unquote safely (canonical style quotes only when needed).
        let want = "workspace W {\n    model {\n        people {\n            person p P\n        }\n    }\n}\n";
        let once = format(src);
        assert_eq!(once, want);
        assert_eq!(format(&once), once, "idempotent");
    }

    #[test]
    fn bom_led_token_gets_a_protective_space() {
        // fuzz find (dsl_fmt crash): a BOM AFTER the indent survives
        // `strip_bom` (which fires at the physical line's byte 0) and re-prints
        // as a token at column 0 — where the NEXT pass's strip eats it, so
        // `fmt` was not idempotent. The guard: one protective space, the
        // line-START mirror of the `\`/`"""` end guards.
        assert_eq!(format("    \u{FEFF}user = p\n"), " \u{FEFF}user = p\n");
        assert_eq!(
            format("    \u{FEFF}    user = p\n"),
            " \u{FEFF} user = p\n",
            "BOM as its own token keeps the single separator space"
        );
        let nested = format("workspace {\n \u{FEFF}user = p\n}\n");
        assert_eq!(format(&nested), nested, "depth > 0 needs no guard");
        // The file-leading BOM (byte 0 of line 1) still vanishes (strip_bom).
        assert_eq!(format("\u{FEFF}workspace {\n}\n"), "workspace {\n}\n");
    }

    #[test]
    fn brace_run_closing_line_dedents_once() {
        let src = "a {\n  b {\n    c\n  } }\n";
        assert_eq!(format(src), "a {\n    b {\n        c\n    } }\n");
    }

    #[test]
    fn blank_runs_compact_and_fences_trim() {
        let src = "\n\n\na\n\n\n\nb\n\n";
        assert_eq!(format(src), "a\n\nb\n");
    }

    #[test]
    fn comment_lines_reindent_content_verbatim() {
        let src = "// top\na {\n        # note   \n}\n";
        assert_eq!(format(src), "// top\na {\n    # note\n}\n");
    }

    #[test]
    fn block_comment_interior_verbatim_opener_reindented() {
        let src = "a {\n      /* open\n   interior */\n  b\n}\n";
        let once = format(src);
        assert_eq!(once, "a {\n    /* open\n   interior */\n    b\n}\n");
        assert_eq!(format(&once), once);
    }

    #[test]
    fn whole_line_block_comment_reindents() {
        let src = "a {\n      /* one-liner */\n}\n";
        assert_eq!(format(src), "a {\n    /* one-liner */\n}\n");
    }

    #[test]
    fn conservative_unquoting() {
        // only `name "foo"` unquotes: whitespace, escapes, empties, and
        // comment/region openers must keep their quotes.
        let src = "name \"foo\"\ndesc \"a b\"\nq \"a\\\"b\"\nnotcomment \"#x\"\nnotregion \"/*x\"\nempty \"\"\ncr \"a\\\\\"\n";
        let once = format(src);
        assert_eq!(
            once,
            "name foo\ndesc \"a b\"\nq \"a\\\"b\"\nnotcomment \"#x\"\nnotregion \"/*x\"\nempty \"\"\ncr \"a\\\\\"\n"
        );
        assert_eq!(format(&once), once);
    }

    #[test]
    fn continuations_join_without_traces() {
        // The official join strips the continuation line's indent, so both
        // tokens fuse — and the fused values are safe bare.
        let src = "name foo\\\nbar\ndesc \"a\\\n  b\"\n";
        let once = format(src);
        assert_eq!(once, "name foobar\ndesc ab\n");
        assert_eq!(format(&once), once);
    }

    #[test]
    fn text_block_region_verbatim() {
        let src = "a {\n        description \"\"\"\n  Line one\n    indented\n\"\"\"\n}\n";
        let once = format(src);
        assert_eq!(
            once,
            "a {\n    description \"\"\"\n  Line one\n    indented\n\"\"\"\n}\n"
        );
        assert_eq!(format(&once), once);
    }

    #[test]
    fn magic_ending_lines_guarded() {
        // `a""" ` must NOT become a text-block opener → protective space.
        let src = "desc a\"\"\"  \n";
        let once = format(src);
        assert_eq!(once, "desc a\"\"\" \n");
        assert_eq!(format(&once), once);
    }

    #[test]
    fn backslash_ending_body_prints_verbatim() {
        // `b\ ` survived x only because trailing spaces hid the endsWith;
        // re-printing `b\` bare would join. Verbatim bytes replay exactly.
        let src = "next b\\  \nlast\n";
        assert_eq!(format(src), src);
        // Comment-guarded join: the joined value ends with `\` — unprintable
        // on one line; the whole logical line goes out as it came in.
        let src2 = "\"\\\n#\\\n";
        assert_eq!(format(src2), src2);
    }

    #[test]
    fn crlf_normalizes_to_lf() {
        let src = "a {\r\n    b\r\n}\r\n";
        assert_eq!(format(src), "a {\n    b\n}\n");
    }

    #[test]
    fn dropped_tail_verbatim() {
        let src = "a\n\nfoo\\";
        assert_eq!(format(src), "a\n\nfoo\\");
    }

    #[test]
    fn backslash_joined_comment_line() {
        // `\` + `# c`: the official parser joins to a comment line (the `\`-line's
        // segment is empty) — a lex_comment tiling regression (caught by proptest).
        assert_eq!(format("\\\n# c"), "# c\n");
        assert_eq!(format("   \\\n\t# c\n"), "# c\n");
    }

    #[test]
    fn mid_chain_continuation_indent_is_a_gap() {
        // Only the JOINER line is `stripLeading`ed — a middle `\`-line keeps
        // its indent, so `b` stays a separate token from `a`, while `c` fuses
        // onto `b` (joiner stripped).
        let src = "a\\\n    b\\\nc\n";
        assert_eq!(format(src), "a bc\n");
        let src2 = "a\\\n  b\\\n  c\n";
        assert_eq!(format(src2), "a bc\n");
    }

    #[test]
    fn empty_input_formats_empty() {
        assert_eq!(format(""), "");
        assert_eq!(format("\n \n\t\n"), "");
    }

    #[test]
    fn quoted_braces_count_depth_and_unquote() {
        // The official parser strips quotes before its scope checks — `"{"` opens a
        // context there too, so the formatter indents after it likewise.
        let src = "a \"{\" \nb\n\"}\"\n";
        assert_eq!(format(src), "a {\n    b\n}\n");
    }
}
