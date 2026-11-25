//! Stage 2 of the DSL pipeline: the bug-compatible tokenizer, lifted to carry
//! physical byte spans and trivia so the result feeds the lossless CST.
//!
//! The token list is defined over the TRIMMED joined logical line (`trim`
//! cannot change it: leading/trailing whitespace is skipped or terminates the
//! open token anyway). We instead walk the untrimmed physical bytes of the
//! logical line, segment by segment, emitting whitespace / `\` / newline /
//! BOM bytes as trivia leaves and token-character runs as token leaves.
//! Tokenizer state (`in_token`, `quoted`, prev-char lookbehind) carries
//! across segment boundaries because those trivia bytes do not exist in the
//! joined text.
//!
//! Replicated quirks, deliberately:
//!
//! * Quote characters are STRIPPED from quoted token values; a `"` is "escaped" iff
//!   the PREVIOUS character of the joined text is `\` (raw lookbehind — the backslash
//!   itself stays in the value, and a bare token starting `a\` before a continuation
//!   escapes a quote on the NEXT physical line).
//! * `"` is only special at token start (opens quoted) or inside a quoted token
//!   (closes); inside a bare token it is an ordinary character (`a"b` is one token).
//! * End of line with a token open pushes it: an unterminated quoted token silently
//!   loses its opening `"` and produces no error at this level.
//! * Whitespace is the ASCII set ` \t\n\x0B\f\r` (`compat::is_java_whitespace`);
//!   Unicode divergences are documented there.

use oxidizr_ast::{NodeKind, TextRange, u32_len};

use crate::compat;
use crate::preprocess::Logical;

/// A trivia or token leaf produced by the lexer (byte range into the source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Leaf {
    /// Leaf taxonomy kind.
    pub kind: NodeKind,
    /// Byte span in the source.
    pub range: TextRange,
}

/// One semantic token: the value is the concatenation of `pieces` (quotes excluded);
/// the leaf range additionally covers the quote delimiters. Pieces can sit on
/// different physical lines when a `\`-continuation splits the token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// Whether the token started with `"` (affects the value rules only — the
    /// lookbehind quirk applies to both kinds).
    pub quoted: bool,
    /// Byte ranges contributing to the value, in order.
    pub pieces: Vec<TextRange>,
}

impl Token {
    /// The token value: its pieces concatenated, quote delimiters excluded.
    #[must_use]
    pub fn value(&self, source: &str) -> String {
        self.pieces
            .iter()
            .map(|p| &source[p.start as usize..p.end as usize])
            .collect()
    }
}

/// A lexed logical line: flat leaves tiling the line's full byte span (in source
/// order, so they also tile within the [`crate::cst`] tree) plus the semantic tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LexedLine {
    /// Leaves covering `logical.span` exactly.
    pub leaves: Vec<Leaf>,
    /// Tokens in order (≥1 for code lines).
    pub tokens: Vec<Token>,
}

/// Leaf-emission accumulator merging adjacent same-kind trivia.
struct LeafSink {
    leaves: Vec<Leaf>,
}

impl LeafSink {
    fn push(&mut self, kind: NodeKind, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let range = TextRange::new(u32_len(start), u32_len(end));
        if let Some(last) = self.leaves.last_mut()
            && last.kind == kind
            && last.range.end == range.start
        {
            last.range.end = range.end;
        } else {
            self.leaves.push(Leaf { kind, range });
        }
    }
}

/// Tokenizer state carried across segments (the joined text's left-fold).
struct FoldState {
    in_token: bool,
    quoted: bool,
    /// Previous character of the JOINED text. Trivia
    /// bytes (continuations, newlines, stripped indent) never update it.
    prev: Option<char>,
    /// Byte where the current leaf part started (includes an opening quote).
    part_start: usize,
    /// Byte where the current value piece started, or `None` between pieces.
    piece_start: Option<usize>,
    /// Value pieces of the token under construction.
    pieces: Vec<TextRange>,
}

impl FoldState {
    fn new() -> FoldState {
        FoldState {
            in_token: false,
            quoted: false,
            prev: None,
            part_start: 0,
            piece_start: None,
            pieces: Vec::new(),
        }
    }

    /// Close the open value piece at `end` (zero-length pieces are dropped).
    fn close_piece(&mut self, end: usize) {
        if let Some(start) = self.piece_start.take()
            && start < end
        {
            self.pieces
                .push(TextRange::new(u32_len(start), u32_len(end)));
        }
    }

