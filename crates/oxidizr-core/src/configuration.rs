//! Workspace-level configuration (`com.structurizr.configuration`) and the
//! [`Workspace`] root.
//!
//! v6.2.4 truth: `WorkspaceConfiguration` is `{scope?, visibility?, users?}` —
//! the `branding`/`styles`/`terminology` blocks seen inside a *top-level*
//! `configuration` in v5-era JSONs are read-IGNORED (the live styles home is
//! `views.configuration`); `users` is legacy and never written (read
//! tolerance only), so it is deliberately absent from the model.

/// Workspace scope (`configuration/WorkspaceScope`, drives
/// `WorkspaceScopeValidatorFactory`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceScope {
    /// `"undefined"` — scope-free workspace.
    Undefined,
    /// `"landscape"` — landscape-scope validator.
    Landscape,
    /// `"softwareSystem"` — software-system-scope validator.
    SoftwareSystem,
}

impl WorkspaceScope {
    /// JSON spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            WorkspaceScope::Undefined => "undefined",
            WorkspaceScope::Landscape => "landscape",
            WorkspaceScope::SoftwareSystem => "softwareSystem",
        }
    }

    /// Parse the JSON spelling (unknown values: the reference `Jackson`
    /// deserializer FAILS; loaders map that to a structural error).
    #[must_use]
    pub fn parse(s: &str) -> Option<WorkspaceScope> {
        match s {
            "undefined" => Some(WorkspaceScope::Undefined),
            "landscape" => Some(WorkspaceScope::Landscape),
            "softwareSystem" => Some(WorkspaceScope::SoftwareSystem),
            _ => None,
        }
    }
}

/// Workspace visibility (`configuration/Visibility`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// `"private"`.
    Private,
    /// `"public"`.
    Public,
}

/// The workspace `configuration` object (`configuration/WorkspaceConfiguration`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceConfiguration {
    /// Workspace scope (drives the scope validator on load).
    pub scope: Option<WorkspaceScope>,
    /// Visibility (written only when set).
    pub visibility: Option<Visibility>,
}

/// A Structurizr workspace: `model` + `views` + `documentation` + top-level
/// metadata/configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Workspace {
    /// Numeric workspace id (JSON `id`, a NUMBER — the only numeric id).
    pub id: i64,
    /// Workspace name.
    pub name: Option<String>,
    /// Workspace description.
    pub description: Option<String>,
    /// Arbitrary version string.
    pub version: Option<String>,
    /// `lastModifiedDate`, epoch SECONDS (serialized `yyyy-MM-dd'T'HH:mm:ss'Z'`
    /// UTC; round-trips when present — nothing is stamped on write).
    pub last_modified_date: Option<i64>,
    /// `lastModifiedUser`.
    pub last_modified_user: Option<String>,
    /// `lastModifiedAgent`.
    pub last_modified_agent: Option<String>,
    /// Base64 thumbnail.
    pub thumbnail: Option<String>,
    /// Top-level properties (`structurizr.dsl` lives here when portable).
    pub properties: std::collections::BTreeMap<String, String>,
    /// The architecture model.
    pub model: crate::model::Model,
    /// The views.
    pub views: crate::views::ViewSet,
    /// Documentation.
    pub documentation: crate::docs::Documentation,
    /// Configuration.
    pub configuration: WorkspaceConfiguration,
}
