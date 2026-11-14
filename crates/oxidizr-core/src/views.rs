//! Views (`com.structurizr.view`): the view set and every view type, the
//! per-view element/relationship entry views, automatic layout, and the view
//! configuration (styles / themes / terminology).
//!
//! v6 shape notes (verified @ `ad34036`; several `plan/JSON-FORMAT.md` "verify"
//! items resolved here): views have NO `background`; `ElementView` is exactly
//! `{id,x,y}` (x,y serialized only when nonzero — `NON_EMPTY` on primitive int);
//! included/excluded lists are GONE (the entry arrays ARE the content);
//! `ImageView` extends `View` (not `ModelView`); animations exist only on
//! `DeploymentView` and `CustomView`; `FilteredView.tags` is a `TreeSet<String>`
//! (alphabetical serialization — an M3 write-catalog rule).

use std::collections::BTreeMap;

use crate::ids::Id;

/// Common fields of every view (`view/View`). `order` is a reference `int`
/// (views are TreeSet-ordered by `(order, key-case-insensitive)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewBase {
    /// View key (`"SystemLandscape-001"`, unique workspace-wide).
    pub key: String,
    /// Sort order.
    pub order: i32,
    /// Optional title.
    pub title: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// Free-form properties.
    pub properties: BTreeMap<String, String>,
}

impl ViewBase {
    /// A new ordered view base.
    #[must_use]
    pub fn new(key: impl Into<String>, order: i32) -> ViewBase {
        ViewBase {
            key: key.into(),
            order,
            title: None,
            description: None,
            properties: BTreeMap::new(),
        }
    }
}

/// The entry-arrays part shared by all `ModelView` subclasses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewContent {
    /// Element views: the inclusion list AND geometry.
    pub elements: Vec<ElementView>,
    /// Relationship views.
    pub relationships: Vec<RelationshipView>,
    /// Optional paper size (export hint).
    pub paper_size: Option<PaperSize>,
    /// Rendered dimensions (read-tolerated, DSL cannot set).
    pub dimensions: Option<Dimensions>,
    /// `Some` ⇒ automatic layout enabled.
    pub automatic_layout: Option<AutomaticLayout>,
}

/// A system landscape view (`view/SystemLandscapeView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemLandscapeView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
}

/// A system context view (`view/SystemContextView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemContextView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Scoped software system (JSON `softwareSystemId`).
    pub software_system_id: Id,
    /// JSON `enterpriseBoundaryVisible` (primitive; written when true).
    pub enterprise_boundary_visible: bool,
}

/// A container view (`view/ContainerView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Scoped software system (JSON `softwareSystemId`).
    pub software_system_id: Id,
    /// JSON `externalSoftwareSystemBoundariesVisible`.
    pub external_software_system_boundaries_visible: bool,
}

/// A component view (`view/ComponentView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Scoped container (JSON `containerId`).
    pub container_id: Id,
    /// JSON `externalContainerBoundariesVisible`.
    pub external_container_boundaries_visible: bool,
}

/// A dynamic view (`view/DynamicView`). The entry arrays are ORDERED
/// (`RelationshipView.order` numbers the steps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Scope element, if any (JSON `elementId`).
    pub element_id: Option<Id>,
    /// Deployment-scope dynamic views (JSON `deploymentEnvironment`).
    pub deployment_environment: Option<String>,
    /// JSON `externalBoundariesVisible` (primitive; written when true).
    pub external_boundaries_visible: bool,
}

/// A deployment view (`view/DeploymentView`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Environment this view renders.
    pub environment: String,
    /// Optional software-system scope (JSON `softwareSystemId`).
    pub software_system_id: Option<Id>,
    /// Animation steps.
    pub animations: Vec<Animation>,
}

/// A custom view (`view/CustomView`) — the target of DSL `image { … }` blocks
/// too (they produce `ImageView`s; `customViews` exist in Java but not the
/// `OpenAPI` schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Animation steps.
    pub animations: Vec<Animation>,
}

/// A filtered view (`view/FilteredView`): a saved filter over a base view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredView {
    /// Common fields.
    pub base: ViewBase,
    /// The base view key (JSON `baseViewKey`).
    pub base_view_key: String,
    /// Include/exclude.
    pub mode: FilterMode,
    /// Filter terms (serialized as the JSON `tags` ARRAY; reference stores a
    /// `TreeSet`, i.e. alphabetical on write).
    pub tags: Vec<String>,
    /// Legacy `filter` expression string — read-tolerance only, never written.
    pub legacy_filter: Option<String>,
}

