//! JSON → `Workspace` (WRITE-CATALOG §6): a `serde_json::Value` walk with the
//! reference Jackson leniency matrix — unknown keys ignored
//! (`FAIL_ON_UNKNOWN_PROPERTIES=false`), unknown enum values → `None`
//! (`READ_UNKNOWN_ENUM_VALUES_AS_NULL`), empty/absent objects → `None`
//! (`ACCEPT_EMPTY_STRING_AS_NULL_OBJECT`), scalars coerced to strings exactly
//! like Jackson's `String` binding — followed by the `hydrate()` equivalent:
//! the M2 validation catalog (first message throws) and instance-name
//! derivation from the linked elements.
//!
//! Property maps are read in DOCUMENT order, which becomes insertion order in
//! [`oxidizr_core::properties::Properties`] — the writer replays that sequence
//! through the `HashMap` emulator, so read→write is byte-stable (§1).

use std::collections::BTreeMap;

use oxidizr_core::Workspace;
use oxidizr_core::configuration::{Role, User, Visibility, WorkspaceScope};
use oxidizr_core::docs::{Decision, Documentation, Format, Image, Link, Section};
use oxidizr_core::ids::Id;
use oxidizr_core::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, HttpHealthCheck,
    InfrastructureNode, Model, ModelItem, Person, Perspective, Relationship, SoftwareSystem,
    SoftwareSystemInstance,
};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::{InteractionStyle, Tags};
use oxidizr_core::views::{
    Animation, AutomaticLayout, ColorScheme, ComponentView, ContainerView, CustomView,
    DeploymentView, Dimensions, DynamicView, ElementStyle, ElementView, FilterMode, FilteredView,
    ImageView, LineStyle, PaperSize, RelationshipStyle, RelationshipView, Routing, Shape, Styles,
    SystemContextView, SystemLandscapeView, Terminology, Vertex, ViewBase, ViewConfiguration,
    ViewContent, ViewSet,
};
use oxidizr_core::{DEFAULT_ENVIRONMENT, DEFAULT_INSTANCES};
use serde_json::{Map, Value};

use crate::error::JsonError;
use crate::writer::id_ord;
use oxidizr_core::colors;

/// Deserialize a workspace JSON document (§6 leniency, then `hydrate()`).
pub fn from_slice(bytes: &[u8]) -> Result<Workspace, JsonError> {
    let root: Value = serde_json::from_slice(bytes)?;
    let obj = root
        .as_object()
        .ok_or_else(|| JsonError::Read("the workspace JSON must be an object".to_string()))?;
    let mut workspace = read_workspace(obj)?;
    // `hydrate()`: the reference validates and throws the FIRST message;
    // instance names are derived only after the link checks pass.
    if let Some(first) = oxidizr_core::validate(&workspace.model, &workspace.views).first() {
        return Err(JsonError::Validation(first.message.clone()));
    }
    derive_instance_names(&mut workspace);
    Ok(workspace)
}

// ---------------------------------------------------------------------------
// Scalar accessors (Jackson binding semantics)
// ---------------------------------------------------------------------------

pub(crate) type Obj<'a> = &'a Map<String, Value>;

fn read<'a>(o: Obj<'a>, key: &str) -> Option<&'a Value> {
    o.get(key).filter(|v| !v.is_null())
}

