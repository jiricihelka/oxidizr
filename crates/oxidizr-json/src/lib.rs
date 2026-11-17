//! `oxidizr-json` — the JSON contract (P1-D5 amended, P1-D6).
//!
//! The serde-derive route turned out to be unable to reproduce the reference's
//! byte order (see the P1-D5 amendment in `agents/plan/build/PHASE_1.md`): Jackson
//! writes POJO keys alphabetically at every level, applies per-field
//! `NON_EMPTY`/`NON_NULL` inclusion, sorts `TreeSet`-backed arrays, orders
//! free-form `properties` maps in Java `HashMap` bucket order, and computes
//! view `name` values on the fly. So this crate hand-rolls the two directions:
//!
//! - [`to_vec`] builds a `printer::Json` tree with FINAL key order and
//!   omissions already decided per `agents/plan/build/WRITE-CATALOG.md` (the §-cites
//!   in `writer.rs`), then renders it with the Jackson pretty-printer
//!   emulation (`printer.rs`, `"key" : value`, 2-space indent, no trailing
//!   newline).
//! - [`from_slice`] walks a `serde_json::Value` (document-order maps via the
//!   `preserve_order` feature) applying the §6 leniency matrix, then runs the
//!   M2 validation catalog exactly like the reference `hydrate()`.
//!
//! The byte witness for both directions is `testdata/golden/` (Category A
//! byte-diff tests); the reader tolerates everything the reference reader
//! tolerates and rejects with the reference's first `hydrate()` message.

pub mod error;
// Public for the C4-PlantUML tags section (Phase 2 M3): the tags-section
// maps iterate in Java `HashMap<String, Style>` order, the same contract the
// JSON writer pins here.
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

/// Serialize to the reference's exact JSON bytes (Jackson pretty, no
/// trailing newline — WRITE-CATALOG §0).
///
/// # Errors
///
/// [`JsonError::Write`] when a computed value cannot be produced — a
/// `CustomView`/`FilteredView` with a missing base view, or a missing
/// relationship an entry references.
pub fn to_vec(workspace: &Workspace) -> Result<Vec<u8>, JsonError> {
    writer::to_vec(workspace)
}

/// Deserialize a workspace JSON document (WRITE-CATALOG §6 leniency, then
/// `hydrate()`-equivalent validation).
///
/// # Errors
///
/// [`JsonError::Parse`] for malformed JSON, [`JsonError::Read`] for
/// structurally invalid values (e.g. a non-numeric `instances` spec),
/// [`JsonError::Validation`] when the reference `hydrate()` would reject the
/// workspace (the first validation message is carried verbatim).
pub fn from_slice(bytes: &[u8]) -> Result<Workspace, JsonError> {
    reader::from_slice(bytes)
}
