//! Stage 1 of the DSL pipeline: split the source into "logical
//! lines" — the exact grouping the official parser performs before
//! parsing, keeping byte spans into the ORIGINAL source so tokens can carry
//! faithful spans and diagnostics can quote original lines.
//!
//! Official-format facts this mirrors verbatim:
//!
//! * BOM: the parse loop strips a leading `\u{FEFF}` from each logical line's start
//!   (reachable on the first line of a file / of an included file).
//! * Continuation: a line ENDING WITH `\` (raw check, CRLF-insensitive, whole-line
//!   `//`/`#` comment lines exempt) loses the backslash; the next line is appended
//!   with its leading whitespace stripped. Nothing is inserted between the parts.
//! * Text blocks: a line ending with `"""` opens a block closed by the next line that
//!   ENDS with `"""`; middle lines are dedented by the first middle line's leading
//!   whitespace count, joined with `\n`; the official parser folds the logical line into
//!   `prefix + "\"${name}\""`, i.e. the prefix is everything before the FIRST `"""`
//!   in the joined opener text. The closing line's content before its marker is
//!   DISCARDED and a `"""` inside the value TRUNCATES it (the official
//!   split behavior) — [`text_block_value`] replicates both.
//! * Dropped tails: at EOF an unfinished continuation or unterminated text block is
//!   SILENTLY DROPPED by the official parser (its buffer only flushes on a complete line);
//!   we mark such a tail [`Logical::dropped`] so the CST can still cover the bytes
//!   while the parser ignores it.
//! * Line terminators: the official reader consumes `\n`, `\r\n`, `\r` — here
//!   lines split on `\n` with trailing `\r` stripped from content; a lone trailing
//!   `\r` at EOF is likewise invisible to the logic but kept in the byte spans.

use crate::compat;

/// The text-block opener/closer marker.
const TEXT_BLOCK_MARKER: &str = "\"\"\"";

/// A physical line with its byte spans (offsets into the whole source).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalLine {
    /// Byte offset of the first character.
    pub start: usize,
    /// Content end excluding the line terminator (and trailing `\r`).
    pub content_end: usize,
    /// End including the line terminator (`\n`, `\r\n`, or EOF).
    pub end: usize,
}

/// One physical line's byte span participating in a logical line's joined text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// Content actually joined (backslash/`\r`/stripped indent excluded).
    pub start: usize,
    /// Exclusive end.
    pub end: usize,
}

impl Segment {
    /// The joined bytes of this segment.
    #[must_use]
    pub fn text<'a>(&self, source: &'a str) -> &'a str {
        &source[self.start..self.end]
    }
}

/// Text-block region folded onto a logical line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextBlock {
    /// From the opening `"""` through the closing line's end (incl. its line
    /// terminator); unterminated blocks run to EOF.
    pub raw: (usize, usize),
    /// Text on the opener line AFTER the opening `"""` (the official buffer holds
    /// the whole opener line; the marker split makes this the start of the
    /// value). Normally empty.
    pub after_opener: (usize, usize),
    /// Start of the middle content region (first byte after the opener line).
    pub middle_start: usize,
    /// End of the middle region (start of the closing line; EOF if unterminated).
    pub middle_end: usize,
    /// Whether a closing line ended the block.
    pub closed: bool,
}

/// Compute the official parser text-block value (drop the last character of
/// the reconstructed buffer).
///
/// The value body = text after the opening `"""` on the opener line, then each
/// middle line dedented by the FIRST middle line's leading-whitespace count and
/// followed by `\n` (a zero-length middle line contributes just `\n`). The body is
/// then cut at the first `"""` occurring in it (the split keeps only that part)
/// and its LAST character is dropped — normally the
/// trailing `\n`).
///
/// Inputs where the official parser would crash (index-out-of-bounds
/// exceptions) return `None` so callers emit a diagnostic instead
/// of crashing (a documented deviation).
#[must_use]
pub fn text_block_value(source: &str, block: &TextBlock) -> Option<String> {
    let mut body = source[block.after_opener.0..block.after_opener.1].to_string();
    let middle = &source[block.middle_start..block.middle_end];
    let lines: Vec<&str> = middle
        .split_inclusive('\n')
        .map(strip_line_terminator)
        .collect();
    let dedent = lines.first().map_or(0, |first| {
        first
            .chars()
            .take_while(|c| compat::is_java_whitespace(*c))
            .count()
    });
    for line in &lines {
        if line.is_empty() {
            body.push('\n');
        } else {
            let stripped = compat::strip_prefix_chars(line, dedent)?;
            body.push_str(stripped);
            body.push('\n');
        }
    }
    if let Some(pos) = body.find("\"\"\"") {
        body.truncate(pos);
    }
    if body.is_empty() {
        return None; // reference: substring(0, -1)
    }
    body.pop();
    Some(body)
}