/// An image view (`view/ImageView`) — raw diagram text (PlantUML/Mermaid/…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageView {
    /// Common fields.
    pub base: ViewBase,
    /// Associated element, if any (JSON `elementId`).
    pub element_id: Option<Id>,
    /// Diagram content.
    pub content: Option<String>,
    /// Light-mode content (`contentLight`, Java-only ★).
    pub content_light: Option<String>,
    /// Dark-mode content (`contentDark`, Java-only ★).
    pub content_dark: Option<String>,
    /// Content type (e.g. `PlantUML`, `Mermaid`).
    pub content_type: Option<String>,
}

/// One included element (JSON `elements[]` entry). `x`/`y` are reference
/// primitive ints: serialized only when nonzero (V-2 quirk), and
/// `x == y == 0` round-trips to an omitted pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementView {
    /// Referenced element id.
    pub id: Id,
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

/// One included relationship (JSON `relationships[]` entry). Dynamic views
/// populate `order`/`description`/`technology`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipView {
    /// Referenced relationship id.
    pub id: Id,
    /// Optional description override.
    pub description: Option<String>,
    /// Optional URL (Java-only ★).
    pub url: Option<String>,
    /// Dynamic-step order number (reference stores it as a STRING).
    pub order: Option<String>,
    /// Async response marker (`response`).
    pub response: Option<bool>,
    /// Optional technology override (DSL dynamic steps; not on the Java
    /// `RelationshipView` JSON — verify at M3).
    pub technology: Option<String>,
    /// Free-form properties.
    pub properties: BTreeMap<String, String>,
    /// Waypoints (`vertices` in v6 JSON).
    pub vertices: Vec<Vertex>,
    /// Routing override.
    pub routing: Option<Routing>,
    /// Jump (`jump`).
    pub jump: Option<bool>,
    /// Start position along the source edge (`position`, 0-100).
    pub position: Option<i32>,
}

/// A relationship waypoint (`view/Vertex`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vertex {
    /// X coordinate.
    pub x: i32,
    /// Y coordinate.
    pub y: i32,
}

/// An animation step (`view/Animation`): ids are stored as STRINGS and the
/// reference keeps them in `TreeSet`s (lexicographic on write — M3 catalog).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Animation {
    /// Step number.
    pub order: i32,
    /// Element ids revealed at this step.
    pub element_ids: Vec<String>,
    /// Relationship ids revealed at this step.
    pub relationship_ids: Vec<String>,
}

/// Automatic layout request (`view/AutomaticLayout`); presence enables layout.
/// Defaults verified from the reference constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomaticLayout {
    /// Rank direction (default `LeftRight`).
    pub rank_direction: RankDirection,
    /// Rank separation (default 100).
    pub rank_separation: i32,
    /// Node separation (default 50).
    pub node_separation: i32,
    /// Edge separation (default 50).
    pub edge_separation: i32,
    /// Route edge vertices (default true).
    pub vertices: bool,
}

impl Default for AutomaticLayout {
    fn default() -> Self {
        AutomaticLayout {
            rank_direction: RankDirection::LeftRight,
            rank_separation: 100,
            node_separation: 50,
            edge_separation: 50,
            vertices: true,
        }
    }
}

/// Rendered dimensions (`view/Dimensions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    /// Width px.
    pub width: i32,
    /// Height px.
    pub height: i32,
}

/// `view/RankDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RankDirection {
    TopBottom,
    BottomTop,
    #[default]
    LeftRight,
    RightLeft,
}

/// `view/Routing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Routing {
    Direct,
    Curved,
    Orthogonal,
}

/// `view/LineStyle` (relationship styles) and `view/Border` (element styles
/// share the same three spellings).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
    Dotted,
}

/// `view/Shape`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Box,
    RoundedBox,
    Circle,
    Ellipse,
    Hexagon,
    Diamond,
    Cylinder,
    Bucket,
    Pipe,
    Person,
    Robot,
    Folder,
    WebBrowser,
    Window,
    Terminal,
    Shell,
    MobileDevicePortrait,
    MobileDeviceLandscape,
    Component,
}

/// `view/IconPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconPosition {
    Top,
    Bottom,
    Left,
}

/// `view/ColorScheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

/// `view/FilterMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMode {
    Include,
    Exclude,
}

/// `view/ViewSortOrder`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewSortOrder {
    Default,
    Type,
    Key,
}

/// `view/MetadataSymbols`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataSymbols {
    SquareBrackets,
    RoundBrackets,
    CurlyBrackets,
    AngleBrackets,
    DoubleAngleBrackets,
    None,
}

