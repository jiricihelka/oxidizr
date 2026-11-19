//! Live reload preserving layout (P3-D6): watch the opened file plus the
//! `!include` closure (notify), debounce, reparse, remap old→new ids through
//! the SHARED reconcile engine (`oxidizr-api::reconcile` — §P3-D1 decision
//! 2026-11-19: it stays in api, the viewer consumes it directly), keep
//! surviving overlay geometry.
//!
//! M0 boundary stub: lands in M4 with the watch wiring.

/// Placeholder for the watch/remap driver (M4).
#[derive(Debug, Default)]
pub struct ReloadDriver;