fn strip_line_terminator(line: &str) -> &str {
    strip_cr(line.strip_suffix('\n').unwrap_or(line))
}

fn strip_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// A logical line: one statement unit as the official parse loop sees it.
#[derive(Debug, Clone)]
pub struct Logical {
    /// 1-based number of the physical line at which the official parser EMITTED this
    /// logical line — the LAST line of a continuation / text-block group (the parse
    /// loop flushes with its already-advanced `lineNumber`).
    pub number: u32,
    /// Physical span including all covered line terminators — consecutive logical
    /// lines tile the source exactly.
    pub span: (usize, usize),
    /// Segments in join order (≥1; more than one iff continuations were folded).
    pub segments: Vec<Segment>,
    /// Text block folded onto this line, if any.
    pub text_block: Option<TextBlock>,
    /// True for the silently-dropped EOF tail (unfinished continuation or
    /// unterminated text block). The official parser never parses these bytes.
    pub dropped: bool,
}

impl Logical {
    /// The joined logical text the official tokenizer would see.
    #[must_use]
    pub fn joined(&self, source: &str) -> String {
        let mut out = String::new();
        for seg in &self.segments {
            out.push_str(seg.text(source));
        }
        out
    }

    /// The blank-line test (`^\s*$`, ASCII `\s`): the logical line is
    /// blank.
    #[must_use]
    pub fn is_empty_line(&self, source: &str) -> bool {
        !self
            .joined(source)
            .chars()
            .any(|c| !compat::is_java_regex_s(c))
    }

    /// The whole-line-comment test (`^\s*(//|#).*$`): whole-line comment.
    #[must_use]
    pub fn is_comment_line(&self, source: &str) -> bool {
        let text = self.joined(source);
        let rest = text.trim_start_matches(compat::is_java_regex_s);
        rest.starts_with("//") || rest.starts_with('#')
    }
}

/// Open text-block accumulation state.
struct TbOpen {
    /// Physical start of the whole logical line.
    span_start: usize,
    /// Segments before the first `"""` (join of any pending continuations + prefix).
    prefix: Vec<Segment>,
    /// Byte offset of the opening `"""`.
    raw_start: usize,
    /// Text on the opener line after the opening marker (the value start;
    /// see the pathological-cross-segment note in the opener branch).
    after_opener: (usize, usize),
    /// First byte after the opener line.
    middle_start: usize,
}

/// Preprocess the whole (decoded, UTF-8) source into logical lines.
///
/// Begin a text block on this line. Computes the byte position of the FIRST
/// `"""` in the reconstructed join (`pending` text + current line) — the boundary
/// the official parser's marker split puts at the prefix's end — drains
/// `pending` into the trimmed prefix segments, and returns the open state.
///
/// The marker straddling a continuation boundary is a pathological, unmodeled
/// corner (debug-asserted; release falls back to the naive mapping).
fn open_text_block(
    source: &str,
    pending: &mut Vec<Segment>,
    pending_start: usize,
    line: &PhysicalLine,
) -> TbOpen {
    let content = (line.start, line.content_end);
    let content_text = &source[content.0..content.1];
    let mut joined_len = 0_usize;
    let mut acc = String::new();
    let mut rel = None;
    for seg in pending.iter() {
        acc.push_str(seg.text(source));
        joined_len += seg.end - seg.start;
        if rel.is_none() {
            rel = acc.find("\"\"\"");
        }
    }
    acc.push_str(content_text);
    // The line ends with the marker, so a hit is guaranteed; the fallback is the
    // trailing marker itself.
    let rel = rel
        .or(acc.find("\"\"\""))
        .unwrap_or(joined_len + content_text.len() - TEXT_BLOCK_MARKER.len());
    let mut raw_start = content.0 + rel.saturating_sub(joined_len);
    if rel < joined_len {
        let mut cum = 0_usize;
        for seg in pending.iter() {
            let len = seg.end - seg.start;
            if rel < cum + len {
                raw_start = seg.start + (rel - cum);
                break;
            }
            cum += len;
        }
        debug_assert!(false, "text-block marker inside a pending continuation");
    }
    let span_start = if pending.is_empty() {
        line.start
    } else {
        pending_start
    };
    let mut prefix: Vec<Segment> = std::mem::take(pending);
    // Trim prefix so it ends exactly at `raw_start`.
    trim_segments_to(&mut prefix, raw_start);
    prefix.retain(|s| s.start < s.end);
    if raw_start > content.0 {
        // Opener-line text before the first `"""` (indent included).
        prefix.push(Segment {
            start: content.0,
            end: raw_start,
        });
    }
    TbOpen {
        span_start,
        prefix,
        raw_start,
        after_opener: (raw_start + TEXT_BLOCK_MARKER.len(), content.1),
        middle_start: line.end,
    }
}