/// `view/PaperSize` (JSON writes the enum name, e.g. `A4_Landscape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaperSize {
    A6Portrait,
    A6Landscape,
    A5Portrait,
    A5Landscape,
    A4Portrait,
    A4Landscape,
    A3Portrait,
    A3Landscape,
    A2Portrait,
    A2Landscape,
    A1Portrait,
    A1Landscape,
    A0Portrait,
    A0Landscape,
    LetterPortrait,
    LetterLandscape,
    LegalPortrait,
    LegalLandscape,
    Slide43,
    Slide169,
    Slide1610,
}

/// An element style (`view/ElementStyle`), matched by `tag`. All paint fields
/// are Option (`NON_EMPTY`: only set fields serialize).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementStyle {
    /// Tag this style applies to.
    pub tag: String,
    /// `colorScheme` (Light/Dark; theme mechanism).
    pub color_scheme: Option<ColorScheme>,
    /// Optional width px.
    pub width: Option<i32>,
    /// Optional height px.
    pub height: Option<i32>,
    /// Background `#rrggbb`.
    pub background: Option<String>,
    /// Stroke `#rrggbb`.
    pub stroke: Option<String>,
    /// Stroke width px.
    pub stroke_width: Option<i32>,
    /// Text color (JSON key `color`; DSL also accepts `colour` — verified the
    /// JSON spelling is always `color`).
    pub color: Option<String>,
    /// Font size px.
    pub font_size: Option<i32>,
    /// Shape.
    pub shape: Option<Shape>,
    /// Icon URL/path.
    pub icon: Option<String>,
    /// Icon position (★ not in the `OpenAPI` schema).
    pub icon_position: Option<IconPosition>,
    /// Border style.
    pub border: Option<LineStyle>,
    /// Opacity percent.
    pub opacity: Option<i32>,
    /// Show metadata (reference `Boolean metadata` — default null).
    pub metadata: Option<bool>,
    /// Show description.
    pub description: Option<bool>,
}

/// A relationship style (`view/RelationshipStyle`), matched by `tag`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipStyle {
    /// Tag this style applies to.
    pub tag: String,
    /// `colorScheme`.
    pub color_scheme: Option<ColorScheme>,
    /// Line thickness.
    pub thickness: Option<i32>,
    /// Line color (`color`).
    pub color: Option<String>,
    /// Font size px.
    pub font_size: Option<i32>,
    /// Style width px.
    pub width: Option<i32>,
    /// Legacy `dashed` boolean (superseded by `style`).
    pub dashed: Option<bool>,
    /// Line style.
    pub style: Option<LineStyle>,
    /// Routing.
    pub routing: Option<Routing>,
    /// Jump.
    pub jump: Option<bool>,
    /// Position percent (0-100).
    pub position: Option<i32>,
    /// Opacity percent.
    pub opacity: Option<i32>,
    /// Show metadata.
    pub metadata: Option<bool>,
    /// Show description.
    pub description: Option<bool>,
}

/// Element + relationship styles (`view/Styles`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Styles {
    /// Element styles, definition order.
    pub element_styles: Vec<ElementStyle>,
    /// Relationship styles, definition order.
    pub relationship_styles: Vec<RelationshipStyle>,
}

/// Terminology overrides (`view/Terminology`); every field defaults to `""`
/// (`NON_EMPTY` omits unset ones).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Terminology {
    /// Legacy `enterprise` (schema-era read; Java field exists).
    pub enterprise: Option<String>,
    /// `person`.
    pub person: Option<String>,
    /// `softwareSystem`.
    pub software_system: Option<String>,
    /// `container`.
    pub container: Option<String>,
    /// `component`.
    pub component: Option<String>,
    /// `code`.
    pub code: Option<String>,
    /// `deploymentNode`.
    pub deployment_node: Option<String>,
    /// `infrastructureNode` (★ not in the schema).
    pub infrastructure_node: Option<String>,
    /// `relationship`.
    pub relationship: Option<String>,
}

/// The `views.configuration` object (`view/Configuration`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewConfiguration {
    /// Styles.
    pub styles: Styles,
    /// Theme URLs / names, in order.
    pub themes: Vec<String>,
    /// Terminology overrides.
    pub terminology: Terminology,
    /// Metadata symbol style (`metadataSymbols`).
    pub metadata_symbols: Option<MetadataSymbols>,
    /// Default view key.
    pub default_view: Option<String>,
    /// Last-saved view key.
    pub last_saved_view: Option<String>,
    /// View sort order.
    pub view_sort_order: Option<ViewSortOrder>,
    /// Free-form properties.
    pub properties: BTreeMap<String, String>,
}

