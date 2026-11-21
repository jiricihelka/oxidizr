//! Port of the reference `DslContext` class hierarchy as a data enum.
//!
//! The reference parser keeps a stack of context objects; statement dispatch
//! guards on `inContext(FooDslContext.class)` — which is
//! `clazz.isAssignableFrom(stack.peek().getClass())`, a TOP-OF-STACK `is-a`
//! test, never a stack search. Here the stack is `Vec<Ctx>` and the guards
//! are [`Ctx::is_*`] predicates mirroring the Java class hierarchy — that
//! hierarchy IS the compatibility contract, so each predicate is annotated
//! with the Java superclass chain it reproduces (verified @ `ad34036`).
//!
//! Payloads are owned values (element ids, view refs, names, builder-side
//! indices), never borrows: the dispatch chain repeatedly reads a context
//! payload and then mutably borrows the builder.

use oxidizr_core::model::Perspective;
use oxidizr_core::{Id, ViewRef};

/// An `ElementGroup`: the DSL pseudo-element whose (separator-joined) name
/// is what grouped elements' `group` property stores. The `uid` identifies
/// the ONE Java group object shared by the registered pseudo-element and
/// every context frame built over the `group` block — expression finders
/// expand it via its live member set (see [`Parser::group_add_member`]).
///
/// [`Parser::group_add_member`]: super::Parser::group_add_member
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslGroup {
    /// Fresh per `group` statement (the Java object identity).
    pub uid: u64,
    /// The full, separator-joined group name (`ElementGroup.getName()`).
    pub name: String,
}

/// A `deploymentEnvironment` — a DSL-only object (`DeploymentEnvironment`
/// never enters the model). Contexts hold a FRESH instance, exactly like
/// `DeploymentEnvironmentDslContext(String)`, which is why identifier
/// lookups through a context environment never resolve (the `"null."`
/// hierarchical-identifier quirk).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DslEnvironment {
    /// Environment name.
    pub name: String,
}

/// One frame of the context stack (a `DslContext` subclass instance).
#[derive(Debug, Clone)]
pub enum Ctx {
    /// `WorkspaceDslContext`.
    Workspace,
    /// `CommentDslContext` (a `/* … */` block spanning lines).
    Comment,

    // ---- model scope -----------------------------------------------------
    /// `ModelDslContext` — `group` set when started as `group "X" {`.
    Model { group: Option<DslGroup> },
    /// `ArchetypesDslContext`.
    Archetypes,
    /// `*ElementArchetypeDslContext` — `kind` is the archetype type token
    /// (`person`, `softwareSystem`, `element`, `container`, `component`,
    /// `deploymentNode`, `infrastructureNode`); `idx` indexes the parser's
    /// archetype list.
    ElementArchetype { kind: String, idx: usize },
    /// `RelationshipArchetypeDslContext`.
    RelationshipArchetype { idx: usize },

    /// `PersonDslContext`.
    Person { id: Id },
    /// `CustomElementDslContext`.
    CustomElement { id: Id },
    /// `SoftwareSystemDslContext` — also pushed for `group "X" {` inside a
    /// system (the Java class re-use), carrying the group payload.
    SoftwareSystem { id: Id, group: Option<DslGroup> },
    /// `ContainerDslContext` — also pushed for `group "X" {` inside a
    /// container scope.
    Container { id: Id, group: Option<DslGroup> },
    /// `ComponentDslContext`.
    Component { id: Id },
    /// `RelationshipDslContext` (single-relationship `{ … }` block).
    Rel { id: Id },

    /// `ElementsDslContext` (a `!elements` finder block or fan-out).
    Elements {
        /// The found element ids (insertion-ordered `LinkedHashSet`).
        items: Vec<Id>,
    },
    /// `RelationshipsDslContext` (a `!relationships` block or a
    /// multi-relationship fan-out statement's context).
    Relationships { items: Vec<Id> },

