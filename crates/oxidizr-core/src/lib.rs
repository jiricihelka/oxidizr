//! `oxidizr-core` — the domain model: workspace, people, software systems, containers,
//! components, relationships, deployment nodes/instances, views, styles, configuration,
//! plus the validation catalog.
//!
//! Dependency-free by design: even diagnostics travel out as plain values with
//! `Option<Span>`; conversion to `oxidizr-diag::Diagnostic` happens in `dsl`/`json`/`cli`.
//! Model instances are immutable once built (`WorkspaceBuilder` → `build()`), cheaply
//! cloneable, field order mirrors the JSON contract.

pub mod builder;
pub mod canonical;
pub mod colors;
pub mod configuration;
pub mod docs;
pub mod error;
pub mod filter;
pub mod ids;
pub mod model;
pub mod properties;
pub mod styles;
pub mod tags;
pub mod url;
pub mod validate;
pub mod viewops;
pub mod views;

/// `DeploymentElement.DEFAULT_DEPLOYMENT_ENVIRONMENT` — the environment name
/// nodes/infra-nodes/instances carry when no `deploymentEnvironment` scope
/// names another one.
pub const DEFAULT_ENVIRONMENT: &str = "Default";

/// `DeploymentNode.instances` — a STRING field (the JSON writer emits
/// it quoted; the reader must also accept plain numbers).
pub const DEFAULT_INSTANCES: &str = "1";

pub use builder::WorkspaceBuilder;
pub use configuration::{
    Role, User, Visibility, Workspace, WorkspaceConfiguration, WorkspaceScope,
};
pub use error::CoreError;
pub use ids::{Id, IdGenerator};
pub use validate::validate;
pub use viewops::ViewRef;
