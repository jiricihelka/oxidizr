//! `oxidizr-diag` — shared diagnostic plumbing (P1-D2):
//!
//! * [`Span`] — file + byte range + 1-based line/col (UTF-8 **char** counts, matching
//!   the reference's human-facing line numbers; the reference emits no column, so the
//!   column here is additive precision within D1 freedom).
//! * [`Severity`] — `Error` / `Warning` / `Info`.
//! * [`Code`] — `OXZ-nnnn` newtype; ranges: 1xxx lexer, 2xxx parser, 3xxx model &
//!   validation, 4xxx JSON, 5xxx I/O & CLI policy, 9xxx internal. Every code is
//!   documented in `docs/diagnostics.md` and stable post-1.0.
//! * [`Collector`] — parsers/validators accumulate diagnostics; the cap is 100, then
//!   one "too many errors" note replaces further detail (multi-error parsing, D1).
//!
//! No `miette` here: the CLI converts at its boundary (P1-D2), libraries stay
//! dependency-free.

use std::fmt;

/// Handle into a [`Files`] table. `0` is conventionally the root input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileId(pub u32);

/// Interned set of source file names, so spans stay `Copy`.
#[derive(Debug, Clone, Default)]
pub struct Files {
    names: Vec<String>,
}

impl Files {
    /// Register a file name, returning its id. Names are stored verbatim.
    pub fn add(&mut self, name: impl Into<String>) -> FileId {
        let name = name.into();
        if let Some(pos) = self.names.iter().position(|n| *n == name) {
            return FileId(u32::try_from(pos).unwrap_or(u32::MAX));
        }
        self.names.push(name);
        FileId(u32::try_from(self.names.len() - 1).unwrap_or(u32::MAX))
    }

    /// Look up a registered name.
    #[must_use]
    pub fn name(&self, id: FileId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }
}

/// 1-based position; `col` counts UTF-8 characters from the start of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Position {
    /// 1-based physical line number.
    pub line: u32,
    /// 1-based character column.
    pub col: u32,
}

impl Position {
    /// The unset/absent position (renders as `0:0`, never printed for real spans).
    pub const NONE: Position = Position { line: 0, col: 0 };
}

/// Region of a source file: byte offsets (machine use) plus start position (human
/// use). The end position is omitted — diagnostics quote whole lines anyway
/// (agents/plan/COMPAT.md: parser errors carry file, line and the offending source line).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// Which file.
    pub file: FileId,
    /// Byte offsets into the (post-decoding) source text.
    pub start: usize,
    /// Exclusive end byte offset.
    pub end: usize,
    /// Human-facing start position.
    pub pos: Position,
}

impl Span {
    /// Construct a span.
    #[must_use]
    pub fn new(file: FileId, start: usize, end: usize, pos: Position) -> Span {
        Span {
            file,
            start,
            end,
            pos,
        }
    }
}

/// Diagnostic severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// Blocks the operation (exit 1 without `--strict`, and with it, warnings too).
    Error,
    /// Suspicious but loadable.
    Warning,
    /// Informational.
    Info,
}

/// `OXZ-nnnn` diagnostic code. Ranges per `docs/diagnostics.md`:
/// `1xxx` lexer · `2xxx` parser/grammar · `3xxx` model & validation · `4xxx` JSON ·
/// `5xxx` I/O & CLI policy · `9xxx` internal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(u16);

impl Code {
    /// Construct a code from its numeric part (`1001` ⇒ `OXZ-1001`).
    #[must_use]
    pub const fn new(n: u16) -> Code {
        Code(n)
    }

    /// Numeric part.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }

    /// Range owner (thousands digit).
    #[must_use]
    pub const fn range(self) -> CodeRange {
        match self.0 / 1000 {
            1 => CodeRange::Lexer,
            2 => CodeRange::Parser,
            3 => CodeRange::Model,
            4 => CodeRange::Json,
            5 => CodeRange::IoPolicy,
            9 => CodeRange::Internal,
            _ => CodeRange::Other,
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OXZ-{:04}", self.0)
    }
}

/// The `OXZ-nnnn` thousands ranges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeRange {
    /// OXZ-1xxx
    Lexer,
    /// OXZ-2xxx
    Parser,
    /// OXZ-3xxx
    Model,
    /// OXZ-4xxx
    Json,
    /// OXZ-5xxx
    IoPolicy,
    /// OXZ-9xxx
    Internal,
    /// Anything else (should not occur; renders defensively).
    Other,
}

/// Registered codes (mirrors `docs/diagnostics.md`; add rows there first).
pub mod codes {
    use super::Code;

    /// `"<X>" is not valid here` — unknown token in this scope (+ did-you-mean).
    pub const UNEXPECTED_TOKEN: Code = Code::new(2001);
    /// Statement rejected — the reference message travels verbatim (M4).
    pub const STATEMENT: Code = Code::new(2002);
    /// Unterminated block: `Unexpected end of DSL content …` (M4).
    pub const UNEXPECTED_EOF: Code = Code::new(2003);
    /// Unterminated quoted token at end of line (bug-compat: the reference does not
    /// error here; reserved for `--strict`-style linting, unused by default).
    pub const LEXER_UNTERMINATED_QUOTED: Code = Code::new(1001);
    /// Network disabled: URL input without `--allow-net` (D5).
    pub const NET_DISABLED: Code = Code::new(5001);
    /// JVM-only directive not supported (`!script`, `!plugin`, `!components`,
    /// `!impliedRelationships <fqcn>`) (D2).
    pub const JVM_ONLY: Code = Code::new(5002);
    /// `!docs`/`!adrs` parsed; importers arrive in Phase 4 (D7, retired by P4-D13).
    pub const DOCS_DEFERRED: Code = Code::new(5003);
    /// `workspace extends` parsed; the base-workspace loader arrives in M5 (D9).
    pub const EXTENDS_DEFERRED: Code = Code::new(5004);
    /// Server-side import deferred: file-based theme inlining and the
    /// raster/plantuml/mermaid/kroki image-view importers need the
    /// `structurizr-import` stack (D11).
    pub const IMPORT_DEFERRED: Code = Code::new(5005);
    /// Too many errors: collection aborted at the cap (9xxx internal).
    pub const TOO_MANY_ERRORS: Code = Code::new(9001);
}