    // ---- deployment ------------------------------------------------------
    /// `DeploymentEnvironmentDslContext`.
    Environment {
        env: DslEnvironment,
        group: Option<DslGroup>,
    },
    /// `DeploymentNodeDslContext` — also pushed for `deploymentGroup "X" {`
    /// inside a node (`GroupParser` semantics apply to node groups too).
    DeploymentNode { id: Id, group: Option<DslGroup> },
    /// `InfrastructureNodeDslContext`.
    InfrastructureNode { id: Id },
    /// `SoftwareSystemInstanceDslContext`.
    SsInstance { id: Id },
    /// `ContainerInstanceDslContext`.
    ContainerInstance { id: Id },
    /// `NoRelationshipInDeploymentEnvironmentDslContext` — a SUBCLASS of
    /// `DeploymentEnvironmentDslContext` (so all environment guards hold
    /// inside it); carries the static (linked) relationship the removed
    /// instance relationships were based on, so `a -> b` inside the block
    /// re-adds them, and the enclosing group (`super(parent.getGroup())`).
    NoRelInEnv {
        /// The environment name.
        environment: String,
        /// The static relationship the removals were based on.
        linked: Id,
        /// Group inherited from the enclosing environment context.
        group: Option<DslGroup>,
    },

    // ---- views -----------------------------------------------------------
    /// `ViewsDslContext`.
    Views,
    /// `SystemLandscapeViewDslContext`.
    LandscapeView { view: ViewRef },
    /// `SystemContextViewDslContext`.
    ContextView { view: ViewRef },
    /// `ContainerViewDslContext`.
    ContainerView { view: ViewRef },
    /// `ComponentViewDslContext`.
    ComponentView { view: ViewRef },
    /// `FilteredViewDslContext` — NOT a `ModelViewDslContext` (the Java quirk
    /// that makes `autolayout` inside it the cast failure).
    FilteredView { view: ViewRef },
    /// `DynamicViewDslContext`.
    DynamicView { view: ViewRef },
    /// `DynamicViewParallelSequenceDslContext` (a `{` block inside a dynamic
    /// view). `has_relationships` is set by `hasRelationships(true)` when a
    /// relationship statement is parsed inside the sequence; `end()` calls
    /// `view.endParallelSequence(!has_relationships)`.
    DynamicParallel {
        view: ViewRef,
        has_relationships: bool,
    },
    /// `DynamicViewRelationshipContext` (a `->` block inside a dynamic view).
    /// Extends `DslContext` DIRECTLY — it is NOT a view context.
    DynamicRel { view: ViewRef, rel: Id },
    /// `DeploymentViewDslContext`.
    DeploymentView { view: ViewRef },
    /// `CustomViewDslContext`.
    CustomView { view: ViewRef },
    /// `ImageViewDslContext` — `light`/`dark` re-push the SAME context with a
    /// color scheme, so the scheme is payload, not a subclass.
    ImageView {
        view: ViewRef,
        color_scheme: Option<ColorScheme>,
    },
    /// `StaticViewAnimationDslContext` — extends `DslContext` (NOT a view).
    StaticViewAnimation { view: ViewRef },
    /// `DeploymentViewAnimationDslContext` — extends `DslContext`.
    DeploymentViewAnimation { view: ViewRef },
    /// `CustomViewAnimationDslContext` — extends `DslContext`.
    CustomViewAnimation { view: ViewRef },

    // ---- styles ----------------------------------------------------------
    /// `StylesDslContext` — `light`/`dark` re-push with the scheme.
    Styles { color_scheme: Option<ColorScheme> },
    /// `ElementStyleDslContext` — `idx` indexes the builder's element-style
    /// list (`views.configuration.styles.element_styles`).
    ElementStyle { idx: usize },
    /// `RelationshipStyleDslContext`.
    RelationshipStyle { idx: usize },

    // ---- configuration / terminology / documentation --------------------
    /// `TerminologyDslContext`.
    Terminology,
    /// `ConfigurationDslContext`.
    Configuration,
    /// `UsersDslContext`.
    Users,
    /// `DocumentationDslContext` (`!docs {`). The payload feeds the
    /// P4-M2 importer at context close (`end()`): path token, resolved
    /// importer FQN, and the `exclude` tokens.
    Documentation {
        holder: Holder,
        path: String,
        fqn: String,
        excludes: Vec<String>,
    },
    /// `DecisionsDslContext` (`!adrs`/`!decisions {`). Same payload as
    /// [`Ctx::Documentation`] — the `end()` shape is identical.
    Decisions {
        holder: Holder,
        path: String,
        fqn: String,
        excludes: Vec<String>,
    },

