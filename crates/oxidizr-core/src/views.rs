//! Views (`com.structurizr.view`): the view set and every view type, the
//! per-view element/relationship entry views, automatic layout, and the view
//! configuration (styles / themes / terminology).
//!
//! Shape notes (library 6.2.4): views have NO `background`; `ElementView` is exactly
//! `{id,x,y}` with `x`/`y` primitive ints — ALWAYS serialized; included/excluded lists are GONE (the entry arrays ARE
//! the content); `ImageView` is a plain view (no entry arrays); animations exist
//! on the four static views + `DeploymentView` + `CustomView` (NOT on
//! `DynamicView`); `FilteredView.tags` is a set (UTF-16
//! alphabetical on write).

use crate::ids::Id;

/// Common fields of every view. `order` is an int in the format
/// (views are ordered by `(order, key-case-insensitive)`).
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
    /// Auto-generated key (`generatedKey`) — a primitive flag written only
    /// when `true`; the official reader binds the package-private setter, so
    /// JSON→JSON round-trips it.
    pub generated_key: bool,
    /// Free-form properties.
    pub properties: crate::properties::Properties,
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
            generated_key: false,
            properties: crate::properties::Properties::new(),
        }
    }
}

/// The entry-arrays part shared by all model-view kinds.
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
    /// JSON `enterpriseBoundaryVisible` (primitive bool — ALWAYS written;
    /// the format default is `true`).
    pub enterprise_boundary_visible: bool,
    /// Animation steps.
    pub animations: Vec<Animation>,
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
    /// JSON `enterpriseBoundaryVisible` (primitive bool — ALWAYS written;
    /// the format default is `true`).
    pub enterprise_boundary_visible: bool,
    /// Animation steps.
    pub animations: Vec<Animation>,
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
    /// JSON `externalSoftwareSystemBoundariesVisible` (primitive bool —
    /// ALWAYS written; the format default is `false`).
    pub external_software_system_boundaries_visible: bool,
    /// Animation steps.
    pub animations: Vec<Animation>,
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
    /// JSON `externalContainerBoundariesVisible` (primitive bool — ALWAYS
    /// written; the format default is `false`).
    pub external_container_boundaries_visible: bool,
    /// Animation steps.
    pub animations: Vec<Animation>,
}

/// A dynamic view. The entry arrays are ORDERED
/// (`RelationshipView.order` numbers the steps). A dynamic view has
/// no animations (unlike the static and deployment views).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DynamicView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Scope element, if any (JSON `elementId`).
    pub element_id: Option<Id>,
    /// Deployment-scope dynamic views. The format keeps this field PRIVATE —
    /// invisible in JSON in BOTH directions; it exists for the DSL only and
    /// is never (de)serialized.
    pub deployment_environment: Option<String>,
    /// JSON `externalBoundariesVisible` (primitive bool — ALWAYS written).
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

/// A custom view — the target of DSL `image { … }` blocks
/// too (they produce `ImageView`s; custom views exist in the format but
/// not the `OpenAPI` schema).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomView {
    /// Common fields.
    pub base: ViewBase,
    /// Entries + layout.
    pub content: ViewContent,
    /// Animation steps.
    pub animations: Vec<Animation>,
    /// Optional software-system scope (`softwareSystemId` — the
    /// public getter makes it visible in JSON when set).
    pub software_system_id: Option<Id>,
}

/// A filtered view (`view/FilteredView`): a saved filter over a base view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilteredView {
    /// Common fields.
    pub base: ViewBase,
    /// The base view key (JSON `baseViewKey`).
    pub base_view_key: String,
    /// Include/exclude — nullable field: the official writer omits the key
    /// when the JSON input carried an unknown/absent mode.
    pub mode: Option<FilterMode>,
    /// Filter terms (serialized as the JSON `tags` ARRAY, sorted
    /// alphabetically on write).
    pub tags: Vec<String>,
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
    /// Light-mode content (`contentLight`, ★ not in the `OpenAPI` schema).
    pub content_light: Option<String>,
    /// Dark-mode content (`contentDark`, ★ not in the `OpenAPI` schema).
    pub content_dark: Option<String>,
    /// Content type (e.g. `PlantUML`, `Mermaid`).
    pub content_type: Option<String>,
}

