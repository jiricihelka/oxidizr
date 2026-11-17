//! `thiserror` error type for the export boundary.

/// An export failure.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The format is in the reference registry (or is a `static`/`png`/`svg`
    /// branch) but oxidizr's exporter has not landed yet (M1–M4) —
    /// surfaced as `OXZ-5007`.
    #[error("`{format}` is not implemented by oxidizr yet (see agents/plan/build/PHASE_2.md)")]
    NotImplemented {
        /// The requested format string (canonical spelling).
        format: String,
    },

    /// `ExportCommand` derives the workspace-export file name with
    /// `name.substring(0, lastIndexOf('.'))` — a workspace file name without
    /// a `.` fails there (`StringIndexOutOfBoundsException`); oxidizr
    /// reports it as an input error instead of panicking.
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

    /// The reference JVM ABORTS the whole export on these (a failed cast
    /// of the untyped `frame`, a `ClassCastException` from a foreign
    /// element kind in a view, a dangling id dereferenced mid-write);
    /// oxidizr surfaces the abort as an error value instead of panicking
    /// (`EXPORT-PLANTUML.md` §14 cast-aborts).
    #[error("cannot export: {0}")]
    ReferenceAbort(&'static str),
}