    // ---- property / perspective containers ------------------------------
    /// `PropertiesDslContext` over the property holders its `name value`
    /// catch-all applies to.
    Properties { holders: Vec<Holder> },
    /// `PerspectivesDslContext` over its perspectives holders.
    Perspectives { holders: Vec<Holder> },
    /// `PerspectiveDslContext` — the staged perspective; `end()` attaches it
    /// to the enclosing [`Ctx::Perspectives`] holders.
    Perspective { def: Perspective },
}

/// `ImageViewDslContext` / `StylesDslContext` color scheme payload — the
/// core enum itself (styles carry it as `colorScheme` in JSON).
pub use oxidizr_core::views::ColorScheme;

/// What a `properties { … }` / `perspectives { … }` / `!docs` statement
/// attaches to — the `PropertyHolder` / `PerspectivesHolder` /
/// `DocumentationOwner` targets of the reference context constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Holder {
    /// Workspace properties (`workspace`, `configuration { properties … }`
    /// — the `ConfigurationDslContext` arm passes the WORKSPACE).
    Workspace,
    /// Model properties (`model { properties … }`).
    Model,
    /// Any model item (element, relationship, instance, node, …).
    Item(Id),
    /// Every item of a fan-out (`ModelItemsDslContext` over `!elements` /
    /// `!relationships` / multi-relationship blocks).
    Items(Vec<Id>),
    /// An archetype (`ArchetypeDslContext` is a `PropertyHolder` +
    /// `PerspectivesHolder`).
    Archetype { idx: usize },
    /// A view (`ViewDslContext` → view properties).
    View(ViewRef),
    /// A dynamic-view relationship statement (`RelationshipView` props).
    DynamicRel { view: ViewRef, rel: Id },
    /// An element style (`elementStyle … { properties … }`).
    ElementStyle { idx: usize },
    /// A relationship style.
    RelationshipStyle { idx: usize },
    /// The view-set configuration (`views { properties … }`).
    ViewSet,
}