/// One included element (JSON `elements[]` entry). `x`/`y` are
/// primitive ints in the format: never treated as empty, so they are
/// ALWAYS written, including the `0`/`0` pair.
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
/// populate `order`/`description`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipView {
    /// Referenced relationship id.
    pub id: Id,
    /// Optional description override.
    pub description: Option<String>,
    /// Optional URL (★ not in the `OpenAPI` schema).
    pub url: Option<String>,
    /// Dynamic-step order number (stored as a STRING in the format).
    pub order: Option<String>,
    /// Async response marker (`response`) — omitted only when absent,
    /// `false` IS written.
    pub response: Option<bool>,
    /// Free-form properties.
    pub properties: crate::properties::Properties,
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
/// the official tooling keeps them sorted (lexicographic on write).
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
/// Defaults match the official constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomaticLayout {
    /// Rank direction — nullable bean field: the JSON reader stores `None`
    /// when the key is absent (then the writer omits it; the official
    /// reader throws when the key is present but unparsable).
    pub rank_direction: Option<RankDirection>,
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
            rank_direction: Some(RankDirection::LeftRight),
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

impl PaperSize {
    /// JSON spelling — the official constant names (`A6_Portrait` … `Slide_16_10`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            PaperSize::A6Portrait => "A6_Portrait",
            PaperSize::A6Landscape => "A6_Landscape",
            PaperSize::A5Portrait => "A5_Portrait",
            PaperSize::A5Landscape => "A5_Landscape",
            PaperSize::A4Portrait => "A4_Portrait",
            PaperSize::A4Landscape => "A4_Landscape",
            PaperSize::A3Portrait => "A3_Portrait",
            PaperSize::A3Landscape => "A3_Landscape",
            PaperSize::A2Portrait => "A2_Portrait",
            PaperSize::A2Landscape => "A2_Landscape",
            PaperSize::A1Portrait => "A1_Portrait",
            PaperSize::A1Landscape => "A1_Landscape",
            PaperSize::A0Portrait => "A0_Portrait",
            PaperSize::A0Landscape => "A0_Landscape",
            PaperSize::LetterPortrait => "Letter_Portrait",
            PaperSize::LetterLandscape => "Letter_Landscape",
            PaperSize::LegalPortrait => "Legal_Portrait",
            PaperSize::LegalLandscape => "Legal_Landscape",
            PaperSize::Slide43 => "Slide_4_3",
            PaperSize::Slide169 => "Slide_16_9",
            PaperSize::Slide1610 => "Slide_16_10",
        }
    }

    /// Parse the JSON spelling (exact case).
    #[must_use]
    pub fn parse(s: &str) -> Option<PaperSize> {
        Self::ALL.into_iter().find(|p| p.as_str() == s)
    }

    /// All variants (for `parse`).
    const ALL: [PaperSize; 21] = [
        PaperSize::A6Portrait,
        PaperSize::A6Landscape,
        PaperSize::A5Portrait,
        PaperSize::A5Landscape,
        PaperSize::A4Portrait,
        PaperSize::A4Landscape,
        PaperSize::A3Portrait,
        PaperSize::A3Landscape,
        PaperSize::A2Portrait,
        PaperSize::A2Landscape,
        PaperSize::A1Portrait,
        PaperSize::A1Landscape,
        PaperSize::A0Portrait,
        PaperSize::A0Landscape,
        PaperSize::LetterPortrait,
        PaperSize::LetterLandscape,
        PaperSize::LegalPortrait,
        PaperSize::LegalLandscape,
        PaperSize::Slide43,
        PaperSize::Slide169,
        PaperSize::Slide1610,
    ];
}

/// Implements `as_str`/`parse` for enums whose JSON name IS the variant name
/// (Rust variants mirror the official JSON enum names exactly).
macro_rules! json_names {
    ($($ty:ident { $($variant:ident),* $(,)? }),* $(,)?) => {
        $(
            impl $ty {
                /// JSON spelling (the official constant name).
                #[must_use]
                pub const fn as_str(self) -> &'static str {
                    match self { $(Self::$variant => stringify!($variant)),* }
                }

                /// Parse the JSON spelling (exact case; the official reader maps
                /// unknown values to null).
                #[must_use]
                pub fn parse(s: &str) -> Option<Self> {
                    match s {
                        $(stringify!($variant) => Some(Self::$variant),)*
                        _ => None,
                    }
                }
            }
        )*
    };
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

json_names! {
    RankDirection { TopBottom, BottomTop, LeftRight, RightLeft },
    Routing { Direct, Curved, Orthogonal },
    LineStyle { Solid, Dashed, Dotted },
    Shape {
        Box, RoundedBox, Circle, Ellipse, Hexagon, Diamond, Cylinder, Bucket,
        Pipe, Person, Robot, Folder, WebBrowser, Window, Terminal, Shell,
        MobileDevicePortrait, MobileDeviceLandscape, Component,
    },
    IconPosition { Top, Bottom, Left },
    ColorScheme { Light, Dark },
    FilterMode { Include, Exclude },
    ViewSortOrder { Default, Type, Key },
    MetadataSymbols {
        SquareBrackets, RoundBrackets, CurlyBrackets, AngleBrackets,
        DoubleAngleBrackets, None,
    },
}

