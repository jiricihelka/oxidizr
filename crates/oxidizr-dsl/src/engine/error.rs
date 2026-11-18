//! Engine error type: the reference `StructurizrDslParserException` shape
//! (`message at line N of /abs/path: trimmed line`) wrapped in an OXZ code.
//!
//! The reference aborts the whole parse at the first failure; oxidizr keeps the
//! message text byte-identical but records diagnostics and recovers to the next
//! logical line (D1). `Display` renders the reference message alone — the
//! location fields are structured extras the CLI renders itself (D1 freedom:
//! the reference embeds them IN the message, we keep them separate so the
//! text stays comparable).

use std::fmt::{self, Write as _};

use oxidizr_core::CoreError;
use oxidizr_diag::Code;
use oxidizr_diag::codes;

/// A rejected statement (or a fatal parse condition such as an unclosed
/// block). Equality is on all fields, so tests pin the full reference text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError {
    /// Stable OXZ code.
    pub code: Code,
    /// Reference message WITHOUT the ` at line …` suffix (which the reference
    /// appends by stripping a single trailing `.` from the message).
    pub message: String,
    /// Absolute path of the file being parsed, when the input was a file
    /// (the reference prints `of <path>` only when `dslFile.isFile()`).
    pub file: Option<String>,
    /// 1-based logical line the reference would have reported.
    pub line: Option<u32>,
    /// The offending logical line, trimmed (reference `line.trim()`).
    pub source_line: Option<String>,
    /// The failure is fatal for the whole FILE (reference: thrown in
    /// `preProcessLines`, before any line is processed) — the parse loop
    /// stops instead of recovering to the next line.
    pub fatal_file: bool,
}

impl DslError {
    /// Statement rejection: reference message as-is, code `OXZ-2002`.
    pub fn stmt(message: impl Into<String>) -> Self {
        DslError {
            code: codes::STATEMENT,
            message: message.into(),
            file: None,
            line: None,
            source_line: None,
            fatal_file: false,
        }
    }

    /// Unknown/misplaced token: the reference `Unexpected tokens (expected: …)`
    /// family, code `OXZ-2001`.
    pub fn unexpected(message: impl Into<String>) -> Self {
        DslError {
            code: codes::UNEXPECTED_TOKEN,
            message: message.into(),
            file: None,
            line: None,
            source_line: None,
            fatal_file: false,
        }
    }

    /// Statement rejection with an explicit OXZ code (the deferred-import
    /// and JVM-only families, D7/D8/OXZ-5001/5002).
    pub fn coded(code: Code, message: impl Into<String>) -> Self {
        DslError {
            code,
            message: message.into(),
            file: None,
            line: None,
            source_line: None,
            fatal_file: false,
        }
    }

    /// A builder failure surfaced as a statement error (the `CoreError`
    /// `Display` text is the reference message by construction).
    #[must_use]
    pub fn core(err: &CoreError) -> Self {
        DslError::stmt(err.to_string())
    }

    /// Attach the reference location triple.
    #[must_use]
    pub fn at(
        mut self,
        file: Option<String>,
        line: Option<u32>,
        source_line: Option<String>,
    ) -> Self {
        self.file = file;
        self.line = line;
        self.source_line = source_line;
        self
    }

    /// The full reference exception message:
    /// `<msg minus trailing dot> at line <N> of <path>: <trimmed line>`.
    /// Parts the reference would omit (no file, no line) are omitted here.
    #[must_use]
    pub fn reference_text(&self) -> String {
        let mut out = String::new();
        // Java: `message.endsWith(".") ? substring(0, len-1) : message`
        let msg = self.message.strip_suffix('.').unwrap_or(&self.message);
        out.push_str(msg);
        if let Some(line) = self.line {
            let _ = write!(out, " at line {line}");
            if let Some(file) = &self.file {
                out.push_str(" of ");
                out.push_str(file);
            }
            if let Some(src) = &self.source_line {
                out.push_str(": ");
                out.push_str(src);
            }
        }
        out
    }
}

impl fmt::Display for DslError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DslError {}
