//! Literal values of the DSL token table (including the context brace
//! tokens). String constants, compared case-insensitively unless the caller
//! says otherwise — each call site must use the comparison the official
//! grammar uses there (exact string equality vs case-insensitive equality).
//!
//! The table is complete: tokens no parser section reads yet (directive and
//! style arms do) are kept here, so dead code is expected.
#![allow(dead_code)]

pub const CONTEXT_START: &str = "{";
pub const CONTEXT_END: &str = "}";
pub const ASSIGNMENT: &str = "=";

pub const THIS: &str = "this";

// model definitions
pub const CUSTOM_ELEMENT: &str = "element";
pub const PERSON: &str = "person";
pub const SOFTWARE_SYSTEM: &str = "softwareSystem";
pub const RELATIONSHIP: &str = "->";
pub const NO_RELATIONSHIP: &str = "-/>";
pub const CONTAINER: &str = "container";
pub const COMPONENT: &str = "component";
pub const GROUP: &str = "group";
pub const NAME: &str = "name";
pub const METADATA: &str = "metadata";
pub const DESCRIPTION: &str = "description";
pub const TECHNOLOGY: &str = "technology";
pub const INSTANCES: &str = "instances";
pub const TAGS: &str = "tags";
pub const TAG: &str = "tag";
pub const URL: &str = "url";
pub const PROPERTIES: &str = "properties";
pub const PERSPECTIVES: &str = "perspectives";
pub const PERSPECTIVE: &str = "perspective";
pub const PERSPECTIVE_DESCRIPTION: &str = "description";
pub const PERSPECTIVE_VALUE: &str = "value";
pub const PERSPECTIVE_URL: &str = "url";

// scopes
pub const WORKSPACE: &str = "workspace";
pub const EXTENDS: &str = "extends";
pub const SCOPE: &str = "scope";
pub const MODEL: &str = "model";
pub const ARCHETYPES: &str = "archetypes";
pub const VIEWS: &str = "views";
pub const ENTERPRISE: &str = "enterprise";

// deployment
pub const DEPLOYMENT_ENVIRONMENT: &str = "deploymentEnvironment";
pub const DEPLOYMENT_GROUP: &str = "deploymentGroup";
pub const DEPLOYMENT_NODE: &str = "deploymentNode";
pub const INFRASTRUCTURE_NODE: &str = "infrastructureNode";
pub const INSTANCE_OF: &str = "instanceOf";
pub const SOFTWARE_SYSTEM_INSTANCE: &str = "softwareSystemInstance";
pub const CONTAINER_INSTANCE: &str = "containerInstance";
pub const HEALTH_CHECK: &str = "healthCheck";

// views
pub const VIEW_CUSTOM: &str = "custom";
pub const VIEW_LANDSCAPE: &str = "systemLandscape";
pub const VIEW_CONTEXT: &str = "systemContext";
pub const VIEW_CONTAINER: &str = "container";
pub const VIEW_COMPONENT: &str = "component";
pub const VIEW_DYNAMIC: &str = "dynamic";
pub const VIEW_DEPLOYMENT: &str = "deployment";
pub const VIEW_FILTERED: &str = "filtered";
pub const VIEW_IMAGE: &str = "image";
pub const VIEW_INCLUDE: &str = "include";
pub const VIEW_EXCLUDE: &str = "exclude";
pub const VIEW_ANIMATION: &str = "animation";
pub const VIEW_ANIMATION_STEP: &str = "animationStep";
pub const VIEW_AUTOLAYOUT: &str = "autolayout";
pub const VIEW_DEFAULT: &str = "default";
pub const VIEW_TITLE: &str = "title";
pub const VIEW_DESCRIPTION: &str = "description";
pub const PLANTUML: &str = "plantuml";
pub const MERMAID: &str = "mermaid";
pub const KROKI: &str = "kroki";
pub const IMAGE: &str = "image";