/// Finish a logical line at this physical line: joins any accumulated
/// continuation (`pending`) with this line — the final
/// joiner line is `stripLeading`ed — and returns the logical line emitted under
/// this line's number.
fn complete_line(
    source: &str,
    pending: &mut Vec<Segment>,
    pending_start: usize,
    line: &PhysicalLine,
    number: u32,
) -> Logical {
    let content = (line.start, line.content_end);
    let mut segments = std::mem::take(pending);
    let start = if segments.is_empty() {
        line.start
    } else {
        pending_start
    };
    let mut s = content.0;
    if !segments.is_empty() {
        // Joiner lines are left-trimmed (the official parser strips leading whitespace).
        let text = &source[s..content.1];
        let stripped = text.trim_start_matches(compat::is_java_whitespace);
        s += text.len() - stripped.len();
    }
    segments.push(Segment {
        start: s,
        end: content.1,
    });
    let mut logical = Logical {
        number,
        span: (start, line.end),
        segments,
        text_block: None,
        dropped: false,
    };
    strip_bom(&mut logical, source);
    logical
}

/// Byte coverage: consecutive logical lines tile the source exactly (each includes
/// its trailing line terminator), which is what the lossless CST builds on.
#[must_use]
pub fn preprocess(source: &str) -> Vec<Logical> {
    let mut out: Vec<Logical> = Vec::new();
    let lines = physical_lines(source);

    // Continuation accumulation (the official parser's pending buffer).
    let mut pending: Vec<Segment> = Vec::new();
    let mut pending_start = 0_usize;
    let mut tb: Option<TbOpen> = None;

    for (i, line) in lines.iter().enumerate() {
        let number = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let content = (line.start, line.content_end);
        let content_text = &source[content.0..content.1];

        if tb.is_some() {
            // Inside a text block: only the closing check runs (the official text-block
            // branch); middle lines are captured by span, not per-line segments.
            if content_text.ends_with(TEXT_BLOCK_MARKER) {
                if let Some(open) = tb.take() {
                    let mut logical = Logical {
                        number, // reference flushes with the closing line's number
                        span: (open.span_start, line.end),
                        segments: open.prefix,
                        text_block: Some(TextBlock {
                            raw: (open.raw_start, line.end),
                            after_opener: open.after_opener,
                            middle_start: open.middle_start,
                            middle_end: content.0,
                            closed: true,
                        }),
                        dropped: false,
                    };
                    strip_bom(&mut logical, source);
                    out.push(logical);
                }
            }
            continue;
        }

        let is_comment = {
            let rest = content_text.trim_start_matches(compat::is_java_regex_s);
            rest.starts_with("//") || rest.starts_with('#')
        };

        if !is_comment && content_text.ends_with('\\') {
            // Continuation: drop the trailing backslash and keep joining (middle
            // lines keep their indentation — the official parser only `stripLeading`s the
            // final completing line).
            if pending.is_empty() {
                pending_start = line.start;
            }
            pending.push(Segment {
                start: content.0,
                end: content.1 - 1,
            });
            continue;
        }

        if !is_comment && content_text.ends_with(TEXT_BLOCK_MARKER) {
            tb = Some(open_text_block(source, &mut pending, pending_start, line));
            continue;
        }

        // Complete line (first or joiner).
        out.push(complete_line(
            source,
            &mut pending,
            pending_start,
            line,
            number,
        ));
    }

    // Dropped tails (the official parser discards the incomplete buffer at EOF).
    let last_line = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    if let Some(open) = tb {
        let mut logical = Logical {
            number: last_line,
            span: (open.span_start, source.len()),
            segments: open.prefix,
            text_block: Some(TextBlock {
                raw: (open.raw_start, source.len()),
                after_opener: open.after_opener,
                middle_start: open.middle_start,
                middle_end: source.len(),
                closed: false,
            }),
            dropped: true,
        };
        strip_bom(&mut logical, source);
        out.push(logical);
    } else if !pending.is_empty() {
        let mut logical = Logical {
            number: last_line,
            span: (pending_start, source.len()),
            segments: pending,
            text_block: None,
            dropped: true,
        };
        strip_bom(&mut logical, source);
        out.push(logical);
    }

    out
}