/// An element style, matched by `tag`. All paint fields are Option
/// (only set fields serialize).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    /// Show metadata (nullable — default null).
    pub metadata: Option<bool>,
    /// Show description.
    pub description: Option<bool>,
    /// Free-form properties — only written when non-empty.
    pub properties: crate::properties::Properties,
}

/// A relationship style, matched by `tag`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
    /// Free-form properties (`AbstractStyle implements PropertyHolder`).
    pub properties: crate::properties::Properties,
}

/// Element + relationship styles. The vectors mirror the format's
/// iteration order: the DSL builder inserts through
/// [`Styles::insert_element_style`] (sorted-set semantics in the format),
/// while the JSON reader appends in document order (the reader binds
/// `elements`/`relationships` by FIELD, replacing the set with an ordered
/// list, so document order survives).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Styles {
    /// Element styles, in format iteration order (see [`Styles`]).
    pub element_styles: Vec<ElementStyle>,
    /// Relationship styles, in format iteration order (see [`Styles`]).
    pub relationship_styles: Vec<RelationshipStyle>,
    /// Themes loaded from the themes directory
    /// loaded from theme files, in add order. Consulted by the
    /// resolution engine (`crate::styles`) BEFORE the workspace styles —
    /// never merged, and NOT serialized (the format has no accessor for it:
    /// a JSON export after loading themes shows no theme content).
    pub themes: Vec<Theme>,
}

impl Styles {
    /// Insert `style` at its sorted position — the DSL path adds into
    /// a sorted set the whole time. The
    /// ordering key (`colorScheme`, `tag`) is fixed at construction, so sorted
    /// insertion reproduces the official iteration exactly. Returns the index.
    pub fn insert_element_style(&mut self, style: ElementStyle) -> usize {
        let pos = self.element_styles.partition_point(|s| {
            style_cmp(s.color_scheme, &s.tag, style.color_scheme, &style.tag).is_lt()
        });
        self.element_styles.insert(pos, style);
        pos
    }

    /// [`Styles::insert_element_style`] for relationship styles.
    pub fn insert_relationship_style(&mut self, style: RelationshipStyle) -> usize {
        let pos = self.relationship_styles.partition_point(|s| {
            style_cmp(s.color_scheme, &s.tag, style.color_scheme, &style.tag).is_lt()
        });
        self.relationship_styles.insert(pos, style);
        pos
    }
}

/// `AbstractStyle.compareTo` (`view/AbstractStyle`): null `colorScheme` sorts
/// before non-null; both null → `tag`, both set → `colorScheme + "/" + tag` —
/// always UTF-16 code-unit order.
fn style_cmp(
    a_scheme: Option<ColorScheme>,
    a_tag: &str,
    b_scheme: Option<ColorScheme>,
    b_tag: &str,
) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a_scheme, b_scheme) {
        (None, None) => a_tag.encode_utf16().cmp(b_tag.encode_utf16()),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (Some(a), Some(b)) => {
            let x = format!("{}/{a_tag}", a.as_str());
            let y = format!("{}/{b_tag}", b.as_str());
            x.encode_utf16().cmp(y.encode_utf16())
        }
    }
}

/// A `theme.json` document — the theme de/serialization shape
/// of `export --format theme` (write: name/description from the
/// workspace, styles the live collections) and of theme loading. Field
/// order is the format's declaration order, written as key order:
/// `name, description, elements, relationships, license`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Theme {
    /// Theme name — the workspace name on the export path (omitted empty).
    pub name: Option<String>,
    /// Theme description (omitted empty).
    pub description: Option<String>,
    /// Element styles, document order on read.
    pub elements: Vec<ElementStyle>,
    /// Relationship styles, document order on read.
    pub relationships: Vec<RelationshipStyle>,
    /// License text — declared last, never set on write.
    pub license: Option<String>,
}

/// Terminology overrides; every field defaults to `""` (unset ones are
/// omitted).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Terminology {
    /// Legacy `enterprise` (schema-era read; the field exists).
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
    pub properties: crate::properties::Properties,
}

/// All views onto the model (`view/ViewSet`). Each collection preserves
/// insertion order; the official tooling keeps sets ordered by
/// `(order, key-ignore-case)` — the writer sorts on output.
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
    /// Every view (key, kind label) in the official traversal
    /// order used for key-uniqueness validation: the union set ordered by
    /// `(order, key lowercased)` is what uniqueness validation walks —
    /// [`mod@crate::validate`] reproduces that ordering.
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

impl<'a> View<'a> {
    /// The view's common base (taken by value, tied to `'a`, so callers can
    /// store the key beyond any local borrow of the enum).
    #[must_use]
    pub const fn base(self) -> &'a ViewBase {
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
    pub fn key(self) -> &'a str {
        &self.base().key
    }

    /// The view's `order`.
    #[must_use]
    pub const fn order(self) -> i32 {
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
