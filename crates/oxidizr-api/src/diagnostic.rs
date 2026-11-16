//! The pure diagnostic data type (moved out of the CLI verbatim in T0 so the
//! headless pipeline can produce diagnostics without miette — the CLI's
//! `Renderer` remains the only miette user).
//!
//! Engine `DslError`s, `JsonError`s and core validation messages normalize
//! into [`Issue`]s here. A `DslError` carries the reference message WITHOUT
//! the ` at line N of <path>` suffix the reference exception text appends —
//! the CLI re-appends it verbatim (users grep for reference texts), and
//! renders the error's `source_line` as a miette snippet.

use std::fmt::Write as _;

use oxidizr_diag::{Code, Severity as OxzSeverity, codes};
use oxidizr_dsl::engine::DslError;
use oxidizr_json::JsonError;

/// One renderable diagnostic.
#[derive(Debug, Clone)]
pub struct Issue {
    /// Severity (`--strict` promotes warnings to exit-1 status).
    pub severity: OxzSeverity,
    /// Stable OXZ code.
    pub code: Code,
    /// Full one-line message (reference text, location suffix included
    /// where the reference has one).
    pub message: String,
    /// Snippet (named source text + labelled byte range); boxed so the
    /// `Err`-variant stays small (`result_large_err`).
    pub snippet: Option<Box<Snippet>>,
    /// Help line ("did you mean …?" family).
    pub help: Option<String>,
}

/// Snippet input: the named source text plus the range labelled in it.
/// Format-neutral (the CLI renders it through miette's `NamedSource`).
#[derive(Debug, Clone)]
pub struct Snippet {
    /// Named source (file path or `-`).
    pub name: String,
    /// The source text.
    pub text: String,
    /// Byte span into `text`.
    pub span: (usize, usize),
}

impl Issue {
    /// Error-severity issue with only a code + message.
    #[must_use]
    pub fn error(code: Code, message: impl Into<String>) -> Self {
        Issue {
            severity: OxzSeverity::Error,
            code,
            message: message.into(),
            snippet: None,
            help: None,
        }
    }

    /// Attach a snippet (named text + labelled byte range).
    #[must_use]
    pub fn with_snippet(mut self, name: impl Into<String>, text: impl Into<String>) -> Self {
        let name = name.into();
        let text = text.into();
        let length = text.len();
        self.snippet = Some(Box::new(Snippet {
            name,
            text,
            span: (0, length.max(1)),
        }));
        self
    }

    /// Engine statement/parse error: reference exception text re-assembled
    /// (`<message>` minus trailing `.` + ` at line N of <file>`), snippet =
    /// the offending logical line.
    #[must_use]
    pub fn from_dsl(e: &DslError) -> Self {
        let mut message = e.message.clone();
        if let Some(line) = e.line {
            if message.ends_with('.') {
                message.pop();
            }
            let _ = write!(message, " at line {line}");
            if let Some(file) = &e.file {
                let _ = write!(message, " of {file}");
            }
        }
        let issue = Issue::error(e.code, message);
        match &e.source_line {
            Some(text) => {
                issue.with_snippet(e.file.clone().unwrap_or_else(|| "-".into()), text.clone())
            }
            None => issue,
        }
    }

    /// JSON pipeline error. `Parse` carries serde's line/column into the
    /// snippet span of the full input text.
    #[must_use]
    pub fn from_json(e: &JsonError, text: &str, name: &str) -> Self {
        match e {
            JsonError::Parse(se) => {
                let mut issue =
                    Issue::error(codes::JSON_READ, format!("failed to parse JSON: {se}"));
                let (line, column) = (se.line(), se.column());
                if line > 0 && column > 0 {
                    let offset = offset_of_line(text, line).saturating_add(column - 1);
                    issue.snippet = Some(Box::new(Snippet {
                        name: name.to_string(),
                        text: text.to_string(),
                        span: (offset.min(text.len()), 1),
                    }));
                }
                issue
            }
            JsonError::Read(m) => {
                Issue::error(codes::JSON_READ, format!("failed to read workspace: {m}"))
            }
            JsonError::Validation(m) => Issue::error(
                codes::JSON_READ,
                format!("workspace validation failed: {m}"),
            ),
            JsonError::Write(m) => {
                Issue::error(codes::JSON_WRITE, format!("failed to write workspace: {m}"))
            }
        }
    }

    /// Core validation-catalog violation (`validate` command).
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Issue::error(codes::VALIDATION, message)
    }

    /// CLI I/O / input-policy failure (exit 3 family).
    #[must_use]
    pub fn io(what: &str, reason: &dyn std::error::Error) -> Self {
        Issue::error(codes::CLI_IO, format!("{what}: {reason}"))
    }

    /// Counts toward exit 1 under `--strict` (severity-aware).
    #[must_use]
    pub fn blocks(&self, strict: bool) -> bool {
        match self.severity {
            OxzSeverity::Error => true,
            OxzSeverity::Warning => strict,
            OxzSeverity::Info => false,
        }
    }
}

/// Byte offset where 1-based physical `line` starts.
pub(crate) fn offset_of_line(text: &str, line: usize) -> usize {
    text.split_inclusive('\n')
        .take(line - 1)
        .map(str::len)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_offsets_match_split_inclusive() {
        let text = "one\ntwo\nthree\n";
        assert_eq!(offset_of_line(text, 1), 0);
        assert_eq!(offset_of_line(text, 2), 4);
        assert_eq!(offset_of_line(text, 3), 8);
    }
}