/// The parse loop strips a leading BOM from the logical line's joined text.
fn strip_bom(logical: &mut Logical, source: &str) {
    if let Some(first) = logical.segments.first_mut()
        && source[first.start..first.end].starts_with('\u{FEFF}')
    {
        first.start += '\u{FEFF}'.len_utf8();
    }
}

/// Cut `segments` so their joined text ends exactly at absolute byte `end`.
fn trim_segments_to(segments: &mut [Segment], end: usize) {
    for seg in segments.iter_mut() {
        if seg.start >= end {
            seg.end = seg.start;
        } else if seg.end > end {
            seg.end = end;
        }
    }
}

/// Split into physical lines on `\n`; content drops a trailing `\r` (the official
/// reader's behavior); terminator bytes stay inside `start..end` spans.
#[must_use]
pub fn physical_lines(source: &str) -> Vec<PhysicalLine> {
    let mut out = Vec::new();
    let bytes = source.as_bytes();
    let mut start = 0_usize;
    for (pos, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            let content_end = if pos > start && bytes[pos - 1] == b'\r' {
                pos - 1
            } else {
                pos
            };
            out.push(PhysicalLine {
                start,
                content_end,
                end: pos + 1,
            });
            start = pos + 1;
        }
    }
    if start < bytes.len() {
        // Final line without terminator; a lone trailing \r is invisible to the
        // logic (the official reader) but stays covered in the span.
        let content_end = if *bytes.last().unwrap_or(&0) == b'\r' {
            bytes.len() - 1
        } else {
            bytes.len()
        };
        out.push(PhysicalLine {
            start,
            content_end,
            end: bytes.len(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logicals(src: &str) -> Vec<(u32, String, bool)> {
        preprocess(src)
            .iter()
            .map(|l| (l.number, l.joined(src), l.dropped))
            .collect()
    }

    fn covered(src: &str) {
        let ls = preprocess(src);
        let mut cursor = 0_usize;
        for l in &ls {
            assert_eq!(l.span.0, cursor, "tiling gap in {src:?}");
            cursor = l.span.1;
        }
        assert_eq!(cursor, src.len(), "full cover for {src:?}");
    }

    #[test]
    fn plain_lines() {
        assert_eq!(
            logicals("a\nb\n"),
            vec![(1, "a".into(), false), (2, "b".into(), false)]
        );
        assert_eq!(logicals("a"), vec![(1, "a".into(), false)]);
        assert_eq!(logicals(""), Vec::<(u32, String, bool)>::new());
    }

    #[test]
    fn bom_stripped_from_first_line() {
        assert_eq!(
            logicals("\u{FEFF}workspace {\n"),
            vec![(1, "workspace {".into(), false)]
        );
    }

    #[test]
    fn continuation_joins_without_separator() {
        // Official semantics: middle lines keep their indentation (only the FINAL
        // completing line is `stripLeading`ed), and the group is emitted under the
        // LAST physical line's number.
        let src = "x = \\\n    y \\\n    \"Software \\\n    System\"\nz\n";
        let got = logicals(src);
        assert_eq!(
            got[0],
            (4, "x =     y     \"Software System\"".into(), false)
        );
        assert_eq!(got[1], (5, "z".into(), false));
        covered(src);
    }

    #[test]
    fn comment_lines_exempt_from_continuation() {
        let src = "# hi \\\nx\n";
        assert_eq!(
            logicals(src),
            vec![(1, "# hi \\".into(), false), (2, "x".into(), false)]
        );
    }

    #[test]
    fn dangling_continuation_is_dropped() {
        let got = logicals("a\nb \\\n");
        assert_eq!(got.len(), 2);
        assert_eq!(got[1], (2, "b ".into(), true));
        covered("a\nb \\\n");
    }

    #[test]
    fn text_block_fold() {
        let src = "a b \"\"\"\n    line one\n    line two\n    \"\"\"\nz\n";
        let ls = preprocess(src);
        assert_eq!(ls.len(), 2);
        assert_eq!(ls[0].number, 4, "emitted under the closing line's number");
        let tb = ls[0].text_block.expect("block present");
        assert!(tb.closed);
        assert_eq!(
            text_block_value(src, &tb).as_deref(),
            Some("line one\nline two")
        );
        // Official prefix: everything before the first `"""`.
        assert_eq!(ls[0].joined(src), "a b ");
        assert_eq!(ls[1].joined(src), "z");
        covered(src);
    }

    #[test]
    fn text_block_empty_middle_line_kept() {
        let src = "d \"\"\"\n  a\n\n  b\n  \"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb).as_deref(), Some("a\n\nb"));
    }

    #[test]
    fn text_block_closing_line_content_discarded() {
        let src = "d \"\"\"\n  a\n  junk\"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb).as_deref(), Some("a"));
    }

    #[test]
    fn text_block_marker_inside_value_truncates() {
        let src = "d \"\"\"\n  a\"\"\"b\n  \"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb).as_deref(), Some(""));
    }

    #[test]
    fn text_block_reference_crash_cases_return_none() {
        // Empty value: the official parser throws here → None.
        let src = "d \"\"\"\n  \"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb), None);
        // Dedent slices CHARACTERS blindly (the official append), so a
        // 2-space-shallow line simply loses its first 4 chars (" shallow" → "allow");
        // only a line SHORTER than the width throws (→ None).
        let src = "d \"\"\"\n    deep\n  shallow\n    \"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb).as_deref(), Some("deep\nallow"));
        let src = "d \"\"\"\n      deep\n   x\n      \"\"\"\n";
        let ls = preprocess(src);
        let tb = ls[0].text_block.unwrap();
        assert_eq!(text_block_value(src, &tb), None);
    }

    #[test]
    fn unterminated_text_block_drops_tail() {
        let src = "x \"\"\"\n  a\n  b\n";
        let got = logicals(src);
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0].0, 3,
            "EOF tail carries the last physical line number"
        );
        assert!(got[0].2, "tail marked dropped like the reference");
        covered(src);
    }

    #[test]
    fn crlf_line_endings_invisible_to_logic() {
        let got = logicals("a = \\\r\n  b\r\nc\r\n");
        assert_eq!(got[0], (2, "a = b".into(), false)); // flushes on the completing line
        assert_eq!(got[1], (3, "c".into(), false));
        covered("a = \\\r\n  b\r\nc\r\n");
    }

    #[test]
    fn blank_and_comment_classification() {
        let src = "   \n\t\n// c\n# c\n x\n";
        let ls = preprocess(src);
        assert!(ls[0].is_empty_line(src) && ls[1].is_empty_line(src));
        assert!(ls[2].is_comment_line(src) && ls[3].is_comment_line(src));
        assert!(!ls[4].is_comment_line(src));
    }

    #[test]
    fn tiling_corner_inputs() {
        for src in [
            "a\nb\n",
            "a",
            "",
            "\n",
            "\r\n",
            "a\r\nb",
            "x \"\"\"\n  t\n  \"\"\"\n",
            "a \\\n",
            "# c\\\n\n",
            "\u{FEFF}a\n",
            "\"\"\"\n  x\n  \"\"\"\n",
        ] {
            covered(src);
        }
    }
}
