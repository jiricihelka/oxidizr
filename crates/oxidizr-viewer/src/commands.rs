//! Invertible editing commands + undo/redo stacks (P3-D4): every edit is a
//! command mutating only the [`LayoutOverlay`] — geometry deltas over the
//! read-only workspace; the model and DSL text are immutable (editor rule,
//! P3-D0). Undo/redo: `Vec` stacks, cap 200.
//!
//! M0 boundary stub: the command set lands in M3.

/// Placeholder for the geometry overlay the commands mutate (M3).
#[derive(Debug, Default)]
pub struct LayoutOverlay;
