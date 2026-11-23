//! `oxidizr-api` — the headless pipeline behind the CLI.
//!
//! Contract ([`pipeline::execute`]): values in / values out. No rendering
//! (the CLI owns miette), no process env reads (the injected
//! [`oxidizr_dsl::engine::EnvLookup`] and JSON-base-loader seams keep parses
//! hermetic), total (never panics), thread-safe (`Send + Sync` end to end).
//! This is the stable surface the e2e suite drives in-process; the CLI
//! binary is a thin argv/render shim over it so tests and the shipped
//! binary cannot drift.
//!
//! Modules: [`diagnostic`] — the pure [`Issue`] data type;
//! [`suggest`] — the `did you mean …?` decoration; [`reconcile`] — the
//! geometry re-attach; [`pipeline`] — the command bodies behind
//! [`execute`]; [`fn@open`] — the viewer-facing load entry (not a
//! byte-contract path).

pub mod diagnostic;
pub mod diff;
pub mod generate;
pub mod init;
pub mod open;
pub mod pipeline;
pub mod query;
pub mod reconcile;
pub mod suggest;

pub use diagnostic::{Issue, Snippet};
// The `Request::Generate` strategy travels on the API surface (the CLI
// maps its clap enum onto it).
pub use generate::RelationshipsStrategy;
pub use init::InitTemplate;
pub use open::{Opened, open};
// `ImportFormat` (+ its `sniff` constructor) lives in `oxidizr-import`
// and travels on this surface (the CLI maps its clap enum onto it).
pub use oxidizr_import::ImportFormat;
// The `Request::Inspect` threshold type travels through the API surface
// (the CLI maps its clap enum onto it).
pub use oxidizr_inspect::Severity;
pub use pipeline::{
    ApiOptions, DiffFormat, ExitClass, FmtMode, Input, InspectFormat, LogItem, Outcome, ParseFlags,
    Request, StdinReader, execute,
};
// The `Request::Query` enums travel on the API surface (the CLI maps its
// clap enums onto these).
pub use query::{QueryFormat, QueryKind};
