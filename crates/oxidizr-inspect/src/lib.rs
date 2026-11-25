//! `oxidizr-inspect` — advisory model inspections.
//!
//! Pure model logic over [`oxidizr_core::Workspace`]: loading, the report
//! formats and the CLI live upstream (`oxidizr-api` / `oxidizr-cli`), and
//! nothing here can touch the JSON byte contract — inspections are
//! ADVISORY: they never change bytes, validation or the parser.
//!
//! Run ORDER is the report contract: workspace, then model (elements in
//! numeric-id order, each element's OUTGOING relationships in numeric-id
//! order, per-kind blocks in a fixed sequence), then views per kind, then
//! element styles. Violations keep that order (no sort).
//!
//! Severity is 100% property-driven: EVERY inspection defaults to
//! [`Severity::Error`]; the workspace (or any holder from an element up)
//! may re-rank or silence one via `structurizr.inspection.<dotted type
//! path>` keys, and `structurizr.inspection = "false"` skips the whole
//! run.
//!
//! The four `structurizr.inspection.{error,warning,info,ignore}` counter
//! properties the official tooling writes after a run are harness
//! artifacts — this crate never computes or stores them.

mod run;
mod severity;
mod strategy;
mod violation;

pub use run::inspect;
pub use severity::Severity;
pub use violation::Violation;

/// Every inspection type string, keyed by a stable name. The strings
/// themselves are the byte contract. Several checks share one string:
/// `workspace.scope` covers the scope checks (unset workspace scope,
/// multiple software systems described in detail, and the multi-system
/// system-context / container view checks); `documentation.embeddedview`
/// covers both embedded-documentation checks. The value is the
/// property-key stem AND the `oxidizr inspect --only/--except` filter
/// value.
pub mod types {
    /// The workspace was last saved by the browser-based DSL editor.
    pub const WORKSPACE_TOOLING: &str = "workspace.tooling";
    /// Scope checks (shared string): unset workspace scope, multiple
    /// software systems described in detail, multi-system context /
    /// container views.
    pub const WORKSPACE_SCOPE: &str = "workspace.scope";
    /// The model is empty (people + software systems + deployment
    /// nodes only).
    pub const MODEL_EMPTY: &str = "model.empty";
    /// A person is missing a description.
    pub const PERSON_DESCRIPTION: &str = "model.person.description";
    /// A software system is missing a description.
    pub const SOFTWARESYSTEM_DESCRIPTION: &str = "model.softwaresystem.description";
    /// A software system with containers is missing documentation.
    pub const SOFTWARESYSTEM_DOCUMENTATION: &str = "model.softwaresystem.documentation";
    /// A software system with containers is missing decisions.
    pub const SOFTWARESYSTEM_DECISIONS: &str = "model.softwaresystem.decisions";
    /// A container is missing a description.
    pub const CONTAINER_DESCRIPTION: &str = "model.container.description";
    /// A container is missing a technology.
    pub const CONTAINER_TECHNOLOGY: &str = "model.container.technology";
    /// A component is missing a description.
    pub const COMPONENT_DESCRIPTION: &str = "model.component.description";
    /// A component is missing a technology.
    pub const COMPONENT_TECHNOLOGY: &str = "model.component.technology";
    /// A deployment node is missing a description.
    pub const DEPLOYMENTNODE_DESCRIPTION: &str = "model.deploymentnode.description";
    /// A deployment node is missing a technology.
    pub const DEPLOYMENTNODE_TECHNOLOGY: &str = "model.deploymentnode.technology";
    /// A deployment node has no children, instances or infrastructure
    /// nodes.
    pub const DEPLOYMENTNODE_EMPTY: &str = "model.deploymentnode.empty";
    /// An infrastructure node is missing a description.
    pub const INFRASTRUCTURENODE_DESCRIPTION: &str = "model.infrastructurenode.description";
    /// An infrastructure node is missing a technology.
    pub const INFRASTRUCTURENODE_TECHNOLOGY: &str = "model.infrastructurenode.technology";
    /// An element carries no relationships (deployment nodes exempt).
    pub const ELEMENT_DISCONNECTED: &str = "model.element.disconnected";
    /// An element appears on no views.
    pub const ELEMENT_NOVIEW: &str = "model.element.noview";
    /// A relationship is missing a description.
    pub const RELATIONSHIP_DESCRIPTION: &str = "model.relationship.description";
    /// A relationship is missing a technology.
    pub const RELATIONSHIP_TECHNOLOGY: &str = "model.relationship.technology";
    /// The workspace has no views.
    pub const VIEWS_EMPTY: &str = "views.empty";
    /// A view key was generated automatically.
    pub const VIEW_KEY: &str = "views.view.key";
    /// A view contains no elements.
    pub const VIEW_EMPTY: &str = "views.view.empty";
    /// A generated-key view has no automatic layout.
    pub const VIEW_LAYOUT: &str = "views.view.layout";
    /// An element style hides metadata.
    pub const STYLE_ELEMENT_METADATA: &str = "views.styles.element.metadata";
    /// Documentation embeds a missing view, or one via a generated key
    /// (shared string, both checks).
    pub const DOCUMENTATION_EMBEDDEDVIEW: &str = "documentation.embeddedview";

    /// The whole catalog, for `--help` and the CLI's filter validation
    /// (stable order: the run order).
    pub const ALL: &[&str] = &[
        WORKSPACE_TOOLING,
        WORKSPACE_SCOPE,
        DOCUMENTATION_EMBEDDEDVIEW,
        MODEL_EMPTY,
        PERSON_DESCRIPTION,
        SOFTWARESYSTEM_DESCRIPTION,
        SOFTWARESYSTEM_DOCUMENTATION,
        SOFTWARESYSTEM_DECISIONS,
        CONTAINER_DESCRIPTION,
        CONTAINER_TECHNOLOGY,
        COMPONENT_DESCRIPTION,
        COMPONENT_TECHNOLOGY,
        DEPLOYMENTNODE_DESCRIPTION,
        DEPLOYMENTNODE_TECHNOLOGY,
        DEPLOYMENTNODE_EMPTY,
        INFRASTRUCTURENODE_DESCRIPTION,
        INFRASTRUCTURENODE_TECHNOLOGY,
        ELEMENT_DISCONNECTED,
        ELEMENT_NOVIEW,
        RELATIONSHIP_DESCRIPTION,
        RELATIONSHIP_TECHNOLOGY,
        VIEWS_EMPTY,
        VIEW_KEY,
        VIEW_EMPTY,
        VIEW_LAYOUT,
        STYLE_ELEMENT_METADATA,
    ];
}
