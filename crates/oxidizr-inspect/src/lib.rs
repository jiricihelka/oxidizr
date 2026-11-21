//! `oxidizr-inspect` — advisory inspections ported from
//! `structurizr-inspection` (P4 §P4-M1, P4-D7; reference @ `ad34036`).
//!
//! Pure model logic over [`oxidizr_core::Workspace`]: loading, the report
//! formats and the CLI live upstream (`oxidizr-api` / `oxidizr-cli`) per
//! the T0-d bodies-in-API invariant, and nothing here can touch the JSON
//! byte contract — inspections are ADVISORY (P4-D7): they never change
//! bytes, validation or the parser.
//!
//! The port mirrors `DefaultInspector`: run ORDER is the report contract —
//! workspace, then model (elements in numeric-id order, each element's
//! OUTGOING relationships in numeric-id order, per-kind blocks exactly as
//! the reference sequences them), then views per kind, then element
//! styles. Violations keep that order (no sort).
//!
//! Severity is 100% property-driven (P4-V2, [`strategy`]): EVERY
//! inspection defaults to [`Severity::Error`]; the workspace (or any
//! holder from an element up) may re-rank or silence one via
//! `structurizr.inspection.<dotted type path>` keys, and
//! `structurizr.inspection = "false"` skips the whole run.
//!
//! The four `structurizr.inspection.{error,warning,info,ignore}` counter
//! properties the reference app writes after a run are HARNESS artifacts
//! (see `agents/plan/COMPAT.md` / the oracle strip rule) — this crate
//! never computes or stores them.

mod run;
mod severity;
mod strategy;
mod violation;

pub use run::inspect;
pub use severity::{ParseThresholdError, Severity};
pub use violation::Violation;

/// Every inspection `getType()` string (`Inspection.getType()`), keyed by
/// the port name. THREE distinct Java classes share `workspace.scope`
/// (`WorkspaceScopeInspection`, `MultipleSoftwareSystemsDetailedInspection`,
/// and the two multi-system view checks share it too — see
/// `agents/plan/build/PHASE_4.md` §P4-M1); both documentation inspections
/// share `documentation.embeddedview`. The value is the property-key stem
/// AND the `oxidizr inspect --only/--except` filter value.
pub mod types {
    /// `WorkspaceToolingInspection`.
    pub const WORKSPACE_TOOLING: &str = "workspace.tooling";
    /// `WorkspaceScopeInspection`, `MultipleSoftwareSystemsDetailedInspection`,
    /// `SystemContextViewsForMultipleSoftwareSystemsInspection`,
    /// `ContainerViewsForMultipleSoftwareSystemsInspection`.
    pub const WORKSPACE_SCOPE: &str = "workspace.scope";
    /// `EmptyModelInspection`.
    pub const MODEL_EMPTY: &str = "model.empty";
    /// `PersonDescriptionInspection`.
    pub const PERSON_DESCRIPTION: &str = "model.person.description";
    /// `SoftwareSystemDescriptionInspection`.
    pub const SOFTWARESYSTEM_DESCRIPTION: &str = "model.softwaresystem.description";
    /// `SoftwareSystemDocumentationInspection`.
    pub const SOFTWARESYSTEM_DOCUMENTATION: &str = "model.softwaresystem.documentation";
    /// `SoftwareSystemDecisionsInspection`.
    pub const SOFTWARESYSTEM_DECISIONS: &str = "model.softwaresystem.decisions";
    /// `ContainerDescriptionInspection`.
    pub const CONTAINER_DESCRIPTION: &str = "model.container.description";
    /// `ContainerTechnologyInspection`.
    pub const CONTAINER_TECHNOLOGY: &str = "model.container.technology";
    /// `ComponentDescriptionInspection`.
    pub const COMPONENT_DESCRIPTION: &str = "model.component.description";
    /// `ComponentTechnologyInspection`.
    pub const COMPONENT_TECHNOLOGY: &str = "model.component.technology";
    /// `DeploymentNodeDescriptionInspection`.
    pub const DEPLOYMENTNODE_DESCRIPTION: &str = "model.deploymentnode.description";
    /// `DeploymentNodeTechnologyInspection`.
    pub const DEPLOYMENTNODE_TECHNOLOGY: &str = "model.deploymentnode.technology";
    /// `EmptyDeploymentNodeInspection`.
    pub const DEPLOYMENTNODE_EMPTY: &str = "model.deploymentnode.empty";
    /// `InfrastructureNodeDescriptionInspection`.
    pub const INFRASTRUCTURENODE_DESCRIPTION: &str = "model.infrastructurenode.description";
    /// `InfrastructureNodeTechnologyInspection`.
    pub const INFRASTRUCTURENODE_TECHNOLOGY: &str = "model.infrastructurenode.technology";
    /// `DisconnectedElementInspection`.
    pub const ELEMENT_DISCONNECTED: &str = "model.element.disconnected";
    /// `ElementNotIncludedInAnyViewsInspection`.
    pub const ELEMENT_NOVIEW: &str = "model.element.noview";
    /// `RelationshipDescriptionInspection`.
    pub const RELATIONSHIP_DESCRIPTION: &str = "model.relationship.description";
    /// `RelationshipTechnologyInspection`.
    pub const RELATIONSHIP_TECHNOLOGY: &str = "model.relationship.technology";
    /// `EmptyViewsInspection`.
    pub const VIEWS_EMPTY: &str = "views.empty";
    /// `GeneratedKeyInspection`.
    pub const VIEW_KEY: &str = "views.view.key";
    /// `EmptyViewInspection`.
    pub const VIEW_EMPTY: &str = "views.view.empty";
    /// `ManualLayoutInspection`.
    pub const VIEW_LAYOUT: &str = "views.view.layout";
    /// `ElementStyleMetadataInspection`.
    pub const STYLE_ELEMENT_METADATA: &str = "views.styles.element.metadata";
    /// `EmbeddedViewMissingInspection` and `EmbeddedViewWithGeneratedKeyInspection`.
    pub const DOCUMENTATION_EMBEDDEDVIEW: &str = "documentation.embeddedview";

    /// The whole catalog, for `--help` and the CLI's filter validation
    /// (stable order: the run order of `DefaultInspector`).
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