    /// Complete the token under construction. A closing quote, a whitespace
    /// boundary, or end of line pushes whatever accumulated — even the
    /// empty string for `""`.
    fn finish_token(&mut self, tokens: &mut Vec<Token>) {
        tokens.push(Token {
            quoted: self.quoted,
            pieces: std::mem::take(&mut self.pieces),
        });
        self.in_token = false;
        self.quoted = false;
    }
}

/// Classify bytes the tokenizer never sees (BOM, `\`, line terminators, indent) into
/// trivia leaves. Whitespace uses the tokenizer's set; anything else that is not one
/// of the known trivia bytes indicates a segmentation bug — it is reported as
/// [`NodeKind::Whitespace`] in release builds (tiling beats a panic) and asserted in
/// debug.
fn trivia_leaves(source: &str, start: usize, end: usize, sink: &mut LeafSink) {
    let mut run_kind = None::<NodeKind>;
    let mut run_start = start;
    for (rel, ch) in source[start..end].char_indices() {
        let i = start + rel;
        let kind = if ch == '\u{FEFF}' {
            NodeKind::Bom
        } else if ch == '\\' {
            NodeKind::Continuation
        } else if ch == '\n' || ch == '\r' {
            NodeKind::Newline
        } else if compat::is_java_whitespace(ch) {
            NodeKind::Whitespace
        } else {
            debug_assert!(false, "unexpected trivia byte {ch:?} at {i}");
            NodeKind::Whitespace
        };
        if run_kind != Some(kind) {
            if let Some(k) = run_kind {
                sink.push(k, run_start, i);
            }
            run_kind = Some(kind);
            run_start = i;
        }
    }
    if let Some(k) = run_kind {
        sink.push(k, run_start, end);
    }
}

/// Lex one logical line's physical span: tokens with byte-accurate pieces, trivia
/// leaves tiling every byte of `logical.span` (including the closing line
/// terminator(s)), and a text-block line's raw region as one [`NodeKind::TextBlockRaw`]
/// leaf.
#[must_use]
pub fn lex_line(source: &str, logical: &Logical) -> LexedLine {
    let mut sink = LeafSink { leaves: Vec::new() };
    let mut tokens: Vec<Token> = Vec::new();
    let mut st = FoldState::new();
    let mut cursor = logical.span.0;

    // Tokenizing joins the segments; the fold state carries across the gap
    // bytes, which are emitted as trivia instead.
    for seg in &logical.segments {
        trivia_leaves(source, cursor, seg.start, &mut sink);
        cursor = seg.end;
        if st.in_token {
            // A token open at the previous segment end continues here (the `\` join):
            // its value piece and leaf part resume at the segment start (the gap
            // bytes are not part of the joined text).
            st.part_start = seg.start;
            st.piece_start = Some(seg.start);
        }
        let seg_text = &source[seg.start..seg.end];
        let mut seg_pos = seg.start;
        for ch in seg_text.chars() {
            let i = seg_pos;
            let w = ch.len_utf8();
            seg_pos += w;
            if !st.in_token {
                if ch == '"' {
                    st.in_token = true;
                    st.quoted = true;
                    st.part_start = i;
                    st.piece_start = Some(i + w); // opening quote excluded
                    st.prev = Some(ch);
                } else if compat::is_java_whitespace(ch) {
                    sink.push(NodeKind::Whitespace, i, i + w);
                    st.prev = Some(ch);
                } else {
                    st.in_token = true;
                    st.quoted = false;
                    st.part_start = i;
                    st.piece_start = Some(i);
                    st.prev = Some(ch);
                }
            } else if ch == '"' && st.prev == Some('\\') {
                // Escaped quote: the `"` stays in the value (the backslash was
                // already covered by the open piece).
                st.prev = Some(ch);
            } else if st.quoted && ch == '"' {
                // End of a quoted token: closing quote excluded from the value.
                st.close_piece(i);
                sink.push(NodeKind::TokenQuoted, st.part_start, i + w);
                st.finish_token(&mut tokens);
                st.prev = Some(ch);
            } else if !st.quoted && compat::is_java_whitespace(ch) {
                // End of a bare token; this whitespace starts a trivia run.
                st.close_piece(i);
                sink.push(NodeKind::TokenBare, st.part_start, i);
                st.finish_token(&mut tokens);
                sink.push(NodeKind::Whitespace, i, i + w);
                st.prev = Some(ch);
            } else {
                st.prev = Some(ch);
                // Ordinary value character: covered by the open piece.
            }
        }
        // Segment boundary: close the running piece and leaf part; an open token
        // continues on the next segment (continuation).
        st.close_piece(seg.end);
        if st.in_token {
            let kind = if st.quoted {
                NodeKind::TokenQuoted
            } else {
                NodeKind::TokenBare
            };
            sink.push(kind, st.part_start, seg.end);
        }
    }

    // An open token is pushed at end of line; unterminated quoted tokens
    // lose their opening quote silently (deliberate bug-compat).
    if st.in_token {
        st.finish_token(&mut tokens);
    }

    match &logical.text_block {
        Some(tb) => {
            // Trailing prefix trivia, then the raw `""" … """` region through the
            // closing line's terminator.
            trivia_leaves(source, cursor, tb.raw.0, &mut sink);
            sink.push(NodeKind::TextBlockRaw, tb.raw.0, tb.raw.1);
        }
        None => trivia_leaves(source, cursor, logical.span.1, &mut sink),
    }

    LexedLine {
        leaves: sink.leaves,
        tokens,
    }
}

