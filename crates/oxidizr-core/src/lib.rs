//! `oxidizr-core` — the domain model: workspace, people, software systems, containers,
//! components, relationships, deployment nodes/instances, views, styles, configuration,
//! plus the validation catalog.
//!
//! Dependency-free by design (P1-D1): even diagnostics travel out as plain values with
//! `Option<Span>`; conversion to `oxidizr-diag::Diagnostic` happens in `dsl`/`json`/`cli`.
//! Model instances are immutable once built (`WorkspaceBuilder` → `build()`), cheaply
//! cloneable, field order mirrors the JSON contract (P1-D4).
