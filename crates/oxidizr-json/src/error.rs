//! `thiserror` error type for the JSON boundary (errors as values; the CLI
//! wraps these in `anyhow`).

/// A JSON read/write failure.
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    /// The input is not syntactically valid JSON (byte position available
    /// through the inner `serde_json::Error`).
    #[error("failed to parse JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// Structurally valid JSON the official reader rejects (e.g. an invalid
    /// `instances` spec on a deployment node). Message text matches the
    /// official tooling's message, verbatim, where one is defined.
    #[error("failed to read workspace: {0}")]
    Read(String),

    /// The workspace loaded but failed validation — validation reports the
    /// FIRST message of the model/view catalog.
    #[error("workspace validation failed: {0}")]
    Validation(String),

    /// The model cannot be written byte-faithfully (e.g. a filtered view
    /// whose base view does not exist — the official tool crashes there; we
    /// fail cleanly instead).
    #[error("failed to write workspace: {0}")]
    Write(String),
}
