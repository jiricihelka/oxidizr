//! `thiserror` error type for the export boundary.

/// An export failure.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The format is in the official tooling's export registry (or is a
    /// `static`/`png`/`svg` branch) but oxidizr's exporter does not
    /// implement it yet — surfaced as `OXZ-5007`.
    #[error("`{format}` is not implemented by oxidizr yet (see agents/plan/build/PHASE_2.md)")]
    NotImplemented {
        /// The requested format string (canonical spelling).
        format: String,
    },

    /// The workspace-export file name is the workspace file name truncated
    /// at its last `.`; a name without a `.` makes the official tooling
    /// fail with an out-of-range error — oxidizr reports it as an input
    /// error instead (a documented deviation).
    #[error(
        "cannot derive the export file name from `{name}`: the workspace file name has no extension"
    )]
    NoExtension {
        /// The workspace file name.
        name: String,
    },

    /// The workspace cannot be serialized (delegated JSON layer).
    #[error(transparent)]
    Json(#[from] oxidizr_json::JsonError),

    /// The official tooling ABORTS the whole export on these (a malformed
    /// animation `frame`, a view element of a kind it cannot handle, a
    /// dangling id dereferenced mid-write); oxidizr surfaces the abort as
    /// an error value instead of panicking.
    #[error("cannot export: {0}")]
    ReferenceAbort(&'static str),
}