/// Lex a whole-line `//`/`#` comment: leading trivia, one [`NodeKind::Comment`] leaf
/// from the comment opener through the content end, trailing line-terminator trivia.
#[must_use]
pub(crate) fn lex_comment(source: &str, logical: &Logical) -> LexedLine {
    let mut sink = LeafSink { leaves: Vec::new() };
    // The comment opener is the first non-`\s` char of the JOINED text — a
    // `\`-join can leave leading all-whitespace segments (e.g. `\` + `# c`,
    // where the pending segment is empty and the joiner carries the comment).
    // The content ends at the last non-empty segment (terminators excluded).
    let mut content_start = None;
    let mut content_end = None;
    for s in &logical.segments {
        if content_start.is_none() {
            let text = &source[s.start..s.end];
            if let Some(off) = text.find(|c: char| !compat::is_java_regex_s(c)) {
                content_start = Some(s.start + off);
            }
        }
        if s.end > s.start {
            content_end = Some(s.end);
        }
    }
    let content_start = content_start.unwrap_or(logical.span.0);
    let content_end = content_end.unwrap_or(logical.span.0);
    trivia_leaves(source, logical.span.0, content_start, &mut sink);
    sink.push(NodeKind::Comment, content_start, content_end);
    trivia_leaves(source, content_end, logical.span.1, &mut sink);
    LexedLine {
        leaves: sink.leaves,
        tokens: Vec::new(),
    }
}

