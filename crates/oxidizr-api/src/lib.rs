//! `oxidizr-api` — the headless pipeline behind the CLI (P1T-D1, feature F9;
//! `agents/plan/build/PHASE_1T.md`).
//!
//! Contract: values in / values out. No rendering (the CLI owns miette), no
//! process env/cwd reads (the injected [`oxidizr_dsl::engine::EnvLookup`] and
//! [`oxidizr_dsl::engine::JsonWorkspaceLoader`] seams keep parses hermetic),
//! total (never panics), thread-safe (`Send + Sync` end to end). This is the
//! stable surface the Phase 1T e2e suite drives in-process; the CLI binary is
//! a thin argv/render shim over it so tests and the shipped binary cannot
//! drift.
//!
//! Landed: [`Issue`] — the pure diagnostic data type shared with the CLI
//! renderer (T0-b). `execute`/`Request`/`Outcome` land with the command-body
//! extraction (T0-d).

pub mod diagnostic;

pub use diagnostic::{Issue, Snippet};
