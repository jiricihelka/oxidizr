//! `oxidizr-json` — the JSON contract.
//!
//! The standard derive route cannot reproduce the workspace JSON byte order:
//! the official format writes object keys alphabetically at every level,
//! omits empty/null values, sorts set-backed arrays, orders free-form
//! `properties` maps in Java `HashMap` bucket order, and computes view
//! `name` values on the fly. So this crate hand-rolls the two directions:
//!
//! - [`to_vec`] builds a `printer::Json` tree with FINAL key order and
//!   omissions already decided per writer, then renders it with the
//!   pretty-printer shape the official tooling uses (`printer.rs`,
//!   `"key" : value`, 2-space indent, no trailing newline).
//! - [`from_slice`] walks a `serde_json::Value` (document-order maps via the
//!   `preserve_order` feature) applying the documented leniency matrix, then
//!   runs the validation catalog, failing with the official tooling's first
//!   message.
//!
//! The byte witness for both directions is `testdata/golden/` (byte-diff
//! tests); the reader tolerates everything the official reader tolerates and
//! rejects with its first validation message.

pub mod error;
// Public for the C4-PlantUML tags section: the tags-section maps iterate in
// Java `HashMap<String, Style>` order, the same contract the JSON writer
// pins here.
pub mod javahash;
pub mod keep;
mod printer;
mod reader;
pub mod theme;
mod writer;

pub use error::JsonError;
pub use keep::roundtrip_keep_unknown;
pub use theme::{theme_from_slice, theme_to_vec, theme_write};

use oxidizr_core::Workspace;

/// Serialize to the official tooling's exact JSON bytes (pretty shape, no
/// trailing newline).
///
/// # Errors
///
/// [`JsonError::Write`] when a computed value cannot be produced — a
/// `CustomView`/`FilteredView` with a missing base view, or a missing
/// relationship an entry references.
pub fn to_vec(workspace: &Workspace) -> Result<Vec<u8>, JsonError> {
    writer::to_vec(workspace)
}

/// Deserialize a workspace JSON document (the official reader's leniency,
/// then model/view validation).
///
/// # Errors
///
/// [`JsonError::Parse`] for malformed JSON, [`JsonError::Read`] for
/// structurally invalid values (e.g. a non-numeric `instances` spec),
/// [`JsonError::Validation`] when validation would reject the
/// workspace (the first message is the official tooling's, verbatim).
pub fn from_slice(bytes: &[u8]) -> Result<Workspace, JsonError> {
    reader::from_slice(bytes)
}