/// A `String` field: any scalar binds (numbers/bools stringify like Jackson's
/// `asText`); objects/arrays are the hostile-input case and bind as absent.
/// An empty string binds as `Some("")` — significant for the `NON_NULL`
/// style fields, byte-invisible for the `NON_EMPTY` ones.
pub(crate) fn text(o: Obj<'_>, key: &str) -> Option<String> {
    match read(o, key)? {
        Value::String(s) => Some(s.clone()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn id_field(o: Obj<'_>, key: &str) -> Option<Id> {
    text(o, key).map(Id::new)
}

/// Sentinel for a missing/unparsable id in the tolerant read (the reference
/// stores `null` there; the validation pass rejects the resulting dangling
/// links before anything else can observe the sentinel).
fn no_id() -> Id {
    Id::new(String::new())
}

fn bool_field(o: Obj<'_>, key: &str, default: bool) -> bool {
    read(o, key).and_then(Value::as_bool).unwrap_or(default)
}

fn int_field(o: Obj<'_>, key: &str, default: i64) -> i64 {
    match read(o, key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(default),
        Some(Value::String(s)) => s.parse().unwrap_or(default),
        _ => default,
    }
}

/// An `int` field (`Integer`/primitive binding): a number outside `i32` or an
/// unparsable string binds the default where Jackson would throw — accepted
/// hostile-input divergence, kept out of the `as`-cast business.
fn i32_field(o: Obj<'_>, key: &str, default: i32) -> i32 {
    match read(o, key) {
        Some(Value::Number(n)) => n
            .as_i64()
            .and_then(|v| i32::try_from(v).ok())
            .unwrap_or(default),
        Some(Value::String(s)) => s.parse().unwrap_or(default),
        _ => default,
    }
}

fn opt_i32_field(o: Obj<'_>, key: &str) -> Option<i32> {
    match read(o, key) {
        Some(Value::Number(n)) => n.as_i64().and_then(|v| i32::try_from(v).ok()),
        Some(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

fn str_array(o: Obj<'_>, key: &str) -> Vec<String> {
    read(o, key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|v| match v {
                    Value::String(s) => Some(s.clone()),
                    Value::Bool(b) => Some(b.to_string()),
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A free-form `properties` map: document order → insertion order (§1).
/// Null values bind a `null` entry in Java (unrepresentable in the model) —
/// they are skipped here, a hostile-input-only divergence.
fn props(o: Obj<'_>, key: &str) -> Properties {
    read(o, key)
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| match v {
                    Value::String(s) => Some((k.clone(), s.clone())),
                    Value::Bool(b) => Some((k.clone(), b.to_string())),
                    Value::Number(n) => Some((k.clone(), n.to_string())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn tags_field(o: Obj<'_>, key: &str) -> Tags {
    text(o, key).map_or_else(Tags::new, |csv| Tags::set_from_csv(&csv))
}

/// `yyyy-MM-dd'T'HH:mm:ss'Z'` (the writer's format) or epoch millis, as the
/// tolerant Jackson `Date` binding; stored as epoch SECONDS.
fn date_field(o: Obj<'_>, key: &str) -> Option<i64> {
    match read(o, key)? {
        Value::Number(n) => n.as_i64().map(|millis| millis.div_euclid(1000)),
        Value::String(s) => chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
            .map(|dt| dt.and_utc().timestamp())
            .ok()
            // all-digit strings bind as epoch millis like Jackson's DateDeserializer
            .or_else(|| s.parse::<i64>().ok().map(|millis| millis.div_euclid(1000))),
        _ => None,
    }
}

fn parse_enum<E>(o: Obj<'_>, key: &str, parse: fn(&str) -> Option<E>) -> Option<E> {
    text(o, key).as_deref().and_then(parse)
}

pub(crate) fn objects<'a>(o: Obj<'a>, key: &str) -> Vec<Obj<'a>> {
    read(o, key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Workspace root (§2)
// ---------------------------------------------------------------------------

fn read_workspace(o: Obj<'_>) -> Result<Workspace, JsonError> {
    Ok(Workspace {
        id: int_field(o, "id", 0),
        name: text(o, "name"),
        description: text(o, "description"),
        version: text(o, "version"),
        last_modified_date: date_field(o, "lastModifiedDate"),
        last_modified_user: text(o, "lastModifiedUser"),
        last_modified_agent: text(o, "lastModifiedAgent"),
        thumbnail: text(o, "thumbnail"),
        properties: props(o, "properties"),
        model: read_model(o.get("model").and_then(Value::as_object))?,
        views: read_views(o.get("views").and_then(Value::as_object))?,
        documentation: read_documentation(o.get("documentation").and_then(Value::as_object)),
        configuration: read_workspace_configuration(
            o.get("configuration").and_then(Value::as_object),
        ),
    })
}

/// `WorkspaceConfiguration`: only `scope`, `visibility`, `users` exist in v6
/// (§2); everything else is read-ignored legacy (`enterprise`, `branding`, …).
fn read_workspace_configuration(o: Option<Obj<'_>>) -> oxidizr_core::WorkspaceConfiguration {
    let Some(o) = o else {
        return oxidizr_core::WorkspaceConfiguration::default();
    };
    oxidizr_core::WorkspaceConfiguration {
        scope: parse_enum(o, "scope", WorkspaceScope::parse),
        visibility: parse_enum(o, "visibility", Visibility::parse),
        users: objects(o, "users")
            .into_iter()
            .map(|u| User {
                username: text(u, "username").unwrap_or_default(),
                role: parse_enum(u, "role", Role::parse),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Model (§2)
// ---------------------------------------------------------------------------

/// Shared `ModelItem` keys (`name`/`tags`/`description`/`url`/`properties`/
/// `perspectives`); instances pass `named: false` — their `name` is
/// `@JsonIgnore` and derived after validation (§6).
fn read_item(o: Obj<'_>, named: bool) -> ModelItem {
    ModelItem {
        id: id_field(o, "id").unwrap_or_else(no_id),
        name: if named {
            text(o, "name").unwrap_or_default()
        } else {
            String::new()
        },
        description: text(o, "description"),
        tags: tags_field(o, "tags"),
        url: text(o, "url"),
        properties: props(o, "properties"),
        perspectives: objects(o, "perspectives")
            .into_iter()
            .map(|p| Perspective {
                name: text(p, "name").unwrap_or_default(),
                description: text(p, "description"),
                value: text(p, "value"),
                url: text(p, "url"),
            })
            .collect(),
    }
}

/// Reads an element's nested `relationships[]` into the shared collector and
/// returns the link list (`Element.setRelationships`; the writer re-sorts).
fn read_relationships(o: Obj<'_>, rels: &mut Vec<Relationship>) -> Vec<Id> {
    let mut ids = Vec::new();
    for r in objects(o, "relationships") {
        let rel = Relationship {
            id: id_field(r, "id").unwrap_or_else(no_id),
            source: id_field(r, "sourceId").unwrap_or_else(no_id),
            destination: id_field(r, "destinationId").unwrap_or_else(no_id),
            description: text(r, "description"),
            technology: text(r, "technology"),
            tags: tags_field(r, "tags"),
            url: text(r, "url"),
            properties: props(r, "properties"),
            perspectives: objects(r, "perspectives")
                .into_iter()
                .map(|p| Perspective {
                    name: text(p, "name").unwrap_or_default(),
                    description: text(p, "description"),
                    value: text(p, "value"),
                    url: text(p, "url"),
                })
                .collect(),
            interaction_style: parse_enum(r, "interactionStyle", InteractionStyle::parse),
            linked_relationship_id: id_field(r, "linkedRelationshipId"),
        };
        ids.push(rel.id.clone());
        rels.push(rel);
    }
    ids
}

/// `DeploymentNode.instances` — the reference setter's validation, verbatim
/// (§6): `Integer.parseInt` first (only `< 1` throws), then on
/// `NumberFormatException` the `\d*\.\.\d*`, `\d*..N` and `\d*..\*` shapes
/// (the middle dots in the last two are UNESCAPED regex dots — reproduced
/// exactly, including backtracking so `"12N"` matches with an empty `\d*`).
fn check_instances(s: &str) -> Result<(), &'static str> {
    match s.parse::<i32>() {
        // Java `Integer.parseInt` accepted: only `< 1` throws. (`parseInt`
        // also accepts `+` and Unicode digits where Rust does not — an
        // accepted hostile-input divergence.)
        Ok(n) if n < 1 => Err(INSTANCES_MSG),
        Ok(_) => Ok(()),
        Err(_) => check_instances_range(s),
    }
}

const INSTANCES_MSG: &str = "Number of instances must be a positive integer or a range.";

fn check_instances_range(s: &str) -> Result<(), &'static str> {
    // `\d*\.\.\d*` — anchored digits, literal "..", anchored digits.
    if let Some((lo, hi)) = s.split_once("..")
        && !lo.contains('.')
        && !hi.contains('.')
        && lo.chars().all(|c| c.is_ascii_digit())
        && hi.chars().all(|c| c.is_ascii_digit())
    {
        // Java `Integer.parseInt`s both sides; an empty or overflowing side
        // throws there too (unchecked `NumberFormatException`/
        // `ArrayIndexOutOfBoundsException` — hostile-input-only divergence).
        let (Ok(lo), Ok(hi)) = (lo.parse::<i32>(), hi.parse::<i32>()) else {
            return Err(INSTANCES_MSG);
        };
        return if lo > hi {
            Err("Range upper bound must be greater than the lower bound.")
        } else {
            Ok(())
        };
    }
    // `\d*..N` / `\d*..\*` — the middle `..` are UNESCAPED regex dots
    // (any two non-newline characters); matched in the Java order.
    if two_star_shape(s, 'N') || two_star_shape(s, '*') {
        return Ok(());
    }
    Err(INSTANCES_MSG)
}

/// Java `matches("\\d*.." + last + "")`: longest digit run `d`, then exactly
/// two non-newline chars, then `last` — with backtracking, so a digit may be
/// consumed by the any-two part (`"12N"` matches with an empty `\d*`).
fn two_star_shape(s: &str, last: char) -> bool {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() < 3
        || cs[cs.len() - 1] != last
        || cs[cs.len() - 3] == '\n'
        || cs[cs.len() - 2] == '\n'
    {
        return false;
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    cs.len() - 3 <= digits
}

/// `healthChecks[]` (`StaticStructureElementInstance` only). `name` and `url`
/// are REQUIRED non-null values: `HttpHealthCheck.hashCode()` dereferences
/// both on the very first `HashSet` insert Jackson performs, so a missing or
/// null key NPEs the reference read (oracle witness 2026-11-14) — oxidizr
/// rejects it as a value error instead. `interval`/`timeout` default to the
/// bean's field initializers (0/0; the 60s interval is the DSL parser's).
fn health_checks(o: Obj<'_>) -> Result<Vec<HttpHealthCheck>, JsonError> {
    objects(o, "healthChecks")
        .into_iter()
        .map(|h| {
            let name = text(h, "name")
                .ok_or_else(|| JsonError::Read("the health check name must not be null".into()))?;
            let url = text(h, "url")
                .ok_or_else(|| JsonError::Read("the health check URL must not be null".into()))?;
            Ok(HttpHealthCheck {
                name,
                interval: i32_field(h, "interval", 0),
                timeout: int_field(h, "timeout", 0),
                url,
                // `headers` binds (document order; duplicate keys replace);
                // `TreeMap` sorting only affects the WRITE-time HashMap copy
                // the writer replays (§2 errata).
                headers: read(h, "headers")
                    .and_then(Value::as_object)
                    .map(|m| {
                        m.iter()
                            .filter_map(|(k, v)| {
                                v.as_str().map(str::to_string).map(|s| (k.clone(), s))
                            })
                            .collect::<BTreeMap<String, String>>()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

fn read_node(o: Obj<'_>, rels: &mut Vec<Relationship>) -> Result<DeploymentNode, JsonError> {
    let instances = match text(o, "instances") {
        Some(s) => {
            check_instances(&s).map_err(|m| JsonError::Read(m.to_string()))?;
            s
        }
        // The setter is not called when the key is absent — field default.
        None => DEFAULT_INSTANCES.to_string(),
    };
    Ok(DeploymentNode {
        item: read_item(o, true),
        group: text(o, "group"),
        environment: text(o, "environment").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string()),
        technology: text(o, "technology"),
        instances,
        deployment_groups: str_array(o, "deploymentGroups"),
        children: objects(o, "children")
            .into_iter()
            .map(|c| read_node(c, rels))
            .collect::<Result<_, _>>()?,
        infrastructure_nodes: objects(o, "infrastructureNodes")
            .into_iter()
            .map(|i| read_infra_node(i, rels))
            .collect(),
        software_system_instances: objects(o, "softwareSystemInstances")
            .into_iter()
            .map(|i| read_ssi(i, rels))
            .collect::<Result<_, _>>()?,
        container_instances: objects(o, "containerInstances")
            .into_iter()
            .map(|i| read_ci(i, rels))
            .collect::<Result<_, _>>()?,
        relationships: read_relationships(o, rels),
    })
}

fn read_infra_node(o: Obj<'_>, rels: &mut Vec<Relationship>) -> InfrastructureNode {
    InfrastructureNode {
        item: read_item(o, true),
        group: text(o, "group"),
        environment: text(o, "environment").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string()),
        technology: text(o, "technology"),
        relationships: read_relationships(o, rels),
    }
}

fn read_ssi(o: Obj<'_>, rels: &mut Vec<Relationship>) -> Result<SoftwareSystemInstance, JsonError> {
    Ok(SoftwareSystemInstance {
        item: read_item(o, false),
        group: text(o, "group"),
        environment: text(o, "environment").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string()),
        software_system_id: id_field(o, "softwareSystemId").unwrap_or_else(no_id),
        instance_id: i32_field(o, "instanceId", 0),
        deployment_groups: str_array(o, "deploymentGroups"),
        health_checks: health_checks(o)?,
        relationships: read_relationships(o, rels),
    })
}

fn read_ci(o: Obj<'_>, rels: &mut Vec<Relationship>) -> Result<ContainerInstance, JsonError> {
    Ok(ContainerInstance {
        item: read_item(o, false),
        group: text(o, "group"),
        environment: text(o, "environment").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string()),
        container_id: id_field(o, "containerId").unwrap_or_else(no_id),
        instance_id: i32_field(o, "instanceId", 0),
        deployment_groups: str_array(o, "deploymentGroups"),
        health_checks: health_checks(o)?,
        relationships: read_relationships(o, rels),
    })
}

fn read_model(o: Option<Obj<'_>>) -> Result<Model, JsonError> {
    let mut rels = Vec::new();
    let mut out = Model {
        custom_elements: Vec::new(),
        people: Vec::new(),
        software_systems: Vec::new(),
        deployment_nodes: Vec::new(),
        relationships: Vec::new(),
        properties: Properties::new(),
    };
    if let Some(o) = o {
        out.custom_elements = objects(o, "customElements")
            .into_iter()
            .map(|e| CustomElement {
                item: read_item(e, true),
                group: text(e, "group"),
                metadata: text(e, "metadata"),
                relationships: read_relationships(e, &mut rels),
            })
            .collect();
        out.deployment_nodes = objects(o, "deploymentNodes")
            .into_iter()
            .map(|n| read_node(n, &mut rels))
            .collect::<Result<_, _>>()?;
        out.people = objects(o, "people")
            .into_iter()
            .map(|p| Person {
                item: read_item(p, true),
                group: text(p, "group"),
                relationships: read_relationships(p, &mut rels),
            })
            .collect();
        out.properties = props(o, "properties");
        out.software_systems = objects(o, "softwareSystems")
            .into_iter()
            .map(|s| SoftwareSystem {
                item: read_item(s, true),
                group: text(s, "group"),
                containers: objects(s, "containers")
                    .into_iter()
                    .map(|c| Container {
                        item: read_item(c, true),
                        group: text(c, "group"),
                        technology: text(c, "technology"),
                        components: objects(c, "components")
                            .into_iter()
                            .map(|k| Component {
                                item: read_item(k, true),
                                group: text(k, "group"),
                                technology: text(k, "technology"),
                                documentation: read_documentation(
                                    k.get("documentation").and_then(Value::as_object),
                                ),
                                relationships: read_relationships(k, &mut rels),
                            })
                            .collect(),
                        documentation: read_documentation(
                            c.get("documentation").and_then(Value::as_object),
                        ),
                        relationships: read_relationships(c, &mut rels),
                    })
                    .collect(),
                documentation: read_documentation(
                    s.get("documentation").and_then(Value::as_object),
                ),
                relationships: read_relationships(s, &mut rels),
            })
            .collect();
    }
    // `Model.relationships` is a `TreeSet` (`Relationship.compareTo` = numeric
    // id) — validation message order follows it.
    rels.sort_by(|a, b| id_ord(&a.id, &b.id));
    out.relationships = rels;
    Ok(out)
}

/// Instance names are derived from the linked element (`getName` is
/// `@JsonIgnore`; after `hydrate()`'s link checks — §6). `validate()` has
/// already passed, so every link resolves.
fn derive_instance_names(workspace: &mut Workspace) {
    fn walk(names: &BTreeMap<&str, &str>, nodes: &mut [DeploymentNode]) {
        for node in nodes {
            for i in &mut node.software_system_instances {
                if let Some(name) = names.get(i.software_system_id.as_str()) {
                    i.item.name = (*name).to_string();
                }
            }
            for i in &mut node.container_instances {
                if let Some(name) = names.get(i.container_id.as_str()) {
                    i.item.name = (*name).to_string();
                }
            }
            walk(names, &mut node.children);
        }
    }

    let names: BTreeMap<&str, &str> = workspace
        .model
        .software_systems
        .iter()
        .flat_map(|s| {
            std::iter::once((s.item.id.as_str(), s.item.name.as_str())).chain(
                s.containers
                    .iter()
                    .map(|c| (c.item.id.as_str(), c.item.name.as_str())),
            )
        })
        .collect();
    walk(&names, &mut workspace.model.deployment_nodes);
}

// ---------------------------------------------------------------------------
// Views (§3)
// ---------------------------------------------------------------------------

fn read_view_base(o: Obj<'_>) -> ViewBase {
    ViewBase {
        key: text(o, "key").unwrap_or_default(),
        order: i32_field(o, "order", 0),
        title: text(o, "title"),
        description: text(o, "description"),
        generated_key: bool_field(o, "generatedKey", false),
        properties: props(o, "properties"),
    }
}

fn read_content(o: Obj<'_>) -> Result<ViewContent, JsonError> {
    Ok(ViewContent {
        elements: objects(o, "elements")
            .into_iter()
            .map(|e| ElementView {
                id: id_field(e, "id").unwrap_or_else(no_id),
                x: i32_field(e, "x", 0),
                y: i32_field(e, "y", 0),
            })
            .collect(),
        relationships: objects(o, "relationships")
            .into_iter()
            .map(|r| RelationshipView {
                id: id_field(r, "id").unwrap_or_else(no_id),
                description: text(r, "description"),
                url: text(r, "url"),
                order: text(r, "order"),
                response: read(r, "response").and_then(Value::as_bool),
                properties: props(r, "properties"),
                vertices: objects(r, "vertices")
                    .into_iter()
                    .map(|v| Vertex {
                        x: i32_field(v, "x", 0),
                        y: i32_field(v, "y", 0),
                    })
                    .collect(),
                routing: parse_enum(r, "routing", Routing::parse),
                jump: read(r, "jump").and_then(Value::as_bool),
                position: opt_i32_field(r, "position"),
            })
            .collect(),
        paper_size: parse_enum(o, "paperSize", PaperSize::parse),
        dimensions: read(o, "dimensions")
            .and_then(Value::as_object)
            .map(|d| Dimensions {
                width: i32_field(d, "width", 0),
                height: i32_field(d, "height", 0),
            }),
        // Presence enables layout; `applied`/`implementation`/`seed` are
        // read-ignored legacy (§6).
        automatic_layout: read(o, "automaticLayout")
            .and_then(Value::as_object)
            .map(read_automatic_layout)
            .transpose()?,
    })
}

/// The `AutomaticLayout` bean's read defaults are its Java field initializers
/// (0/0/0/false, `rankDirection` null — oracle probe 2026-11-14; the
/// LeftRight/100/50/50/true values are the DSL parser's, not the reader's).
/// The one guard is `setRankDirection(null)`: a PRESENT-but-unparsable value
/// (unknown enum → null under `READ_UNKNOWN_ENUM_VALUES_AS_NULL`) throws;
/// an absent key never calls the setter.
fn read_automatic_layout(l: Obj<'_>) -> Result<AutomaticLayout, JsonError> {
    let rank_direction = if l.contains_key("rankDirection") {
        Some(
            parse_enum(
                l,
                "rankDirection",
                oxidizr_core::views::RankDirection::parse,
            )
            .ok_or_else(|| JsonError::Read("A rank direction must be specified.".to_string()))?,
        )
    } else {
        None
    };
    Ok(AutomaticLayout {
        rank_direction,
        rank_separation: i32_field(l, "rankSeparation", 0),
        node_separation: i32_field(l, "nodeSeparation", 0),
        edge_separation: i32_field(l, "edgeSeparation", 0),
        vertices: bool_field(l, "vertices", false),
    })
}

fn read_animations(o: Obj<'_>) -> Vec<Animation> {
    objects(o, "animations")
        .into_iter()
        .map(|a| Animation {
            order: i32_field(a, "order", 0),
            element_ids: str_array(a, "elements"),
            relationship_ids: str_array(a, "relationships"),
        })
        .collect()
}

/// Java `Configuration.addTheme`: trim, skip empty; URLs deduplicate,
/// installed-theme names do not. Which names are "installed" is a
/// filesystem-dependent reference behavior (the unified app's themes dir) —
/// oxidizr keeps them (DEVIATIONS D3 tolerance; URLs, the byte-relevant
/// case, deduplicate exactly).
fn add_theme(themes: &mut Vec<String>, theme: &str) {
    let theme = theme.trim();
    if theme.is_empty() {
        return;
    }
    let is_url = theme.contains("://");
    if !is_url || !themes.iter().any(|t| t == theme) {
        themes.push(theme.to_string());
    }
}

fn read_view_configuration(o: Option<Obj<'_>>) -> Result<ViewConfiguration, JsonError> {
    let Some(o) = o else {
        return Ok(ViewConfiguration::default());
    };
    // `theme` (singular string) aliases into `themes[]` (`@JsonSetter`);
    // Jackson binds keys in DOCUMENT order, addTheme appends.
    let mut themes = Vec::new();
    for (key, value) in o {
        match key.as_str() {
            "theme" => {
                if let Some(t) = value.as_str() {
                    add_theme(&mut themes, t);
                }
            }
            "themes" => {
                if let Some(items) = value.as_array() {
                    for t in items.iter().filter_map(Value::as_str) {
                        add_theme(&mut themes, t);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(ViewConfiguration {
        styles: read_styles(o.get("styles").and_then(Value::as_object))?,
        themes,
        terminology: read_terminology(o.get("terminology").and_then(Value::as_object)),
        metadata_symbols: parse_enum(
            o,
            "metadataSymbols",
            oxidizr_core::views::MetadataSymbols::parse,
        ),
        default_view: text(o, "defaultView"),
        last_saved_view: text(o, "lastSavedView"),
        view_sort_order: parse_enum(
            o,
            "viewSortOrder",
            oxidizr_core::views::ViewSortOrder::parse,
        ),
        properties: props(o, "properties"),
    })
}

/// Style color fields (`background`/`stroke`/`color`) go through the Java
/// setter normalization: `#rrggbb` lowercased, HTML names mapped to lowercased
/// hex, anything else rejected (`"<color> is not a valid hex colour code or
/// HTML colour name."`). A present-but-null value hits the setter too — the
/// reference stringifies the null in its message.
fn style_color(o: Obj<'_>, key: &str) -> Result<Option<String>, JsonError> {
    if !o.contains_key(key) {
        return Ok(None);
    }
    let Some(color) = text(o, key) else {
        return Err(JsonError::Read(
            "null is not a valid hex colour code or HTML colour name.".to_string(),
        ));
    };
    colors::normalize(&color).map(Some).map_err(JsonError::Read)
}

/// `opt_i32_field` with the `JavaBean` setter clamps applied — Jackson binds
/// style numbers THROUGH the setters (WRITE-CATALOG §6 "read normalizes"):
/// `ElementStyle.setStrokeWidth` pins to [1, 10]; `setOpacity` (both styles)
/// and `RelationshipStyle.setPosition` pin to [0, 100]; null leaves the
/// field untouched (oracle probes 2026-11-17: `strokeWidth 0` reads back 1).
fn clamped_i32(o: Obj<'_>, key: &str, low: i32, high: i32) -> Option<i32> {
    opt_i32_field(o, key).map(|n| n.clamp(low, high))
}

fn read_styles(o: Option<Obj<'_>>) -> Result<Styles, JsonError> {
    let Some(o) = o else {
        return Ok(Styles::default());
    };
    Ok(Styles {
        element_styles: objects(o, "elements")
            .into_iter()
            .map(read_element_style)
            .collect::<Result<Vec<ElementStyle>, JsonError>>()?,
        relationship_styles: objects(o, "relationships")
            .into_iter()
            .map(read_relationship_style)
            .collect::<Result<Vec<RelationshipStyle>, JsonError>>()?,
        // `Styles.themes` has no JSON representation (no getter/setter).
        themes: Vec::new(),
    })
}

/// One `ElementStyle` bean (workspace `styles.elements[]` and the identical
/// `theme.json` shape; the setters normalize — see [`clamped_i32`] and
/// [`style_color`]).
pub(crate) fn read_element_style(s: Obj<'_>) -> Result<ElementStyle, JsonError> {
    Ok(ElementStyle {
        tag: text(s, "tag").unwrap_or_default(),
        color_scheme: parse_enum(s, "colorScheme", ColorScheme::parse),
        width: opt_i32_field(s, "width"),
        height: opt_i32_field(s, "height"),
        background: style_color(s, "background")?,
        stroke: style_color(s, "stroke")?,
        stroke_width: clamped_i32(s, "strokeWidth", 1, 10),
        color: style_color(s, "color")?,
        font_size: opt_i32_field(s, "fontSize"),
        shape: parse_enum(s, "shape", Shape::parse),
        icon: text(s, "icon"),
        icon_position: parse_enum(s, "iconPosition", oxidizr_core::views::IconPosition::parse),
        border: parse_enum(s, "border", LineStyle::parse),
        opacity: clamped_i32(s, "opacity", 0, 100),
        metadata: read(s, "metadata").and_then(Value::as_bool),
        description: read(s, "description").and_then(Value::as_bool),
        properties: props(s, "properties"),
    })
}

/// One `RelationshipStyle` bean (see [`read_element_style`]).
pub(crate) fn read_relationship_style(s: Obj<'_>) -> Result<RelationshipStyle, JsonError> {
    Ok(RelationshipStyle {
        tag: text(s, "tag").unwrap_or_default(),
        color_scheme: parse_enum(s, "colorScheme", ColorScheme::parse),
        thickness: opt_i32_field(s, "thickness"),
        color: style_color(s, "color")?,
        font_size: opt_i32_field(s, "fontSize"),
        width: opt_i32_field(s, "width"),
        dashed: read(s, "dashed").and_then(Value::as_bool),
        style: parse_enum(s, "style", LineStyle::parse),
        routing: parse_enum(s, "routing", Routing::parse),
        jump: read(s, "jump").and_then(Value::as_bool),
        position: clamped_i32(s, "position", 0, 100),
        opacity: clamped_i32(s, "opacity", 0, 100),
        metadata: read(s, "metadata").and_then(Value::as_bool),
        description: read(s, "description").and_then(Value::as_bool),
        properties: props(s, "properties"),
    })
}

fn read_terminology(o: Option<Obj<'_>>) -> Terminology {
    let Some(o) = o else {
        return Terminology::default();
    };
    Terminology {
        enterprise: text(o, "enterprise"),
        person: text(o, "person"),
        software_system: text(o, "softwareSystem"),
        container: text(o, "container"),
        component: text(o, "component"),
        code: text(o, "code"),
        deployment_node: text(o, "deploymentNode"),
        infrastructure_node: text(o, "infrastructureNode"),
        relationship: text(o, "relationship"),
    }
}

fn read_views(o: Option<Obj<'_>>) -> Result<ViewSet, JsonError> {
    let Some(o) = o else {
        return Ok(ViewSet::default());
    };
    let mut out = ViewSet {
        configuration: read_view_configuration(o.get("configuration").and_then(Value::as_object))?,
        ..ViewSet::default()
    };
    // The legacy `enterpriseContextViews` key REPLACES the landscape list
    // (`@JsonSetter`); document order decides which key binds last, so walk
    // the object in order rather than key-by-key.
    for (key, value) in o {
        let views = value.as_array();
        let items: Vec<Obj<'_>> = views
            .as_ref()
            .map(|a| a.iter().filter_map(Value::as_object).collect())
            .unwrap_or_default();
        match key.as_str() {
            "systemLandscapeViews" | "enterpriseContextViews" => {
                out.system_landscape_views = items
                    .into_iter()
                    .map(read_landscape_view)
                    .collect::<Result<_, _>>()?;
            }
            "systemContextViews" => {
                out.system_context_views = items
                    .into_iter()
                    .map(read_context_view)
                    .collect::<Result<_, _>>()?;
            }
            "containerViews" => {
                out.container_views = items
                    .into_iter()
                    .map(read_container_view)
                    .collect::<Result<_, _>>()?;
            }
            "componentViews" => {
                out.component_views = items
                    .into_iter()
                    .map(read_component_view)
                    .collect::<Result<_, _>>()?;
            }
            "dynamicViews" => {
                out.dynamic_views = items
                    .into_iter()
                    .map(read_dynamic_view)
                    .collect::<Result<_, _>>()?;
            }
            "deploymentViews" => {
                out.deployment_views = items
                    .into_iter()
                    .map(read_deployment_view)
                    .collect::<Result<_, _>>()?;
            }
            "filteredViews" => {
                out.filtered_views = items.into_iter().map(read_filtered_view).collect();
            }
            "imageViews" => {
                out.image_views = items.into_iter().map(read_image_view).collect();
            }
            "customViews" => {
                out.custom_views = items
                    .into_iter()
                    .map(read_custom_view)
                    .collect::<Result<_, _>>()?;
            }
            _ => {}
        }
    }
    Ok(out)
}

fn read_landscape_view(o: Obj<'_>) -> Result<SystemLandscapeView, JsonError> {
    Ok(SystemLandscapeView {
        base: read_view_base(o),
        content: read_content(o)?,
        enterprise_boundary_visible: bool_field(o, "enterpriseBoundaryVisible", true),
        animations: read_animations(o),
    })
}

fn read_context_view(o: Obj<'_>) -> Result<SystemContextView, JsonError> {
    Ok(SystemContextView {
        base: read_view_base(o),
        content: read_content(o)?,
        software_system_id: id_field(o, "softwareSystemId").unwrap_or_else(no_id),
        enterprise_boundary_visible: bool_field(o, "enterpriseBoundaryVisible", true),
        animations: read_animations(o),
    })
}

fn read_container_view(o: Obj<'_>) -> Result<ContainerView, JsonError> {
    Ok(ContainerView {
        base: read_view_base(o),
        content: read_content(o)?,
        software_system_id: id_field(o, "softwareSystemId").unwrap_or_else(no_id),
        external_software_system_boundaries_visible: bool_field(
            o,
            "externalSoftwareSystemBoundariesVisible",
            false,
        ),
        animations: read_animations(o),
    })
}

fn read_component_view(o: Obj<'_>) -> Result<ComponentView, JsonError> {
    Ok(ComponentView {
        base: read_view_base(o),
        content: read_content(o)?,
        container_id: id_field(o, "containerId").unwrap_or_else(no_id),
        external_container_boundaries_visible: bool_field(
            o,
            "externalContainerBoundariesVisible",
            false,
        ),
        animations: read_animations(o),
    })
}

fn read_dynamic_view(o: Obj<'_>) -> Result<DynamicView, JsonError> {
    Ok(DynamicView {
        base: read_view_base(o),
        content: read_content(o)?,
        element_id: id_field(o, "elementId"),
        // `deploymentEnvironment` is read-ignored legacy (§6).
        deployment_environment: None,
        external_boundaries_visible: bool_field(o, "externalBoundariesVisible", false),
    })
}

fn read_deployment_view(o: Obj<'_>) -> Result<DeploymentView, JsonError> {
    Ok(DeploymentView {
        base: read_view_base(o),
        content: read_content(o)?,
        environment: text(o, "environment").unwrap_or_else(|| DEFAULT_ENVIRONMENT.to_string()),
        software_system_id: id_field(o, "softwareSystemId"),
        animations: read_animations(o),
    })
}

fn read_custom_view(o: Obj<'_>) -> Result<CustomView, JsonError> {
    Ok(CustomView {
        base: read_view_base(o),
        content: read_content(o)?,
        animations: read_animations(o),
        software_system_id: id_field(o, "softwareSystemId"),
    })
}

fn read_filtered_view(o: Obj<'_>) -> FilteredView {
    FilteredView {
        base: read_view_base(o),
        base_view_key: text(o, "baseViewKey").unwrap_or_default(),
        mode: parse_enum(o, "mode", FilterMode::parse),
        tags: str_array(o, "tags"),
    }
}

fn read_image_view(o: Obj<'_>) -> ImageView {
    ImageView {
        base: read_view_base(o),
        element_id: id_field(o, "elementId"),
        content: text(o, "content"),
        content_light: text(o, "contentLight"),
        content_dark: text(o, "contentDark"),
        content_type: text(o, "contentType"),
    }
}

// ---------------------------------------------------------------------------
// Documentation (§4)
// ---------------------------------------------------------------------------

fn read_documentation(o: Option<Obj<'_>>) -> Documentation {
    let Some(o) = o else {
        return Documentation::default();
    };
    Documentation {
        sections: objects(o, "sections")
            .into_iter()
            .map(|s| Section {
                order: i32_field(s, "order", 0),
                filename: text(s, "filename"),
                content: text(s, "content").unwrap_or_default(),
                format: parse_enum(s, "format", Format::parse),
                element_id: id_field(s, "elementId"),
            })
            .collect(),
        decisions: objects(o, "decisions")
            .into_iter()
            .map(|d| Decision {
                id: text(d, "id").unwrap_or_default(),
                title: text(d, "title").unwrap_or_default(),
                status: text(d, "status").unwrap_or_default(),
                date: date_field(d, "date").unwrap_or_default(),
                links: objects(d, "links")
                    .into_iter()
                    .map(|l| Link {
                        id: text(l, "id").unwrap_or_default(),
                        description: text(l, "description").unwrap_or_default(),
                    })
                    .collect(),
                content: text(d, "content").unwrap_or_default(),
                format: parse_enum(d, "format", Format::parse),
                element_id: id_field(d, "elementId"),
            })
            .collect(),
        images: objects(o, "images")
            .into_iter()
            .map(|i| Image {
                name: text(i, "name").unwrap_or_default(),
                content: text(i, "content").unwrap_or_default(),
                content_type: text(i, "type"),
            })
            .collect(),
    }
}