// styles
pub const STYLES: &str = "styles";
pub const LIGHT: &str = "light";
pub const DARK: &str = "dark";
pub const THEME: &str = "theme";
pub const THEMES: &str = "themes";
pub const ELEMENT_STYLE: &str = "element";
pub const ELEMENT_STYLE_SHAPE: &str = "shape";
pub const ELEMENT_STYLE_BACKGROUND: &str = "background";
pub const ELEMENT_STYLE_STROKE: &str = "stroke";
pub const ELEMENT_STYLE_STROKE_WIDTH: &str = "strokeWidth";
pub const ELEMENT_STYLE_COLOUR: &str = "colour";
pub const ELEMENT_STYLE_COLOR: &str = "color";
pub const ELEMENT_STYLE_ICON: &str = "icon";
pub const ELEMENT_STYLE_ICON_POSITION: &str = "iconPosition";
pub const ELEMENT_STYLE_OPACITY: &str = "opacity";
pub const ELEMENT_STYLE_BORDER: &str = "border";
pub const ELEMENT_STYLE_FONT_SIZE: &str = "fontSize";
pub const ELEMENT_STYLE_WIDTH: &str = "width";
pub const ELEMENT_STYLE_HEIGHT: &str = "height";
pub const ELEMENT_STYLE_METADATA: &str = "metadata";
pub const ELEMENT_STYLE_DESCRIPTION: &str = "description";
pub const RELATIONSHIP_STYLE: &str = "relationship";
pub const RELATIONSHIP_STYLE_THICKNESS: &str = "thickness";
pub const RELATIONSHIP_STYLE_COLOR: &str = "color";
pub const RELATIONSHIP_STYLE_COLOUR: &str = "colour";
pub const RELATIONSHIP_STYLE_DASHED: &str = "dashed";
pub const RELATIONSHIP_STYLE_OPACITY: &str = "opacity";
pub const RELATIONSHIP_STYLE_ROUTING: &str = "routing";
pub const RELATIONSHIP_STYLE_JUMP: &str = "jump";
pub const RELATIONSHIP_STYLE_LINE_STYLE: &str = "style";
pub const RELATIONSHIP_STYLE_FONT_SIZE: &str = "fontSize";
pub const RELATIONSHIP_STYLE_WIDTH: &str = "width";
pub const RELATIONSHIP_STYLE_POSITION: &str = "position";

// configuration and terminology
pub const CONFIGURATION: &str = "configuration";
pub const VISIBILITY: &str = "visibility";
pub const TERMINOLOGY: &str = "terminology";
pub const TERMINOLOGY_RELATIONSHIP: &str = "relationship";
pub const METADATA_SYMBOLS: &str = "metadata";
pub const USERS: &str = "users";

// directives
pub const INCLUDE_FILE: &str = "!include";
pub const DOCS: &str = "!docs";
pub const DOCS_EXCLUDE: &str = "exclude";
pub const ADRS: &str = "!adrs";
pub const DECISIONS: &str = "!decisions";
pub const DECISIONS_EXCLUDE: &str = "exclude";
pub const CONSTANT: &str = "!constant";
pub const CONST: &str = "!const";
pub const VAR: &str = "!var";
pub const IDENTIFIERS: &str = "!identifiers";
pub const IMPLIED_RELATIONSHIPS: &str = "!impliedRelationships";
pub const REF: &str = "!ref";
pub const EXTEND: &str = "!extend";
pub const FIND_ELEMENT: &str = "!element";
pub const FIND_ELEMENTS: &str = "!elements";
pub const FIND_RELATIONSHIP: &str = "!relationship";
pub const FIND_RELATIONSHIPS: &str = "!relationships";
pub const PLUGIN: &str = "!plugin";
pub const SCRIPT: &str = "!script";
pub const COMPONENT_FINDER: &str = "!components";

/// Archetype arrow token shape: `--<name>->`.
pub const RELATIONSHIP_ARCHETYPE_PREFIX: &str = "--";
pub const RELATIONSHIP_ARCHETYPE_SUFFIX: &str = "->";
