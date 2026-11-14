//! Checked-operation errors for [`crate::builder::WorkspaceBuilder`].
//!
//! Hand-rolled (no `thiserror` — `oxidizr-core` is dependency-free, P1-D1).
//! Variants are STRUCTURED: the DSL/JSON layers map them to `oxidizr-diag`
//! diagnostics with OXZ codes and reference-style text (D1); the reference's
//! original message is carried along where it exists, since those texts are
//! what users compared against for a decade.

use std::fmt;

use crate::ids::Id;

/// A rejected builder operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoreError {
    /// A name is required but was null/empty (reference
    /// `IllegalArgumentException("A name must be specified.")`).
    NameRequired,
    /// A view key is required but was null/empty.
    ViewKeyRequired,
    /// Top-level (person/system/custom) name collision — reference
    /// "A top-level element named 'X' already exists."
    DuplicateTopLevelName {
        /// The colliding name.
        name: String,
    },
    /// Container name collision within its system.
    DuplicateContainerName {
        /// The colliding name.
        name: String,
        /// Parent software system name.
        system: String,
    },
    /// Component name collision within its container.
    DuplicateComponentName {
        /// The colliding name.
        name: String,
        /// Parent container name.
        container: String,
    },
    /// Top-level deployment node name collision within one environment.
    DuplicateDeploymentNodeName {
        /// The colliding name.
        name: String,
        /// Environment.
        environment: String,
    },
    /// Child deployment node name collision within the parent node.
    DuplicateChildDeploymentNodeName {
        /// The colliding name.
        name: String,
        /// Parent node name.
        parent: String,
    },
    /// Infrastructure node name collision within the parent node (reference
    /// "A deployment/infrastructure node named 'X' already exists." — the two
    /// kinds share the parent's child-name space).
    DuplicateInfrastructureNodeName {
        /// The colliding name.
        name: String,
        /// Parent node name.
        parent: String,
    },
    /// View key already taken — reference "A view with the key X already exists."
    DuplicateViewKey {
        /// The colliding key.
        key: String,
    },
    /// An element id did not resolve.
    ElementNotFound {
        /// The unknown id.
        id: Id,
    },
    /// The element resolved, but not to the required kind.
    WrongElementKind {
        /// The id.
        id: Id,
        /// Human label of the required kind.
        expected: &'static str,
    },
    /// A relationship id did not resolve.
    RelationshipNotFound {
        /// The unknown id.
        id: Id,
    },
    /// A deployment node id did not resolve.
    DeploymentNodeNotFound {
        /// The unknown id.
        id: Id,
    },
    /// `Model.addRelationship` returned null (reference `Element.has` matches
    /// source+destination+description) — the DSL layer turns this into
    /// `A relationship between "A" and "B" already exists` (canonical names).
    DuplicateRelationship {
        /// Canonical name of the source element.
        source: String,
        /// Canonical name of the destination element.
        destination: String,
    },
    /// Reference `IllegalArgumentException("Relationships cannot be added
    /// between parents and children.")` in `Model.addRelationship`.
    ParentChildRelationship,
    /// Reference `ElementNotPermittedInViewException` — message texts are the
    /// reference strings verbatim (the DSL content parsers CATCH this one).
    ElementNotPermittedInView(String),
    /// Reference `IllegalArgumentException("The element named 'X' cannot be
    /// removed from this view.")` in `ModelView.removeElement`.
    ElementCannotBeRemoved {
        /// Element name.
        name: String,
    },
    /// Dynamic-view `add`: no matching model relationship. Reference texts
    /// `A relationship between A and B does not exist in model.` (and the
    /// `… with technology T …` variant).
    DynamicRelationshipMissing {
        /// Source element name.
        source: String,
        /// Destination element name.
        destination: String,
        /// Technology filter, when one was given.
        technology: Option<String>,
    },
    /// Reference `IllegalArgumentException("None of the specified elements
    /// exist in this view.")` in `addAnimation`.
    NoAnimatableElements,
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::NameRequired => f.write_str("A name must be specified."),
            CoreError::ViewKeyRequired => f.write_str("A key must be specified."),
            CoreError::DuplicateTopLevelName { name } => {
                write!(f, "A top-level element named '{name}' already exists.")
            }
            CoreError::DuplicateContainerName { name, system } => write!(
                f,
                "A container named '{name}' already exists for this software system (in '{system}')."
            ),
            CoreError::DuplicateComponentName { name, container } => write!(
                f,
                "A component named '{name}' already exists for this container (in '{container}')."
            ),
            CoreError::DuplicateDeploymentNodeName { name, environment } => write!(
                f,
                "A top-level deployment node named '{name}' already exists for the environment named '{environment}'."
            ),
            CoreError::DuplicateChildDeploymentNodeName { name, parent } => write!(
                f,
                "A deployment node named '{name}' already exists within '{parent}'."
            ),
            CoreError::DuplicateInfrastructureNodeName { name, parent } => {
                write!(
                    f,
                    "A deployment/infrastructure node named '{name}' already exists (in '{parent}')."
                )
            }
            CoreError::DuplicateViewKey { key } => {
                write!(f, "A view with the key {key} already exists.")
            }
            CoreError::ElementNotFound { id } => {
                write!(f, "There is no element with the ID \"{id}\".")
            }
            CoreError::WrongElementKind { id, expected } => {
                write!(f, "The element with the ID \"{id}\" is not a {expected}.")
            }
            CoreError::RelationshipNotFound { id } => {
                write!(f, "There is no relationship with the ID \"{id}\".")
            }
            CoreError::DeploymentNodeNotFound { id } => {
                write!(f, "There is no deployment node with the ID \"{id}\".")
            }
            CoreError::DuplicateRelationship {
                source,
                destination,
            } => write!(
                f,
                "A relationship between \"{source}\" and \"{destination}\" already exists"
            ),
            CoreError::ParentChildRelationship => {
                f.write_str("Relationships cannot be added between parents and children.")
            }
            CoreError::ElementNotPermittedInView(msg) => f.write_str(msg),
            CoreError::ElementCannotBeRemoved { name } => write!(
                f,
                "The element named '{name}' cannot be removed from this view."
            ),
            CoreError::DynamicRelationshipMissing {
                source,
                destination,
                technology: Some(tech),
            } => write!(
                f,
                "A relationship between {source} and {destination} with technology {tech} does not exist in model."
            ),
            CoreError::DynamicRelationshipMissing {
                source,
                destination,
                technology: None,
            } => write!(
                f,
                "A relationship between {source} and {destination} does not exist in model."
            ),
            CoreError::NoAnimatableElements => {
                f.write_str("None of the specified elements exist in this view.")
            }
        }
    }
}

impl std::error::Error for CoreError {}
