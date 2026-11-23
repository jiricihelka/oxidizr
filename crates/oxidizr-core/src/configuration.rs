//! Workspace-level configuration (`com.structurizr.configuration`) and the
//! [`Workspace`] root.
//!
//! v6.2.4 truth: `WorkspaceConfiguration` is `{scope?, visibility?, users?}` —
//! the `branding`/`styles`/`terminology` blocks seen inside a *top-level*
//! `configuration` in v5-era JSONs are read-IGNORED (the live styles home is
//! `views.configuration`); `users` is legacy but DOES round-trip (the
//! official reader binds even package-private setters).

/// Workspace scope;
/// the official model has exactly two constants —
/// "undefined" is represented by `None`, not an enum value; the JSON names
/// are the constant names as the official serializer writes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceScope {
    /// `"Landscape"` — landscape-scope validator.
    Landscape,
    /// `"SoftwareSystem"` — software-system-scope validator.
    SoftwareSystem,
}

impl WorkspaceScope {
    /// JSON spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            WorkspaceScope::Landscape => "Landscape",
            WorkspaceScope::SoftwareSystem => "SoftwareSystem",
        }
    }

    /// Parse the JSON spelling (exact case; unknown enum values deserialize to
    /// null — `READ_UNKNOWN_ENUM_VALUES_AS_NULL`).
    #[must_use]
    pub fn parse(s: &str) -> Option<WorkspaceScope> {
        match s {
            "Landscape" => Some(WorkspaceScope::Landscape),
            "SoftwareSystem" => Some(WorkspaceScope::SoftwareSystem),
            _ => None,
        }
    }
}

/// Workspace visibility (JSON = the official constant names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    /// `"Private"`.
    Private,
    /// `"Public"`.
    Public,
}

impl Visibility {
    /// JSON spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Visibility::Private => "Private",
            Visibility::Public => "Public",
        }
    }

    /// Parse the JSON spelling (exact case).
    #[must_use]
    pub fn parse(s: &str) -> Option<Visibility> {
        match s {
            "Private" => Some(Visibility::Private),
            "Public" => Some(Visibility::Public),
            _ => None,
        }
    }
}

/// Access role for a legacy `configuration.users` entry
/// (`configuration/Role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// `"ReadWrite"`.
    ReadWrite,
    /// `"ReadOnly"`.
    ReadOnly,
}

impl Role {
    /// JSON spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::ReadWrite => "ReadWrite",
            Role::ReadOnly => "ReadOnly",
        }
    }

    /// Parse the JSON spelling (exact case).
    #[must_use]
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "ReadWrite" => Some(Role::ReadWrite),
            "ReadOnly" => Some(Role::ReadOnly),
            _ => None,
        }
    }
}

/// A legacy `configuration.users` entry; the format keeps these ordered by
/// `username`. Round-trips in both directions: the official reader binds
/// even the package-private setter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Username.
    pub username: String,
    /// Access role — nullable bean field (unknown/absent JSON `role` binds
    /// null; the writer then omits the key).
    pub role: Option<Role>,
}

/// The workspace `configuration` object (`configuration/WorkspaceConfiguration`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspaceConfiguration {
    /// Workspace scope (drives the scope validator on load).
    pub scope: Option<WorkspaceScope>,
    /// Visibility (written only when set).
    pub visibility: Option<Visibility>,
    /// Legacy user list (see [`User`]).
    pub users: Vec<User>,
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
    pub properties: crate::properties::Properties,
    /// The architecture model.
    pub model: crate::model::Model,
    /// The views.
    pub views: crate::views::ViewSet,
    /// Documentation.
    pub documentation: crate::docs::Documentation,
    /// Configuration.
    pub configuration: WorkspaceConfiguration,
}
