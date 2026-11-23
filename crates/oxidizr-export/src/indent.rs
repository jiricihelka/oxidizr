//! The accumulating text buffer every diagram export builds on.
//! Semantics:
//!
//! * `writeLine()` appends a bare `\n` (NO padding);
//! * `writeLine(content)` pads, writes, and escapes embedded newlines as
//!   the literal two characters `\n`;
//! * the render drops ONE trailing newline;
//! * padding is `indent * quantity` spaces or tabs — a negative indent
//!   (unguarded `outdent`) pads nothing.

use std::fmt;

/// Spaces or tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndentType {
    #[default]
    Spaces,
    Tabs,
}

/// The buffer: level starts at 0, two spaces per level — the default
/// quantity is not derivable, see the manual [`Default`] below.
#[derive(Debug)]
pub struct IndentingWriter {
    // `i32` like the indent level upstream: an unbalanced outdent goes
    // negative and simply pads nothing (a `usize` would underflow).
    indent: i32,
    indent_type: IndentType,
    indent_quantity: i32,
    buf: String,
}

impl Default for IndentingWriter {
    fn default() -> Self {
        Self {
            indent: 0,
            indent_type: IndentType::Spaces,
            indent_quantity: 2,
            buf: String::new(),
        }
    }
}

impl IndentingWriter {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_indent_type(&mut self, indent_type: IndentType) {
        self.indent_type = indent_type;
    }

    pub fn set_indent_quantity(&mut self, indent_quantity: i32) {
        self.indent_quantity = indent_quantity;
    }

    pub fn indent(&mut self) {
        self.indent += 1;
    }

    pub fn outdent(&mut self) {
        self.indent -= 1;
    }

    /// `writeLine()` / `writeLine(content)` — `None` appends the bare
    /// newline, `Some` pads, writes (newlines inside the content become the
    /// literal `\n`), and terminates.
    pub fn write_line(&mut self, content: Option<&str>) {
        match content {
            None => self.buf.push('\n'),
            Some(text) => {
                // the pad is written straight into the buffer
                // (the old `padding()` allocated a `String` per line) and
                // the newline-escape copy runs ONLY when the content
                // actually contains one. Same bytes either way.
                let unit = match self.indent_type {
                    IndentType::Spaces => ' ',
                    IndentType::Tabs => '\t',
                };
                let count = self.indent.saturating_mul(self.indent_quantity).max(0);
                self.buf.extend(std::iter::repeat_n(
                    unit,
                    usize::try_from(count).unwrap_or(0),
                ));
                if text.contains('\n') {
                    self.buf.push_str(&text.replace('\n', "\\n"));
                } else {
                    self.buf.push_str(text);
                }
                self.buf.push('\n');
            }
        }
    }

    /// `replace(before, after)` — first occurrence only. The official
    /// throws when `before` is absent (its callers only replace anchors they
    /// just wrote); oxidizr silently skips, matching every reachable
    /// official path and avoiding a panic on hostile input.
    pub fn replace(&mut self, before: &str, after: &str) {
        if let Some(start) = self.buf.find(before) {
            self.buf.replace_range(start..start + before.len(), after);
        }
    }
}

/// The render — the buffer without ONE trailing newline.
impl fmt::Display for IndentingWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.buf.strip_suffix('\n').unwrap_or(&self.buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_two_spaces_and_to_string_drops_one_trailing_newline() {
        let mut w = IndentingWriter::new();
        w.write_line(Some("a"));
        w.write_line(Some("b"));
        assert_eq!(w.to_string(), "a\nb");
        // exactly ONE newline dropped:
        w.write_line(None);
        assert_eq!(w.to_string(), "a\nb\n");
    }

    #[test]
    fn indent_levels_and_tabs() {
        let mut w = IndentingWriter::new();
        w.indent();
        w.write_line(Some("in"));
        w.indent();
        w.write_line(Some("deeper"));
        w.outdent();
        w.outdent();
        w.outdent(); // negative: pads nothing, like the Java int
        w.write_line(Some("out"));
        assert_eq!(w.to_string(), "  in\n    deeper\nout");

        let mut t = IndentingWriter::new();
        t.set_indent_type(IndentType::Tabs);
        t.set_indent_quantity(2);
        t.indent();
        t.write_line(Some("x"));
        assert_eq!(t.to_string(), "\t\tx");
    }

    #[test]
    fn write_line_escapes_embedded_newlines() {
        let mut w = IndentingWriter::new();
        w.write_line(Some("one\ntwo"));
        assert_eq!(w.to_string(), r"one\ntwo");
    }

    #[test]
    fn bare_write_line_has_no_padding() {
        let mut w = IndentingWriter::new();
        w.indent();
        w.write_line(None);
        // the bare newline is padded by nothing — and the render drops the
        // single trailing newline, leaving nothing:
        assert_eq!(w.to_string(), "");
    }

    #[test]
    fn replace_touches_only_the_first_occurrence() {
        let mut w = IndentingWriter::new();
        w.write_line(Some("aa aa"));
        w.replace("aa", "b");
        assert_eq!(w.to_string(), "b aa");
        w.replace("zz", "y"); // absent: no-op, no panic
        assert_eq!(w.to_string(), "b aa");
    }
}
