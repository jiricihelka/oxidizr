//! `thiserror` error type for the JSON boundary (library crate, AGENTS.md
//! code style; the CLI wraps these in `anyhow`).

/// A JSON read/write failure.
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    /// The input is not syntactically valid JSON (byte position available
    /// through the inner `serde_json::Error`).
    #[error("failed to parse JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// Structurally valid JSON that the reference reader would reject
    /// (e.g. an invalid `instances` spec on a deployment node). Message text
    /// matches the reference exception where it defines one.
    #[error("failed to read workspace: {0}")]
    Read(String),

    /// The workspace loaded but failed validation — `hydrate()` runs the M2
    /// validation catalog and the reference throws the FIRST message.
    #[error("workspace validation failed: {0}")]
    Validation(String),

    /// The model cannot be written byte-faithfully (e.g. a filtered view
    /// whose base view does not exist — the reference NPEs during export).
    #[error("failed to write workspace: {0}")]
    Write(String),
}
