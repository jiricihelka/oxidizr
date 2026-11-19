//! `oxidizr-api` — the headless pipeline behind the CLI (P1T-D1, feature F9;
//! `agents/plan/build/PHASE_1T.md`).
//!
//! Contract ([`pipeline::execute`]): values in / values out. No rendering
//! (the CLI owns miette), no process env reads (the injected
//! [`oxidizr_dsl::engine::EnvLookup`] and JSON-base-loader seams keep parses
//! hermetic), total (never panics), thread-safe (`Send + Sync` end to end).
//! This is the stable surface the Phase 1T e2e suite drives in-process; the
//! CLI binary is a thin argv/render shim over it so tests and the shipped
//! binary cannot drift.
//!
//! Modules: [`diagnostic`] — the pure [`Issue`] data type (T0-b);
//! [`suggest`] — the `did you mean …?` decoration (T0-c); [`reconcile`] —
//! the F1 geometry re-attach (T0-d); [`pipeline`] — the command bodies
//! behind [`execute`] (T0-d); [`open`] — the viewer-facing load entry
//! (P3-M1-1, not a byte-contract path).

pub mod diagnostic;
pub mod open;
pub mod pipeline;
pub mod reconcile;
pub mod suggest;

pub use diagnostic::{Issue, Snippet};
pub use open::{Opened, open};
pub use pipeline::{
    ApiOptions, ExitClass, FmtMode, Input, LogItem, Outcome, Request, StdinReader, execute,
};