impl Ctx {
    /// `getPermittedTokens()` — feeds the catch-all
    /// `Unexpected tokens (expected: a, b, …)` message (joined `", "` minus
    /// the trailing separator; an empty list produces bare `Unexpected
    /// tokens`). Order and content verified verbatim against the reference
    /// classes @ `ad34036`.
    #[must_use]
    #[allow(clippy::too_many_lines)] // per-context tables mirror the reference classes 1:1
    #[allow(clippy::match_same_arms)] // empty permitted-lists are per-class facts
    pub fn permitted_tokens(&self) -> Vec<&'static str> {
        use super::tok as t;
        match self {
            Ctx::Workspace => vec![
                t::NAME,
                t::DESCRIPTION,
                t::PROPERTIES,
                t::DOCS,
                t::DECISIONS,
                t::IDENTIFIERS,
                t::IMPLIED_RELATIONSHIPS,
                t::MODEL,
                t::VIEWS,
                t::CONFIGURATION,
            ],
            Ctx::Comment => vec![],
            Ctx::Model { .. } => vec![
                t::ARCHETYPES,
                t::IDENTIFIERS,
                t::GROUP,
                t::PERSON,
                t::SOFTWARE_SYSTEM,
                t::DEPLOYMENT_ENVIRONMENT,
                t::CUSTOM_ELEMENT,
                t::RELATIONSHIP,
            ],
            Ctx::Archetypes => vec![],
            // Element-archetype permitted lists differ per subclass; they
            // are assembled by `ArchetypeParser` guards, not by these
            // token lists (the lists below mirror each subclass).
            Ctx::ElementArchetype { kind, .. } => match kind.as_str() {
                // PersonArchetype / SoftwareSystemArchetype: no technology.
                t::PERSON | t::SOFTWARE_SYSTEM => {
                    vec![
                        t::DESCRIPTION,
                        t::TAG,
                        t::TAGS,
                        t::PROPERTIES,
                        t::PERSPECTIVES,
                    ]
                }
                // CustomElementArchetype adds metadata.
                t::CUSTOM_ELEMENT => vec![
                    t::METADATA,
                    t::DESCRIPTION,
                    t::TAG,
                    t::TAGS,
                    t::PROPERTIES,
                    t::PERSPECTIVES,
                ],
                // Container/Component/DeploymentNode/InfrastructureNode.
                _ => vec![
                    t::DESCRIPTION,
                    t::TECHNOLOGY,
                    t::TAG,
                    t::TAGS,
                    t::PROPERTIES,
                    t::PERSPECTIVES,
                ],
            },
            Ctx::RelationshipArchetype { .. } => vec![
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::TAG,
                t::TAGS,
                t::PROPERTIES,
                t::PERSPECTIVES,
            ],
            Ctx::Person { .. } => vec![
                t::DESCRIPTION,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::RELATIONSHIP,
            ],
            Ctx::CustomElement { .. } => vec![
                t::DESCRIPTION,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::RELATIONSHIP,
            ],
            Ctx::SoftwareSystem { .. } => vec![
                t::DOCS,
                t::DECISIONS,
                t::GROUP,
                t::CONTAINER,
                t::DESCRIPTION,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::RELATIONSHIP,
            ],
            Ctx::Container { .. } => vec![
                t::DOCS,
                t::DECISIONS,
                t::GROUP,
                t::COMPONENT,
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::RELATIONSHIP,
            ],
            Ctx::Component { .. } => vec![
                t::DOCS,
                t::DECISIONS,
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::GROUP,
                t::RELATIONSHIP,
            ],
            Ctx::Rel { .. } => vec![t::TAGS, t::URL, t::PROPERTIES, t::PERSPECTIVES],
            Ctx::Elements { .. } => vec![
                t::RELATIONSHIP,
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::TAG,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
            ],
            Ctx::Relationships { .. } => vec![
                t::TECHNOLOGY,
                t::TAG,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
            ],
            Ctx::Environment { .. } | Ctx::NoRelInEnv { .. } => vec![
                t::GROUP,
                t::DEPLOYMENT_GROUP,
                t::DEPLOYMENT_NODE,
                t::RELATIONSHIP,
            ],
            Ctx::DeploymentNode { .. } => vec![
                t::GROUP,
                t::DEPLOYMENT_NODE,
                t::INFRASTRUCTURE_NODE,
                t::INSTANCE_OF,
                t::SOFTWARE_SYSTEM_INSTANCE,
                t::CONTAINER_INSTANCE,
                t::RELATIONSHIP,
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::INSTANCES,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::DEPLOYMENT_GROUP,
            ],
            Ctx::InfrastructureNode { .. } => vec![
                t::RELATIONSHIP,
                t::DESCRIPTION,
                t::TECHNOLOGY,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
            ],
            Ctx::SsInstance { .. } | Ctx::ContainerInstance { .. } => vec![
                t::RELATIONSHIP,
                t::DESCRIPTION,
                t::TAGS,
                t::URL,
                t::PROPERTIES,
                t::PERSPECTIVES,
                t::HEALTH_CHECK,
            ],
            Ctx::Views => vec![
                t::VIEW_LANDSCAPE,
                t::VIEW_CONTEXT,
                t::VIEW_CONTAINER,
                t::VIEW_COMPONENT,
                t::VIEW_FILTERED,
                t::VIEW_DYNAMIC,
                t::VIEW_DEPLOYMENT,
                t::VIEW_CUSTOM,
                t::STYLES,
                t::THEME,
                t::THEMES,
                t::TERMINOLOGY,
                t::PROPERTIES,
            ],
            Ctx::LandscapeView { .. }
            | Ctx::ContextView { .. }
            | Ctx::ContainerView { .. }
            | Ctx::ComponentView { .. } => vec![
                t::VIEW_INCLUDE,
                t::VIEW_EXCLUDE,
                t::VIEW_AUTOLAYOUT,
                t::VIEW_DEFAULT,
                t::VIEW_ANIMATION,
                t::VIEW_TITLE,
                t::DESCRIPTION,
                t::PROPERTIES,
            ],
            Ctx::FilteredView { .. } => {
                vec![
                    t::VIEW_DEFAULT,
                    t::VIEW_TITLE,
                    t::DESCRIPTION,
                    t::PROPERTIES,
                ]
            }
            Ctx::DynamicView { .. } | Ctx::DynamicParallel { .. } => vec![
                t::VIEW_AUTOLAYOUT,
                t::VIEW_DEFAULT,
                t::VIEW_TITLE,
                t::DESCRIPTION,
                t::PROPERTIES,
                t::RELATIONSHIP,
            ],
            Ctx::DynamicRel { .. } => vec![t::URL],
            Ctx::DeploymentView { .. } | Ctx::CustomView { .. } => vec![
                t::VIEW_INCLUDE,
                t::VIEW_EXCLUDE,
                t::VIEW_AUTOLAYOUT,
                t::VIEW_DEFAULT,
                t::VIEW_ANIMATION,
                t::VIEW_TITLE,
                t::DESCRIPTION,
                t::PROPERTIES,
            ],
            Ctx::ImageView { .. } => vec![
                t::VIEW_TITLE,
                t::DESCRIPTION,
                t::PLANTUML,
                t::MERMAID,
                t::KROKI,
                t::IMAGE,
            ],
            Ctx::StaticViewAnimation { .. }
            | Ctx::DeploymentViewAnimation { .. }
            | Ctx::CustomViewAnimation { .. } => vec![t::VIEW_ANIMATION_STEP],
            Ctx::Styles { .. } => vec![t::ELEMENT_STYLE, t::RELATIONSHIP_STYLE],
            Ctx::ElementStyle { .. } => vec![
                t::ELEMENT_STYLE_SHAPE,
                t::ELEMENT_STYLE_ICON,
                t::ELEMENT_STYLE_ICON_POSITION,
                t::ELEMENT_STYLE_WIDTH,
                t::ELEMENT_STYLE_HEIGHT,
                t::ELEMENT_STYLE_BACKGROUND,
                t::ELEMENT_STYLE_COLOR,
                t::ELEMENT_STYLE_COLOUR,
                t::ELEMENT_STYLE_STROKE,
                t::ELEMENT_STYLE_STROKE_WIDTH,
                t::ELEMENT_STYLE_FONT_SIZE,
                t::ELEMENT_STYLE_BORDER,
                t::ELEMENT_STYLE_OPACITY,
                t::ELEMENT_STYLE_METADATA,
                t::ELEMENT_STYLE_DESCRIPTION,
                t::PROPERTIES,
            ],
            // Java's `RelationshipStyleDslContext` list omits
            // `dashed`/`jump`/`metadata`/`description` even though the
            // chain has arms for them — the list is catch-all-message
            // material only, reproduced exactly.
            Ctx::RelationshipStyle { .. } => vec![
                t::RELATIONSHIP_STYLE_THICKNESS,
                t::RELATIONSHIP_STYLE_COLOR,
                t::RELATIONSHIP_STYLE_COLOUR,
                t::RELATIONSHIP_STYLE_LINE_STYLE,
                t::RELATIONSHIP_STYLE_ROUTING,
                t::RELATIONSHIP_STYLE_FONT_SIZE,
                t::RELATIONSHIP_STYLE_WIDTH,
                t::RELATIONSHIP_STYLE_POSITION,
                t::RELATIONSHIP_STYLE_OPACITY,
                t::PROPERTIES,
            ],
            Ctx::Terminology => vec![
                t::PERSON,
                t::SOFTWARE_SYSTEM,
                t::CONTAINER,
                t::COMPONENT,
                t::DEPLOYMENT_NODE,
                t::INFRASTRUCTURE_NODE,
                t::TERMINOLOGY_RELATIONSHIP,
                t::METADATA_SYMBOLS,
            ],
            Ctx::Configuration => vec![t::SCOPE, t::VISIBILITY, t::USERS, t::PROPERTIES],
            Ctx::Users => vec![],
            Ctx::Documentation { .. } => vec![t::DOCS_EXCLUDE],
            Ctx::Decisions { .. } => vec![t::DECISIONS_EXCLUDE],
            Ctx::Properties { .. } => vec![],
            Ctx::Perspectives { .. } => vec![t::PERSPECTIVE],
            Ctx::Perspective { .. } => vec![t::PERSPECTIVE_DESCRIPTION, t::PERSPECTIVE_VALUE],
        }
    }

    /// The joined form used in the `Unexpected tokens (expected: …)` message.
    /// `None` means the reference prints bare `Unexpected tokens`.
    #[must_use]
    pub fn permitted_tokens_message(&self) -> Option<String> {
        let tokens = self.permitted_tokens();
        if tokens.is_empty() {
            None
        } else {
            Some(format!(
                "Unexpected tokens (expected: {})",
                tokens.join(", ")
            ))
        }
    }

    // ------------------------------------------------------------------
    // is-a guards (Java superclass chains annotated per predicate)
    // ------------------------------------------------------------------

    /// `ModelItemDslContext`: element contexts, the single-relationship
    /// context, and both archetype context families
    /// (`ArchetypeDslContext implements` the same interface surface used
    /// by the dispatch guards).
    #[must_use]
    pub fn is_model_item(&self) -> bool {
        // `ModelItemDslContext` subclasses: the element hierarchy and
        // `RelationshipDslContext` (archetype contexts extend `DslContext`
        // directly — their `tags`/`properties`/`perspectives` arms are the
        // separate `ArchetypeDslContext` arms further down the chain).
        self.is_element() || matches!(self, Ctx::Rel { .. })
    }

    /// `ElementDslContext` = Person | `CustomElement` | `SoftwareSystem` |
    /// Container | Component | `DeploymentNode` | `InfrastructureNode` |
    /// `SoftwareSystemInstance` | `ContainerInstance` (all through
    /// `GroupableElementDslContext` / `StaticStructureElementInstanceDslContext`).
    #[must_use]
    pub fn is_element(&self) -> bool {
        matches!(
            self,
            Ctx::Person { .. }
                | Ctx::CustomElement { .. }
                | Ctx::SoftwareSystem { .. }
                | Ctx::Container { .. }
                | Ctx::Component { .. }
                | Ctx::DeploymentNode { .. }
                | Ctx::InfrastructureNode { .. }
                | Ctx::SsInstance { .. }
                | Ctx::ContainerInstance { .. }
        )
    }

    /// `StaticStructureElementInstanceDslContext`.
    #[allow(dead_code)] // Java parity surface: the port's arms inline this guard.
    #[must_use]
    pub fn is_instance(&self) -> bool {
        matches!(self, Ctx::SsInstance { .. } | Ctx::ContainerInstance { .. })
    }

    /// `ModelItemsDslContext` (`!elements` / `!relationships` families).
    #[must_use]
    pub fn is_model_items(&self) -> bool {
        matches!(self, Ctx::Elements { .. } | Ctx::Relationships { .. })
    }

    /// `ArchetypeDslContext` (element + relationship archetypes).
    #[allow(dead_code)] // Java parity surface: the port's arms inline this guard.
    #[must_use]
    pub fn is_archetype(&self) -> bool {
        matches!(
            self,
            Ctx::ElementArchetype { .. } | Ctx::RelationshipArchetype { .. }
        )
    }

    /// `GroupableDslContext`: every `GroupableElementDslContext` subclass
    /// plus `ModelDslContext`, `DeploymentEnvironmentDslContext` and its
    /// subclass `NoRelationshipInDeploymentEnvironmentDslContext`.
    #[must_use]
    pub fn is_groupable(&self) -> bool {
        self.is_element()
            || matches!(
                self,
                Ctx::Model { .. } | Ctx::Environment { .. } | Ctx::NoRelInEnv { .. }
            )
    }

    /// `isGroup(context)` in the dispatch chain: `hasGroup()` — true only
    /// for the five context classes whose constructors take a group
    /// payload (`Model`, `SoftwareSystem`, `Container`, `DeploymentNode`,
    /// `DeploymentEnvironment` — and `NoRelInEnv`, which inherits its
    /// parent environment's group via `super(parent.getGroup())`).
    #[must_use]
    pub fn group_payload(&self) -> Option<&DslGroup> {
        match self {
            Ctx::Model { group }
            | Ctx::SoftwareSystem { group, .. }
            | Ctx::Container { group, .. }
            | Ctx::DeploymentNode { group, .. }
            | Ctx::Environment { group, .. }
            | Ctx::NoRelInEnv { group, .. } => group.as_ref(),
            _ => None,
        }
    }

    /// `inContext(DeploymentEnvironmentDslContext.class)` — true for
    /// `NoRelationshipInDeploymentEnvironmentDslContext` too (subclass).
    #[must_use]
    pub fn is_environment(&self) -> bool {
        matches!(self, Ctx::Environment { .. } | Ctx::NoRelInEnv { .. })
    }

    /// The environment name carried by an environment context —
    /// `getEnvironment().getName()`, the key for the name-based
    /// `findIdentifier(new DeploymentEnvironment(name))` lookups.
    #[must_use]
    pub fn env_name(&self) -> Option<&str> {
        match self {
            Ctx::Environment { env, .. } => Some(&env.name),
            Ctx::NoRelInEnv { environment, .. } => Some(environment),
            _ => None,
        }
    }

    /// `ModelViewDslContext`: landscape/context/container/component/dynamic/
    /// parallel/deployment/custom — NOT filtered, NOT image.
    #[must_use]
    pub fn is_model_view(&self) -> bool {
        matches!(
            self,
            Ctx::LandscapeView { .. }
                | Ctx::ContextView { .. }
                | Ctx::ContainerView { .. }
                | Ctx::ComponentView { .. }
                | Ctx::DynamicView { .. }
                | Ctx::DynamicParallel { .. }
                | Ctx::DeploymentView { .. }
                | Ctx::CustomView { .. }
        )
    }

    /// `StaticViewDslContext` (landscape/context/container/component).
    #[must_use]
    pub fn is_static_view(&self) -> bool {
        matches!(
            self,
            Ctx::LandscapeView { .. }
                | Ctx::ContextView { .. }
                | Ctx::ContainerView { .. }
                | Ctx::ComponentView { .. }
        )
    }

    /// `ViewDslContext`: every `*ViewDslContext` incl. filtered and image —
    /// but NOT `DynamicViewRelationshipContext`, NOT the animation
    /// contexts (all extend `DslContext` directly).
    #[must_use]
    pub fn is_view(&self) -> bool {
        self.is_model_view() || matches!(self, Ctx::FilteredView { .. } | Ctx::ImageView { .. })
    }

    /// Payload: the element id of any element context.
    #[must_use]
    pub fn item_id(&self) -> Option<&Id> {
        match self {
            Ctx::Person { id }
            | Ctx::CustomElement { id }
            | Ctx::SoftwareSystem { id, .. }
            | Ctx::Container { id, .. }
            | Ctx::Component { id }
            | Ctx::DeploymentNode { id, .. }
            | Ctx::InfrastructureNode { id }
            | Ctx::SsInstance { id }
            | Ctx::ContainerInstance { id } => Some(id),
            _ => None,
        }
    }

    /// `ModelItemDslContext.getModelItem()` — the item payload of the
    /// element hierarchy PLUS `RelationshipDslContext`. (Distinct from
    /// [`Ctx::item_id`], which models the narrower `ElementDslContext
    /// .getElement()` used by the hierarchical-identifier walk.)
    #[must_use]
    pub fn model_item_id(&self) -> Option<&Id> {
        match self {
            Ctx::Rel { id } => Some(id),
            other => other.item_id(),
        }
    }

    /// Payload: the view ref of a view-family context (parallel included;
    /// `DynamicRel` carries its own view but is not a view context).
    #[must_use]
    pub fn view(&self) -> Option<ViewRef> {
        match self {
            Ctx::LandscapeView { view }
            | Ctx::ContextView { view }
            | Ctx::ContainerView { view }
            | Ctx::ComponentView { view }
            | Ctx::FilteredView { view }
            | Ctx::DynamicView { view }
            | Ctx::DynamicParallel { view, .. }
            | Ctx::DynamicRel { view, .. }
            | Ctx::DeploymentView { view }
            | Ctx::CustomView { view }
            | Ctx::ImageView { view, .. }
            | Ctx::StaticViewAnimation { view }
            | Ctx::DeploymentViewAnimation { view }
            | Ctx::CustomViewAnimation { view } => Some(*view),
            _ => None,
        }
    }

    /// The `Holder` a `properties`/`perspectives`/`!docs` statement on THIS
    /// (top) context attaches to, per the reference context-constructor
    /// call at each dispatch arm. `None` when the context is not a holder.
    #[must_use]
    #[allow(clippy::match_same_arms)] // holder kinds collapse identically by design
    #[allow(dead_code)] // Java parity surface: `DslContext.getHolder()`.
    pub fn holder(&self) -> Option<Holder> {
        match self {
            Ctx::Workspace => Some(Holder::Workspace),
            Ctx::Model { .. } => Some(Holder::Model),
            Ctx::Person { id }
            | Ctx::CustomElement { id }
            | Ctx::SoftwareSystem { id, .. }
            | Ctx::Container { id, .. }
            | Ctx::Component { id }
            | Ctx::DeploymentNode { id, .. }
            | Ctx::InfrastructureNode { id }
            | Ctx::SsInstance { id }
            | Ctx::ContainerInstance { id } => Some(Holder::Item(id.clone())),
            Ctx::Rel { id } => Some(Holder::Item(id.clone())),
            Ctx::Elements { items } | Ctx::Relationships { items } => {
                Some(Holder::Items(items.clone()))
            }
            Ctx::ElementArchetype { idx, .. } | Ctx::RelationshipArchetype { idx } => {
                Some(Holder::Archetype { idx: *idx })
            }
            Ctx::LandscapeView { view }
            | Ctx::ContextView { view }
            | Ctx::ContainerView { view }
            | Ctx::ComponentView { view }
            | Ctx::FilteredView { view }
            | Ctx::DynamicView { view }
            | Ctx::DynamicParallel { view, .. }
            | Ctx::DeploymentView { view }
            | Ctx::CustomView { view }
            | Ctx::ImageView { view, .. } => Some(Holder::View(*view)),
            Ctx::DynamicRel { view, rel } => Some(Holder::DynamicRel {
                view: *view,
                rel: rel.clone(),
            }),
            Ctx::ElementStyle { idx } => Some(Holder::ElementStyle { idx: *idx }),
            Ctx::RelationshipStyle { idx } => Some(Holder::RelationshipStyle { idx: *idx }),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn workspace_permitted_tokens_verbatim() {
        let msg = Ctx::Workspace.permitted_tokens_message().unwrap();
        assert_eq!(
            msg,
            "Unexpected tokens (expected: name, description, properties, !docs, !decisions, \
             !identifiers, !impliedRelationships, model, views, configuration)"
        );
    }

    #[test]
    fn model_permitted_tokens_verbatim() {
        let msg = Ctx::Model { group: None }
            .permitted_tokens_message()
            .unwrap();
        assert_eq!(
            msg,
            "Unexpected tokens (expected: archetypes, !identifiers, group, person, \
             softwareSystem, deploymentEnvironment, element, ->)"
        );
    }

    #[test]
    fn empty_permitted_lists_render_bare() {
        assert!(Ctx::Comment.permitted_tokens_message().is_none());
        assert!(Ctx::Archetypes.permitted_tokens_message().is_none());
        assert!(
            Ctx::Properties {
                holders: vec![Holder::Workspace]
            }
            .permitted_tokens_message()
            .is_none()
        );
        assert_eq!(Ctx::Users.permitted_tokens(), [] as [&str; 0]);
    }

    #[test]
    fn filtered_view_is_not_model_view() {
        let v = Ctx::FilteredView {
            view: ViewRef::Filtered(0),
        };
        assert!(!v.is_model_view());
        assert!(v.is_view());
    }

    #[test]
    fn no_rel_in_env_is_an_environment() {
        let v = Ctx::NoRelInEnv {
            environment: "Live".into(),
            linked: Id::new("1"),
            group: None,
        };
        assert!(v.is_environment());
        assert!(v.is_groupable());
        assert!(!v.is_element());
    }

    #[test]
    fn dynamic_rel_is_not_a_view_context() {
        let v = Ctx::DynamicRel {
            view: ViewRef::Dynamic(0),
            rel: Id::new("1"),
        };
        assert!(!v.is_view());
        assert_eq!(v.permitted_tokens(), vec!["url"]);
    }

    #[test]
    fn deployment_node_is_an_element() {
        let v = Ctx::DeploymentNode {
            id: Id::new("1"),
            group: None,
        };
        assert!(v.is_element());
        assert!(v.is_model_item());
        assert!(v.is_groupable());
    }

    #[test]
    fn single_relationship_context_is_model_item() {
        let v = Ctx::Rel { id: Id::new("1") };
        assert!(v.is_model_item());
        assert!(!v.is_element());
    }
}
