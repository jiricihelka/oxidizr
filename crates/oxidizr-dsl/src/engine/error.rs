//! Engine error type: the official tooling's error-message shape
//! (`message at line N of /abs/path: trimmed line`) wrapped in an OXZ code.
//!
//! The official tooling aborts the whole parse at the first failure; oxidizr
//! keeps the message text byte-identical but records diagnostics and recovers
//! to the next logical line. `Display` renders the bare message alone — the
//! location fields are structured extras the CLI renders itself: the official
//! format embeds them IN the message, we keep them separate so the text
//! stays comparable.

use std::fmt::{self, Write as _};

use oxidizr_core::CoreError;
use oxidizr_diag::Code;
use oxidizr_diag::codes;

/// A rejected statement (or a fatal parse condition such as an unclosed
/// block). Equality is on all fields, so tests pin the full official text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslError {
    /// Stable OXZ code.
    pub code: Code,
    /// The official message WITHOUT the ` at line …` suffix (the official
    /// format appends that suffix after stripping a single trailing `.`).
    pub message: String,
    /// Absolute path of the file being parsed, when the input was a file
    /// (the official format prints `of <path>` only for a real file).
    pub file: Option<String>,
    /// 1-based logical line as the official tooling would have reported it.
    pub line: Option<u32>,
    /// The offending logical line, trimmed.
    pub source_line: Option<String>,
    /// The failure is fatal for the whole FILE: the official tooling throws
    /// it during preprocessing, before any line is processed — the parse
    /// loop stops instead of recovering to the next line.
    pub fatal_file: bool,
}

impl DslError {
    /// Statement rejection: official message as-is, code `OXZ-2002`.
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

    /// Unknown/misplaced token: the official `Unexpected tokens (expected: …)`
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
    /// and the official tooling-only families, `OXZ-5001`/`OXZ-5002`).
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
    /// `Display` text is the official message by construction).
    #[must_use]
    pub fn core(err: &CoreError) -> Self {
        DslError::stmt(err.to_string())
    }

    /// Attach the location triple (file, line, trimmed source line).
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

    /// The full exception text in the official format:
    /// `<msg minus trailing dot> at line <N> of <path>: <trimmed line>`.
    /// Parts the official format omits (no file, no line) are omitted here.
    #[must_use]
    pub fn reference_text(&self) -> String {
        let mut out = String::new();
        // A single trailing `.` is stripped before the location suffix.
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