/// Lex a blank logical line (everything is trivia).
#[must_use]
pub(crate) fn lex_blank(source: &str, logical: &Logical) -> LexedLine {
    let mut sink = LeafSink { leaves: Vec::new() };
    trivia_leaves(source, logical.span.0, logical.span.1, &mut sink);
    LexedLine {
        leaves: sink.leaves,
        tokens: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preprocess::preprocess;

    #[test]
    fn comment_after_all_ws_continuation_segments() {
        // Regression: a `\`-join whose pending segments are
        // whitespace-only — the comment opener lives in a LATER segment, and
        // the leaf tiling must survive (used to start the Comment leaf at the
        // empty first segment and feed `#` to trivia_leaves).
        for src in ["\\\n# c", "   \\\n\t# c"] {
            let ls = preprocess(src);
            assert_eq!(ls.len(), 1, "{src:?}");
            let ll = lex_comment(src, &ls[0]);
            let kinds: Vec<NodeKind> = ll.leaves.iter().map(|l| l.kind).collect();
            assert!(kinds.contains(&NodeKind::Comment), "{src:?}: {kinds:?}");
            // Leaves tile the logical span contiguously in order.
            let mut cursor = ls[0].span.0;
            for leaf in &ll.leaves {
                assert_eq!(leaf.range.start as usize, cursor, "{src:?}: {kinds:?}");
                cursor = leaf.range.end as usize;
            }
            assert_eq!(cursor, ls[0].span.1, "{src:?}");
        }
    }

    /// Lex the single logical line of a fixture source (panics if not exactly one).
    fn one(src: &str) -> LexedLine {
        let ls = preprocess(src);
        assert_eq!(
            ls.len(),
            1,
            "fixture must produce one logical line: {src:?}"
        );
        lex_line(src, &ls[0])
    }

    fn vals(src: &str) -> Vec<String> {
        let l = one(src);
        l.tokens.iter().map(|t| t.value(src)).collect()
    }

    fn kinds(src: &str) -> Vec<(NodeKind, String)> {
        let l = one(src);
        l.leaves
            .iter()
            .map(|leaf| {
                (
                    leaf.kind,
                    src[leaf.range.start as usize..leaf.range.end as usize].to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn basic_tokens_and_whitespace_leaves() {
        assert_eq!(vals("  a b \"c d\"  "), ["a", "b", "c d"]);
        assert_eq!(
            kinds("  a b"),
            vec![
                (NodeKind::Whitespace, "  ".into()),
                (NodeKind::TokenBare, "a".into()),
                (NodeKind::Whitespace, " ".into()),
                (NodeKind::TokenBare, "b".into()),
            ]
        );
    }

    #[test]
    fn quotes_stripped_escapes_kept() {
        // `"` starts a quoted token (quotes stripped); `\"` keeps BOTH chars.
        assert_eq!(vals("\"hello\""), ["hello"]);
        assert_eq!(vals("\"a\\\"b\""), ["a\\\"b"]);
        assert_eq!(vals("\"\""), [""]);
    }

    #[test]
    fn quote_inside_bare_token_is_ordinary() {
        assert_eq!(vals("a\"b"), ["a\"b"]);
        // A `"` after non-backslash inside a quoted token CLOSES it; the rest starts
        // a bare token: `a` quoted, `b` bare (one leaf each, adjacent ranges).
        let l = one("\"a\"b");
        assert_eq!(l.tokens[0].value("\"a\"b"), "a");
        assert_eq!(l.tokens[1].value("\"a\"b"), "b");
        assert!(!l.tokens[1].quoted);
    }

    #[test]
    fn unterminated_quote_pushes_silently() {
        assert_eq!(vals("\"oops"), ["oops"]);
        let l = one("\"oops");
        assert!(
            l.tokens[0].quoted,
            "opening quote marked quoted, then dropped"
        );
    }

    #[test]
    fn continuation_splits_token_with_lookbehind_across_boundary() {
        // `foo\` + `"bar` joins to `foo"bar` (bare token; the `\` itself is gone from
        // the joined text, so NO escaping applies).
        assert_eq!(vals("foo\\\n\"bar\n"), ["foo\"bar"]);
        // But literal backslashes at the end of the CONTENT (source `foo\\` +
        // continuation `\`) join to `foo\\"bar`: the second `\` escapes the quote,
        // and BOTH backslashes stay in the value (backslash-keep quirk).
        assert_eq!(vals("foo\\\\\\\n\"bar\n"), [r#"foo\\"bar"#]);
    }

    #[test]
    fn leaves_tile_the_line_span() {
        for src in [
            "  a b \"c d\"  \n",
            "x\\\n    y\\\n    z\n",
            "#!/no  \n",
            "a\t\tb\r\n",
            "tail-no-newline",
            "\u{FEFF}workspace {\n",
        ] {
            let ls = preprocess(src);
            let mut cursor = 0_usize;
            for l in &ls {
                let lex = lex_line(src, l);
                for leaf in &lex.leaves {
                    assert_eq!(leaf.range.start as usize, cursor, "tiling {src:?}");
                    cursor = leaf.range.end as usize;
                }
                assert_eq!(cursor, l.span.1, "line cover {src:?}");
                cursor = l.span.1;
            }
            assert_eq!(cursor, src.len(), "full cover {src:?}");
        }
    }

    #[test]
    fn hash_and_slash_midline_stay_tokens() {
        // The tokenizer knows nothing about comments: `#ffffff` and `// x` tokenize.
        assert_eq!(
            vals("style #ffffff // trailing"),
            ["style", "#ffffff", "//", "trailing"]
        );
    }

    #[test]
    fn text_block_line_lexes_prefix_plus_raw() {
        let src = "p q \"\"\"\n  body\n  \"\"\"\n";
        let ls = preprocess(src);
        let lex = lex_line(src, &ls[0]);
        assert_eq!(lex.tokens.len(), 2);
        let last = lex.leaves.last().expect("raw leaf");
        assert_eq!(last.kind, NodeKind::TextBlockRaw);
        assert_eq!(
            &src[last.range.start as usize..last.range.end as usize],
            "\"\"\"\n  body\n  \"\"\"\n"
        );
    }

    #[test]
    fn tab_and_control_trim_boundaries() {
        // Tabs separate tokens (both tab and form feed are whitespace).
        assert_eq!(vals("a\u{C}b"), ["a", "b"]);
        // Vertical tab, form feed:
        assert_eq!(vals("a\u{B}\u{C}b"), ["a", "b"]);
    }
}