/// One diagnostic: severity, stable code, message, optional primary span and extras.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Severity.
    pub severity: Severity,
    /// Stable code.
    pub code: Code,
    /// One-line message (no location prefix; renderers add it).
    pub message: String,
    /// Primary source location, when known.
    pub primary: Option<Span>,
    /// Suggested fix word, e.g. "did you mean `include`?" (CLI renders it).
    pub suggestion: Option<String>,
    /// Extra context lines.
    pub notes: Vec<String>,
}

impl Diagnostic {
    /// Error with code + message.
    #[must_use]
    pub fn error(code: Code, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            primary: None,
            suggestion: None,
            notes: Vec::new(),
        }
    }

    /// Warning with code + message.
    #[must_use]
    pub fn warning(code: Code, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, message)
        }
    }

    /// Attach a primary span (builder style).
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Diagnostic {
        self.primary = Some(span);
        self
    }

    /// Attach a suggestion (builder style).
    #[must_use]
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Diagnostic {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Append a note (builder style).
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Diagnostic {
        self.notes.push(note.into());
        self
    }

    /// True when any error is present.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Accumulates diagnostics with the P1-D2 cap: after [`Collector::CAP`] entries,
/// further pushes are dropped and a single [`codes::TOO_MANY_ERRORS`] note is
/// appended by [`Collector::finish`].
#[derive(Debug, Clone)]
pub struct Collector {
    items: Vec<Diagnostic>,
    overflow: usize,
}

impl Collector {
    /// Maximum collected diagnostics before aborting detail.
    pub const CAP: usize = 100;

    /// Empty collector.
    #[must_use]
    pub fn new() -> Collector {
        Collector {
            items: Vec::new(),
            overflow: 0,
        }
    }

    /// Push a diagnostic (counting, but not storing, beyond the cap).
    pub fn push(&mut self, diagnostic: Diagnostic) {
        if self.items.len() < Self::CAP {
            self.items.push(diagnostic);
        } else {
            self.overflow += 1;
        }
    }

    /// Extend from an iterator of diagnostics.
    pub fn extend<I: IntoIterator<Item = Diagnostic>>(&mut self, iter: I) {
        for d in iter {
            self.push(d);
        }
    }

    /// True if any collected diagnostic is an error.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.items.iter().any(Diagnostic::is_error)
    }

    /// Number of stored diagnostics.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether nothing was collected (overflow counts as non-empty).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty() && self.overflow == 0
    }

    /// Consume, appending the abort note if the cap was exceeded.
    #[must_use]
    pub fn finish(mut self) -> Vec<Diagnostic> {
        if self.overflow > 0 {
            self.items.push(Diagnostic::error(
                codes::TOO_MANY_ERRORS,
                format!(
                    "too many errors: {} further diagnostics suppressed",
                    self.overflow
                ),
            ));
        }
        self.items
    }
}

impl Default for Collector {
    fn default() -> Collector {
        Collector::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(files: &mut Files) -> Span {
        let f = files.add("a.dsl");
        Span::new(f, 4, 9, Position { line: 2, col: 5 })
    }

    #[test]
    fn files_dedup_and_lookup() {
        let mut files = Files::default();
        let a1 = files.add("x.dsl");
        let b = files.add("y.dsl");
        let a2 = files.add("x.dsl");
        assert_eq!(a1, a2);
        assert_ne!(a1, b);
        assert_eq!(files.name(a1), Some("x.dsl"));
        assert_eq!(files.name(FileId(99)), None);
    }

    #[test]
    fn code_display_and_ranges() {
        assert_eq!(codes::UNEXPECTED_TOKEN.to_string(), "OXZ-2001");
        assert_eq!(codes::LEXER_UNTERMINATED_QUOTED.range(), CodeRange::Lexer);
        assert_eq!(codes::NET_DISABLED.range(), CodeRange::IoPolicy);
        assert_eq!(Code::new(7000).range(), CodeRange::Other);
    }

    #[test]
    fn collector_caps_and_notes_overflow() {
        let mut files = Files::default();
        let s = span(&mut files);
        let mut c = Collector::new();
        for _ in 0..(Collector::CAP + 7) {
            c.push(Diagnostic::error(codes::UNEXPECTED_TOKEN, "boom").with_span(s));
        }
        let out = c.finish();
        assert_eq!(out.len(), Collector::CAP + 1);
        assert_eq!(out.last().unwrap().code, codes::TOO_MANY_ERRORS);
        assert!(out.last().unwrap().message.contains("7 further"));
    }

    #[test]
    fn builder_sets_fields() {
        let mut files = Files::default();
        let s = span(&mut files);
        let d = Diagnostic::warning(codes::DOCS_DEFERRED, "deferred")
            .with_span(s)
            .with_suggestion("include")
            .with_note("lands in P4");
        assert!(!d.is_error());
        assert_eq!(d.primary, Some(s));
        assert_eq!(d.suggestion.as_deref(), Some("include"));
        assert_eq!(d.notes.len(), 1);
    }
}