/// All views onto the model (`view/ViewSet`). Each collection preserves
/// insertion order; the reference keeps `TreeSet`s ordered by
/// `(order, key-ignore-case)` — the M3 write catalog sorts on output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ViewSet {
    /// Custom views (★ in Java, absent from the schema).
    pub custom_views: Vec<CustomView>,
    /// System landscape views.
    pub system_landscape_views: Vec<SystemLandscapeView>,
    /// System context views (JSON read alias `enterpriseContextViews`).
    pub system_context_views: Vec<SystemContextView>,
    /// Container views.
    pub container_views: Vec<ContainerView>,
    /// Component views.
    pub component_views: Vec<ComponentView>,
    /// Dynamic views.
    pub dynamic_views: Vec<DynamicView>,
    /// Deployment views.
    pub deployment_views: Vec<DeploymentView>,
    /// Filtered views.
    pub filtered_views: Vec<FilteredView>,
    /// Image views.
    pub image_views: Vec<ImageView>,
    /// Styles, themes, terminology.
    pub configuration: ViewConfiguration,
}

impl ViewSet {
    /// Every view (key, kind label) in the reference `getViews()` traversal
    /// order used for key-uniqueness validation: the union `TreeSet` ordered by
    /// `(order, key lowercased)` is what `checkViewKeysAreUnique` iterates —
    /// [`crate::validate`] reproduces that ordering.
    #[must_use]
    pub fn all_views(&self) -> Vec<View<'_>> {
        let mut out = Vec::new();
        for v in &self.custom_views {
            out.push(View::Custom(v));
        }
        for v in &self.system_landscape_views {
            out.push(View::Landscape(v));
        }
        for v in &self.system_context_views {
            out.push(View::Context(v));
        }
        for v in &self.container_views {
            out.push(View::Container(v));
        }
        for v in &self.component_views {
            out.push(View::Component(v));
        }
        for v in &self.dynamic_views {
            out.push(View::Dynamic(v));
        }
        for v in &self.deployment_views {
            out.push(View::Deployment(v));
        }
        for v in &self.filtered_views {
            out.push(View::Filtered(v));
        }
        for v in &self.image_views {
            out.push(View::Image(v));
        }
        out
    }
}

/// A borrowed view of any kind.
#[derive(Debug, Clone, Copy)]
pub enum View<'a> {
    /// Custom view.
    Custom(&'a CustomView),
    /// System landscape view.
    Landscape(&'a SystemLandscapeView),
    /// System context view.
    Context(&'a SystemContextView),
    /// Container view.
    Container(&'a ContainerView),
    /// Component view.
    Component(&'a ComponentView),
    /// Dynamic view.
    Dynamic(&'a DynamicView),
    /// Deployment view.
    Deployment(&'a DeploymentView),
    /// Filtered view.
    Filtered(&'a FilteredView),
    /// Image view.
    Image(&'a ImageView),
}

impl View<'_> {
    /// The view's common base.
    #[must_use]
    pub const fn base(&self) -> &ViewBase {
        match self {
            View::Custom(v) => &v.base,
            View::Landscape(v) => &v.base,
            View::Context(v) => &v.base,
            View::Container(v) => &v.base,
            View::Component(v) => &v.base,
            View::Dynamic(v) => &v.base,
            View::Deployment(v) => &v.base,
            View::Filtered(v) => &v.base,
            View::Image(v) => &v.base,
        }
    }

    /// The view key.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.base().key
    }

    /// The view's `order`.
    #[must_use]
    pub const fn order(&self) -> i32 {
        self.base().order
    }

    /// A human label for diagnostics ("system context view", …).
    #[must_use]
    pub const fn kind_label(&self) -> &'static str {
        match self {
            View::Custom(_) => "custom view",
            View::Landscape(_) => "system landscape view",
            View::Context(_) => "system context view",
            View::Container(_) => "container view",
            View::Component(_) => "component view",
            View::Dynamic(_) => "dynamic view",
            View::Deployment(_) => "deployment view",
            View::Filtered(_) => "filtered view",
            View::Image(_) => "image view",
        }
    }

    /// Model-view content, if this kind has entries (`ImageView` and
    /// `FilteredView` do not).
    #[must_use]
    pub fn content(&self) -> Option<&ViewContent> {
        match self {
            View::Custom(v) => Some(&v.content),
            View::Landscape(v) => Some(&v.content),
            View::Context(v) => Some(&v.content),
            View::Container(v) => Some(&v.content),
            View::Component(v) => Some(&v.content),
            View::Dynamic(v) => Some(&v.content),
            View::Deployment(v) => Some(&v.content),
            View::Filtered(_) | View::Image(_) => None,
        }
    }
}
