//! `Workspace` → the reference's exact JSON bytes (P1-D5 amended, P1-D6).
//!
//! Every rule here cites `agents/plan/build/WRITE-CATALOG.md` (sections §-quoted
//! below); the byte witness is `testdata/golden/m3-order.json`. The writer
//! builds a `printer::Json` tree with FINAL key order and omissions already
//! decided (Jackson `SORT_PROPERTIES_ALPHABETICALLY` + `NON_EMPTY`-family
//! per-field inclusion), then renders it with the Jackson pretty-printer
//! emulation.

use std::cmp::Ordering;
use std::collections::HashMap;

use oxidizr_core::Workspace;
use oxidizr_core::configuration::WorkspaceConfiguration;
use oxidizr_core::docs::{Decision, Documentation, Link, Section};
use oxidizr_core::ids::Id;
use oxidizr_core::model::{
    Component, Container, ContainerInstance, CustomElement, DeploymentNode, HttpHealthCheck,
    InfrastructureNode, Model, ModelItem, Person, Perspective, Relationship, SoftwareSystem,
    SoftwareSystemInstance,
};
use oxidizr_core::tags::{TagKind, relationship_defaults};
use oxidizr_core::views::{
    Animation, AutomaticLayout, ComponentView, ContainerView, CustomView, DeploymentView,
    Dimensions, DynamicView, ElementStyle, ElementView, FilteredView, ImageView, RelationshipStyle,
    RelationshipView, Styles, SystemContextView, SystemLandscapeView, Terminology, View, ViewBase,
    ViewConfiguration, ViewContent, ViewSet,
};

use crate::error::JsonError;
use crate::printer::{self, Json};

/// Serialize to the reference's exact bytes (no trailing newline, §0).
pub fn to_vec(workspace: &Workspace) -> Result<Vec<u8>, JsonError> {
    Ok(printer::to_bytes(&to_tree(workspace)?))
}

/// The writable `Json` tree behind [`to_vec`], exposed for the
/// `--keep-unknown` overlay (`keep.rs`, D3). Parity output is unaffected.
pub(crate) fn to_tree(workspace: &Workspace) -> Result<Json, JsonError> {
    let ctx = Ctx::new(workspace);
    ctx.workspace(workspace)
}

// ---------------------------------------------------------------------------
// Java ordering comparators (§5)
// ---------------------------------------------------------------------------

/// `String.compareTo`: UTF-16 code-unit lexicographic order (differs from
/// Rust byte order only for supplementary vs U+E000-class characters).
pub(crate) fn java_ord(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `String.compareToIgnoreCase`: UTF-16 units lowercased pairwise.
fn java_ord_ci(lhs: &str, rhs: &str) -> Ordering {
    let mut lu = lhs.encode_utf16();
    let mut ru = rhs.encode_utf16();
    loop {
        match (lu.next(), ru.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match lower_unit(x).cmp(&lower_unit(y)) {
                Ordering::Equal => {}
                ord => return ord,
            },
        }
    }
}

/// Java `Character.toLowerCase(char)` for BMP units (surrogates unchanged);
/// view keys are ASCII in practice — the approximation only reorders exotic
/// keys.
fn lower_unit(u: u16) -> u16 {
    match char::from_u32(u32::from(u)) {
        Some(c) => match c.to_lowercase().next() {
            Some(l) if u32::from(l) <= 0xFFFF => l as u16,
            _ => u,
        },
        None => u,
    }
}

/// `ModelItem.compareTo` (numeric id when both parse as Java ints, else
/// UTF-16 text); `Id`'s cached numeric view implements the same parse rule.
pub(crate) fn id_ord(a: &Id, b: &Id) -> Ordering {
    match (a.as_num(), b.as_num()) {
        (Some(x), Some(y)) => x.cmp(&y),
        _ => java_ord(a.as_str(), b.as_str()),
    }
}

/// `(order, key-ignore-case)` — the `View.compareTo` `TreeSet` order (§5).
fn view_cmp(a: View<'_>, b: View<'_>) -> Ordering {
    a.order()
        .cmp(&b.order())
        .then(java_ord_ci(a.key(), b.key()))
}

/// Render a `TreeSet`-serialized list: sorted copy, `None` when empty
/// (`NON_EMPTY`).
fn set_list<T, F, R>(items: &[T], mut cmp: F, mut render: R) -> Result<Option<Json>, JsonError>
where
    F: FnMut(&T, &T) -> Ordering,
    R: FnMut(&T) -> Result<Json, JsonError>,
{
    if items.is_empty() {
        return Ok(None);
    }
    let mut sorted: Vec<&T> = items.iter().collect();
    sorted.sort_by(|a, b| cmp(*a, *b));
    sorted
        .iter()
        .map(|item| render(item))
        .collect::<Result<Vec<_>, _>>()
        .map(|arr| Some(Json::Arr(arr)))
}

/// Render a stored-order list, `None` when empty (`NON_EMPTY`). Styles keep
/// their loaded order: the reference JSON reader field-binds `Styles.elements`
/// (no setter — the `TreeSet` is replaced, document order survives), while the
/// DSL path adds into the `TreeSet` and the engine inserts sorted (§5, P2
/// oracle probes 2026-11-17).
fn list<T, R>(items: &[T], mut render: R) -> Result<Option<Json>, JsonError>
where
    R: FnMut(&T) -> Result<Json, JsonError>,
{
    if items.is_empty() {
        return Ok(None);
    }
    items
        .iter()
        .map(&mut render)
        .collect::<Result<Vec<_>, _>>()
        .map(|arr| Some(Json::Arr(arr)))
}

/// A `TreeSet<String>` array (UTF-16 order), `None` when empty.
fn sorted_str_set(items: &[String]) -> Option<Json> {
    if items.is_empty() {
        return None;
    }
    let mut sorted: Vec<&str> = items.iter().map(String::as_str).collect();
    sorted.sort_by(|a, b| java_ord(a, b));
    Some(Json::Arr(
        sorted
            .into_iter()
            .map(|s| Json::Str(s.to_string()))
            .collect(),
    ))
}

/// Insert `entry` into an alphabetically-sorted key list (`String.compareTo`
/// order — the key tables of WRITE-CATALOG §2/§3).
fn insert_sorted(
    entries: &mut Vec<(&'static str, Option<Json>)>,
    entry: (&'static str, Option<Json>),
) {
    let pos = entries
        .binary_search_by(|(k, _)| java_ord(k, entry.0))
        .unwrap_or_else(|p| p);
    entries.insert(pos, entry);
}

/// `NON_NULL` string: a `Some` value is written even when empty (style fields
/// carry `@JsonInclude(NON_NULL)`, V-2 note).
pub(crate) fn opt_nnn(s: Option<&str>) -> Option<Json> {
    s.map(|t| Json::Str(t.to_string()))
}

/// `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC (`SimpleDateFormat` on the mapper, §0).
fn opt_date(secs: Option<i64>) -> Result<Option<Json>, JsonError> {
    let Some(secs) = secs else { return Ok(None) };
    Ok(Some(Json::Str(date_str(secs)?)))
}

fn date_str(secs: i64) -> Result<String, JsonError> {
    chrono::DateTime::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .ok_or_else(|| JsonError::Write(format!("date {secs} is out of range for serialization")))
}

fn missing(what: &str, id: &Id) -> JsonError {
    JsonError::Write(format!(
        "the {what} with id \"{id}\" referenced by a view does not exist"
    ))
}

// ---------------------------------------------------------------------------
// Lookups (computed view names §3.1, relationship arena)
// ---------------------------------------------------------------------------

struct ElemRef<'a> {
    name: &'a str,
    is_container: bool,
}

/// Id/name lookups needed for computed names plus the relationship arena.
struct Ctx<'a> {
    elem: HashMap<&'a str, ElemRef<'a>>,
    elem_parent: HashMap<&'a str, &'a str>,
    views_by_key: HashMap<&'a str, View<'a>>,
    rels: HashMap<&'a str, &'a Relationship>,
}

/// Walk every element: item, `is_container`, static parent id (hydrate walk
/// order — the writer only needs the resulting maps).
fn visit_items<'a>(model: &'a Model, f: &mut impl FnMut(&'a ModelItem, bool, Option<&'a Id>)) {
    for e in &model.custom_elements {
        f(&e.item, false, None);
    }
    for p in &model.people {
        f(&p.item, false, None);
    }
    for s in &model.software_systems {
        f(&s.item, false, None);
        for c in &s.containers {
            f(&c.item, true, Some(&s.item.id));
            for k in &c.components {
                f(&k.item, false, Some(&c.item.id));
            }
        }
    }
    for n in &model.deployment_nodes {
        visit_deployment_node(n, f);
    }
}

/// Depth-first node walk (self, instances, infra nodes, then children).
fn visit_deployment_node<'a>(
    n: &'a DeploymentNode,
    f: &mut impl FnMut(&'a ModelItem, bool, Option<&'a Id>),
) {
    f(&n.item, false, None);
    for i in &n.software_system_instances {
        f(&i.item, false, None);
    }
    for i in &n.container_instances {
        f(&i.item, false, None);
    }
    for i in &n.infrastructure_nodes {
        f(&i.item, false, None);
    }
    for c in &n.children {
        visit_deployment_node(c, f);
    }
}

/// Every view of the set, unsorted.
fn all_views(views: &ViewSet) -> Vec<View<'_>> {
    let mut out = Vec::new();
    out.extend(views.custom_views.iter().map(View::Custom));
    out.extend(views.system_landscape_views.iter().map(View::Landscape));
    out.extend(views.system_context_views.iter().map(View::Context));
    out.extend(views.container_views.iter().map(View::Container));
    out.extend(views.component_views.iter().map(View::Component));
    out.extend(views.dynamic_views.iter().map(View::Dynamic));
    out.extend(views.deployment_views.iter().map(View::Deployment));
    out.extend(views.filtered_views.iter().map(View::Filtered));
    out.extend(views.image_views.iter().map(View::Image));
    out
}

impl<'a> Ctx<'a> {
    fn new(workspace: &'a Workspace) -> Ctx<'a> {
        let mut elem = HashMap::new();
        let mut elem_parent = HashMap::new();
        visit_items(&workspace.model, &mut |item, is_container, parent| {
            elem.insert(
                item.id.as_str(),
                ElemRef {
                    name: &item.name,
                    is_container,
                },
            );
            if let Some(p) = parent {
                elem_parent.insert(item.id.as_str(), p.as_str());
            }
        });
        let mut views_by_key = HashMap::new();
        for v in all_views(&workspace.views) {
            views_by_key.insert(v.key(), v);
        }
        let mut rels = HashMap::new();
        for r in &workspace.model.relationships {
            rels.insert(r.id.as_str(), r);
        }
        Ctx {
            elem,
            elem_parent,
            views_by_key,
            rels,
        }
    }

    fn elem_name(&self, id: &Id) -> Result<&'a str, JsonError> {
        self.elem
            .get(id.as_str())
            .map(|e| e.name)
            .ok_or_else(|| missing("element", id))
    }

    /// The computed `name` of a view (§3.1 — every formula mirrors the Java
    /// `getName()` override verbatim).
    fn view_name(&self, v: View<'a>) -> Result<String, JsonError> {
        Ok(match v {
            View::Landscape(_) => "System Landscape View".to_string(),
            View::Context(x) => {
                format!(
                    "System Context View: {}",
                    self.elem_name(&x.software_system_id)?
                )
            }
            View::Container(x) => {
                format!("Container View: {}", self.elem_name(&x.software_system_id)?)
            }
            View::Component(x) => {
                let container = self.elem_name(&x.container_id)?;
                let system = self
                    .parent_name(&x.container_id)
                    .ok_or_else(|| missing("parent software system", &x.container_id))?;
                format!("Component View: {system} - {container}")
            }
            View::Dynamic(x) => match &x.element_id {
                Some(id) => {
                    let e = self
                        .elem
                        .get(id.as_str())
                        .ok_or_else(|| missing("element", id))?;
                    if e.is_container {
                        let parent = self
                            .parent_name(id)
                            .ok_or_else(|| missing("parent software system", id))?;
                        format!("Dynamic View: {parent} - {}", e.name)
                    } else {
                        format!("Dynamic View: {}", e.name)
                    }
                }
                None => "Dynamic View".to_string(),
            },
            View::Deployment(x) => match &x.software_system_id {
                Some(id) => format!(
                    "Deployment View: {} - {}",
                    self.elem_name(id)?,
                    x.environment
                ),
                None => format!("Deployment View: {}", x.environment),
            },
            View::Filtered(x) => {
                // `"Filtered: " + baseView.getName()`; the reference NPEs on
                // a missing base during export — we return a clean error.
                let base = self
                    .views_by_key
                    .get(x.base_view_key.as_str())
                    .ok_or_else(|| {
                        JsonError::Write(format!(
                            "the base view \"{}\" of filtered view \"{}\" does not exist",
                            x.base_view_key, x.base.key
                        ))
                    })?;
                format!("Filtered: {}", self.view_name(*base)?)
            }
            // `"Custom View: " + getTitle()` — Java string concatenation of
            // null yields the literal "null" (bug-compatible, §3.1).
            View::Custom(x) => {
                format!("Custom View: {}", x.base.title.as_deref().unwrap_or("null"))
            }
            // ImageView.name == title verbatim; a null title hides the key.
            View::Image(x) => x.base.title.clone().unwrap_or_default(),
        })
    }

    fn parent_name(&self, id: &Id) -> Option<&'a str> {
        self.elem_parent
            .get(id.as_str())
            .and_then(|p| self.elem.get(*p))
            .map(|e| e.name)
    }
}

// ---------------------------------------------------------------------------
// Workspace root (§2)
// ---------------------------------------------------------------------------

impl Ctx<'_> {
    fn workspace(&self, ws: &Workspace) -> Result<Json, JsonError> {
        Ok(Json::obj(vec![
            (
                "configuration",
                Some(workspace_configuration(&ws.configuration)),
            ),
            ("description", Json::opt_text(ws.description.as_deref())),
            ("documentation", Some(documentation(&ws.documentation))),
            ("id", Some(Json::Num(ws.id))),
            (
                "lastModifiedAgent",
                Json::opt_text(ws.last_modified_agent.as_deref()),
            ),
            ("lastModifiedDate", opt_date(ws.last_modified_date)?),
            (
                "lastModifiedUser",
                Json::opt_text(ws.last_modified_user.as_deref()),
            ),
            ("model", Some(self.model(&ws.model)?)),
            ("name", Json::opt_text(ws.name.as_deref())),
            ("properties", Json::props(&ws.properties)),
            ("thumbnail", Json::opt_text(ws.thumbnail.as_deref())),
            ("version", Json::opt_text(ws.version.as_deref())),
            ("views", Some(self.views(&ws.views)?)),
        ]))
    }
}

/// `users` is a `TreeSet` by username (`User.compareTo`).
fn workspace_configuration(config: &WorkspaceConfiguration) -> Json {
    let users = set_list(
        &config.users,
        |a, b| java_ord(&a.username, &b.username),
        |u| {
            Ok(Json::obj(vec![
                ("role", u.role.map(|r| Json::Str(r.as_str().to_string()))),
                ("username", Json::text(&u.username)),
            ]))
        },
    )
    .unwrap_or(None); // infallible renderer
    Json::obj(vec![
        (
            "scope",
            config.scope.map(|s| Json::Str(s.as_str().to_string())),
        ),
        ("users", users),
        (
            "visibility",
            config.visibility.map(|v| Json::Str(v.as_str().to_string())),
        ),
    ])
}

// ---------------------------------------------------------------------------
// Model (§2)
// ---------------------------------------------------------------------------

impl Ctx<'_> {
    fn model(&self, model: &Model) -> Result<Json, JsonError> {
        // Alphabetical: customElements, deploymentNodes, people, properties,
        // softwareSystems (§2).
        Ok(Json::obj(vec![
            (
                "customElements",
                set_list(
                    &model.custom_elements,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |e| Ok(self.custom_element(e)),
                )?,
            ),
            (
                "deploymentNodes",
                set_list(
                    &model.deployment_nodes,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |n| Ok(self.node(n)),
                )?,
            ),
            (
                "people",
                set_list(
                    &model.people,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |p| Ok(self.person(p)),
                )?,
            ),
            ("properties", Json::props(&model.properties)),
            (
                "softwareSystems",
                set_list(
                    &model.software_systems,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |s| self.software_system(s),
                )?,
            ),
        ]))
    }

    /// `ModelItem` + `Element` shared keys (alphabetical, §2). `kind`
    /// supplies the derived default tag stack (Locked, `tags.rs`).
    fn item_entries(
        &self,
        item: &ModelItem,
        kind: TagKind,
        rels: &[Id],
    ) -> Vec<(&'static str, Option<Json>)> {
        vec![
            ("description", Json::opt_text(item.description.as_deref())),
            ("id", Some(Json::Str(item.id.as_str().to_string()))),
            ("name", Json::text(&item.name)),
            ("perspectives", perspectives(&item.perspectives)),
            ("properties", Json::props(&item.properties)),
            ("relationships", self.relationships(rels)),
            ("tags", Json::text(&item.tags.serialized_for(kind))),
            ("url", Json::opt_text(item.url.as_deref())),
        ]
    }

    fn person(&self, p: &Person) -> Json {
        let mut entries = self.item_entries(&p.item, TagKind::Person, &p.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(p.group.as_deref())));
        Json::from_entries(&entries)
    }

    fn custom_element(&self, e: &CustomElement) -> Json {
        let mut entries = self.item_entries(&e.item, TagKind::CustomElement, &e.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(e.group.as_deref())));
        insert_sorted(
            &mut entries,
            ("metadata", Json::opt_text(e.metadata.as_deref())),
        );
        Json::from_entries(&entries)
    }

    fn software_system(&self, s: &SoftwareSystem) -> Result<Json, JsonError> {
        let mut entries = self.item_entries(&s.item, TagKind::SoftwareSystem, &s.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(s.group.as_deref())));
        insert_sorted(
            &mut entries,
            (
                "containers",
                set_list(
                    &s.containers,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |c| self.container(c),
                )?,
            ),
        );
        insert_sorted(
            &mut entries,
            ("documentation", Some(documentation(&s.documentation))),
        );
        Ok(Json::from_entries(&entries))
    }

    fn container(&self, c: &Container) -> Result<Json, JsonError> {
        let mut entries = self.item_entries(&c.item, TagKind::Container, &c.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(c.group.as_deref())));
        insert_sorted(
            &mut entries,
            (
                "components",
                set_list(
                    &c.components,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |k| Ok(self.component(k)),
                )?,
            ),
        );
        insert_sorted(
            &mut entries,
            ("documentation", Some(documentation(&c.documentation))),
        );
        insert_sorted(
            &mut entries,
            ("technology", Json::opt_text(c.technology.as_deref())),
        );
        Ok(Json::from_entries(&entries))
    }

    fn component(&self, k: &Component) -> Json {
        let mut entries = self.item_entries(&k.item, TagKind::Component, &k.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(k.group.as_deref())));
        insert_sorted(
            &mut entries,
            ("documentation", Some(documentation(&k.documentation))),
        );
        insert_sorted(
            &mut entries,
            ("technology", Json::opt_text(k.technology.as_deref())),
        );
        Json::from_entries(&entries)
    }

    fn node(&self, n: &DeploymentNode) -> Json {
        let mut entries = self.item_entries(&n.item, TagKind::DeploymentNode, &n.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(n.group.as_deref())));
        insert_sorted(
            &mut entries,
            (
                "children",
                set_list(
                    &n.children,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |c| Ok(self.node(c)),
                )
                .unwrap_or(None),
            ),
        );
        insert_sorted(
            &mut entries,
            ("deploymentGroups", sorted_str_set(&n.deployment_groups)),
        );
        insert_sorted(&mut entries, ("environment", Json::text(&n.environment)));
        insert_sorted(
            &mut entries,
            (
                "infrastructureNodes",
                set_list(
                    &n.infrastructure_nodes,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |i| Ok(self.infra_node(i)),
                )
                .unwrap_or(None),
            ),
        );
        // `instances` is a Java String; NON_EMPTY hides "", default "1".
        insert_sorted(&mut entries, ("instances", Json::text(&n.instances)));
        insert_sorted(
            &mut entries,
            (
                "softwareSystemInstances",
                set_list(
                    &n.software_system_instances,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |i| Ok(self.ssi(i)),
                )
                .unwrap_or(None),
            ),
        );
        insert_sorted(
            &mut entries,
            (
                "containerInstances",
                set_list(
                    &n.container_instances,
                    |a, b| id_ord(&a.item.id, &b.item.id),
                    |i| Ok(self.ci(i)),
                )
                .unwrap_or(None),
            ),
        );
        insert_sorted(
            &mut entries,
            ("technology", Json::opt_text(n.technology.as_deref())),
        );
        Json::from_entries(&entries)
    }

    fn infra_node(&self, n: &InfrastructureNode) -> Json {
        let mut entries = self.item_entries(&n.item, TagKind::InfrastructureNode, &n.relationships);
        insert_sorted(&mut entries, ("group", Json::opt_text(n.group.as_deref())));
        insert_sorted(&mut entries, ("environment", Json::text(&n.environment)));
        insert_sorted(
            &mut entries,
            ("technology", Json::opt_text(n.technology.as_deref())),
        );
        Json::from_entries(&entries)
    }

    /// Instances: shared keys MINUS `name` (getName `@JsonIgnore`, §2) plus
    /// the static link; `instanceId` is an int, always written.
    fn ssi(&self, i: &SoftwareSystemInstance) -> Json {
        let mut entries = self.instance_entries(&i.item, &i.relationships);
        insert_sorted(
            &mut entries,
            (
                "softwareSystemId",
                Some(Json::Str(i.software_system_id.as_str().to_string())),
            ),
        );
        Self::deployment_instance_tail(
            &mut entries,
            &i.environment,
            &i.deployment_groups,
            &i.health_checks,
            i.instance_id,
            i.group.as_ref(),
        );
        Json::from_entries(&entries)
    }

    fn ci(&self, i: &ContainerInstance) -> Json {
        let mut entries = self.instance_entries(&i.item, &i.relationships);
        insert_sorted(
            &mut entries,
            (
                "containerId",
                Some(Json::Str(i.container_id.as_str().to_string())),
            ),
        );
        Self::deployment_instance_tail(
            &mut entries,
            &i.environment,
            &i.deployment_groups,
            &i.health_checks,
            i.instance_id,
            i.group.as_ref(),
        );
        Json::from_entries(&entries)
    }

    fn instance_entries(&self, item: &ModelItem, rels: &[Id]) -> Vec<(&'static str, Option<Json>)> {
        // `serialized_for` on the instance kinds has an EMPTY default stack
        // (the kind tag is stored at creation — tags.rs).
        vec![
            ("description", Json::opt_text(item.description.as_deref())),
            ("id", Some(Json::Str(item.id.as_str().to_string()))),
            ("perspectives", perspectives(&item.perspectives)),
            ("properties", Json::props(&item.properties)),
            ("relationships", self.relationships(rels)),
            ("tags", Json::text(&item.tags.serialized(&[]))),
            ("url", Json::opt_text(item.url.as_deref())),
        ]
    }

    #[allow(clippy::too_many_arguments)]
    fn deployment_instance_tail(
        entries: &mut Vec<(&'static str, Option<Json>)>,
        environment: &str,
        groups: &[String],
        checks: &[HttpHealthCheck],
        instance_id: i32,
        group: Option<&String>,
    ) {
        insert_sorted(entries, ("deploymentGroups", sorted_str_set(groups)));
        insert_sorted(entries, ("environment", Json::text(environment)));
        insert_sorted(entries, ("group", group.and_then(|g| Json::text(g))));
        insert_sorted(entries, ("healthChecks", health_checks(checks)));
        insert_sorted(
            entries,
            ("instanceId", Some(Json::Num(i64::from(instance_id)))),
        );
    }

    /// Nested `relationships[]` (`TreeSet` numeric order, §2/§5).
    fn relationships(&self, ids: &[Id]) -> Option<Json> {
        if ids.is_empty() {
            return None;
        }
        let mut rels: Vec<&Relationship> = ids
            .iter()
            .filter_map(|id| self.rels.get(id.as_str()).copied())
            .collect();
        rels.sort_by(|a, b| id_ord(&a.id, &b.id));
        Some(Json::Arr(rels.into_iter().map(relationship).collect()))
    }
}

fn relationship(r: &Relationship) -> Json {
    // Linked (implied / instance-replicated) rels carry NO default tags
    // (§2, `relationship_defaults`).
    let defaults = relationship_defaults(r.linked_relationship_id.is_some(), r.interaction_style);
    Json::obj(vec![
        ("description", Json::opt_text(r.description.as_deref())),
        (
            "destinationId",
            Some(Json::Str(r.destination.as_str().to_string())),
        ),
        ("id", Some(Json::Str(r.id.as_str().to_string()))),
        (
            "interactionStyle",
            r.interaction_style
                .map(|s| Json::Str(s.as_str().to_string())),
        ),
        (
            "linkedRelationshipId",
            r.linked_relationship_id
                .as_ref()
                .map(|i| Json::Str(i.as_str().to_string())),
        ),
        ("perspectives", perspectives(&r.perspectives)),
        ("properties", Json::props(&r.properties)),
        ("sourceId", Some(Json::Str(r.source.as_str().to_string()))),
        ("tags", Json::text(&r.tags.serialized(&defaults))),
        ("technology", Json::opt_text(r.technology.as_deref())),
        ("url", Json::opt_text(r.url.as_deref())),
    ])
}

/// `TreeSet` by name (§2).
fn perspectives(items: &[Perspective]) -> Option<Json> {
    set_list(
        items,
        |a, b| java_ord(&a.name, &b.name),
        |p| {
            Ok(Json::obj(vec![
                ("description", Json::opt_text(p.description.as_deref())),
                ("name", Json::text(&p.name)),
                ("url", Json::opt_text(p.url.as_deref())),
                ("value", Json::opt_text(p.value.as_deref())),
            ]))
        },
    )
    .unwrap_or(None) // infallible renderer
}

/// `TreeSet` by name then url (`HttpHealthCheck.compareTo`); `headers` keys go
/// through `getHeaders()`' `new HashMap<>(TreeMap)` copy, so Jackson emits
/// them in `HashMap` BUCKET order replayed from the UTF-16-sorted insertion
/// sequence (§2 errata, oracle probe 2026-11-14: `{"X-A","X-B"}` writes as
/// `X-B`, `X-A`); `interval`/`timeout` are primitives — always written (§2).
fn health_checks(items: &[HttpHealthCheck]) -> Option<Json> {
    set_list(
        items,
        |a, b| java_ord(&a.name, &b.name).then_with(|| java_ord(&a.url, &b.url)),
        |h| {
            let mut headers: Vec<(&String, &String)> = h.headers.iter().collect();
            headers.sort_by(|a, b| java_ord(a.0, b.0));
            let keys: Vec<&str> = headers.iter().map(|(k, _)| k.as_str()).collect();
            let headers: Vec<(&String, &String)> = crate::javahash::entry_order(&keys)
                .into_iter()
                .map(|i| headers[i])
                .collect();
            let mut entries = vec![
                ("interval", Some(Json::Num(i64::from(h.interval)))),
                ("name", Json::text(&h.name)),
                ("timeout", Some(Json::Num(h.timeout))),
                ("url", Json::text(&h.url)),
            ];
            if !headers.is_empty() {
                insert_sorted(
                    &mut entries,
                    (
                        "headers",
                        Some(Json::Obj(
                            headers
                                .into_iter()
                                .map(|(k, v)| (k.clone(), Json::Str(v.clone())))
                                .collect(),
                        )),
                    ),
                );
            }
            Ok(Json::obj(entries))
        },
    )
    .unwrap_or(None) // infallible renderer
}

// ---------------------------------------------------------------------------
// Documentation (§4)
// ---------------------------------------------------------------------------

fn documentation(doc: &Documentation) -> Json {
    // Sections/decisions/images keep DEFINITION order (`ArrayList`, §4) —
    // the `Equal` comparator keeps the stable sort a no-op.
    Json::obj(vec![
        (
            "decisions",
            set_list(&doc.decisions, |_, _| Ordering::Equal, decision).unwrap_or(None),
        ),
        (
            "images",
            set_list(
                &doc.images,
                |_, _| Ordering::Equal,
                |i| {
                    Ok(Json::obj(vec![
                        ("content", Json::text(&i.content)),
                        ("name", Json::text(&i.name)),
                        ("type", Json::opt_text(i.content_type.as_deref())),
                    ]))
                },
            )
            .unwrap_or(None),
        ),
        (
            "sections",
            set_list(&doc.sections, |_, _| Ordering::Equal, |s| Ok(section(s))).unwrap_or(None),
        ),
    ])
}

fn section(s: &Section) -> Json {
    Json::obj(vec![
        ("content", Json::text(&s.content)),
        (
            "elementId",
            s.element_id
                .as_ref()
                .map(|i| Json::Str(i.as_str().to_string())),
        ),
        ("filename", Json::opt_text(s.filename.as_deref())),
        (
            "format",
            s.format.map(|f| Json::Str(f.as_str().to_string())),
        ),
        ("order", Some(Json::Num(i64::from(s.order)))),
        // `@JsonInclude(ALWAYS)` back-compat getter: always written, "" (§4).
        ("title", Some(Json::Str(String::new()))),
    ])
}

fn decision(d: &Decision) -> Result<Json, JsonError> {
    Ok(Json::obj(vec![
        ("content", Json::text(&d.content)),
        ("date", Some(Json::Str(date_str(d.date)?))),
        (
            "elementId",
            d.element_id
                .as_ref()
                .map(|i| Json::Str(i.as_str().to_string())),
        ),
        (
            "format",
            d.format.map(|f| Json::Str(f.as_str().to_string())),
        ),
        ("id", Json::text(&d.id)),
        (
            "links",
            set_list(&d.links, |_, _| Ordering::Equal, |l| Ok(link(l))).unwrap_or(None),
        ),
        ("status", Json::text(&d.status)),
        ("title", Json::text(&d.title)),
    ]))
}

fn link(l: &Link) -> Json {
    Json::obj(vec![
        ("description", Json::text(&l.description)),
        ("id", Json::text(&l.id)),
    ])
}

// ---------------------------------------------------------------------------
// Views (§3)
// ---------------------------------------------------------------------------

impl Ctx<'_> {
    fn views(&self, views: &ViewSet) -> Result<Json, JsonError> {
        Ok(Json::obj(vec![
            (
                "componentViews",
                set_list(
                    &views.component_views,
                    |a, b| view_cmp(View::Component(a), View::Component(b)),
                    |v| self.component_view(v),
                )?,
            ),
            (
                "configuration",
                Some(Self::view_configuration(&views.configuration)),
            ),
            (
                "containerViews",
                set_list(
                    &views.container_views,
                    |a, b| view_cmp(View::Container(a), View::Container(b)),
                    |v| self.container_view(v),
                )?,
            ),
            (
                "customViews",
                set_list(
                    &views.custom_views,
                    |a, b| view_cmp(View::Custom(a), View::Custom(b)),
                    |v| self.custom_view(v),
                )?,
            ),
            (
                "deploymentViews",
                set_list(
                    &views.deployment_views,
                    |a, b| view_cmp(View::Deployment(a), View::Deployment(b)),
                    |v| self.deployment_view(v),
                )?,
            ),
            (
                "dynamicViews",
                set_list(
                    &views.dynamic_views,
                    |a, b| view_cmp(View::Dynamic(a), View::Dynamic(b)),
                    |v| self.dynamic_view(v),
                )?,
            ),
            (
                "filteredViews",
                set_list(
                    &views.filtered_views,
                    |a, b| view_cmp(View::Filtered(a), View::Filtered(b)),
                    |v| self.filtered_view(v),
                )?,
            ),
            (
                "imageViews",
                set_list(
                    &views.image_views,
                    |a, b| view_cmp(View::Image(a), View::Image(b)),
                    |v| self.image_view(v),
                )?,
            ),
            (
                "systemContextViews",
                set_list(
                    &views.system_context_views,
                    |a, b| view_cmp(View::Context(a), View::Context(b)),
                    |v| self.context_view(v),
                )?,
            ),
            (
                "systemLandscapeViews",
                set_list(
                    &views.system_landscape_views,
                    |a, b| view_cmp(View::Landscape(a), View::Landscape(b)),
                    |v| self.landscape_view(v),
                )?,
            ),
        ]))
    }

    fn view_configuration(config: &ViewConfiguration) -> Json {
        Json::obj(vec![
            (
                "defaultView",
                Json::opt_text(config.default_view.as_deref()),
            ),
            (
                "lastSavedView",
                Json::opt_text(config.last_saved_view.as_deref()),
            ),
            (
                "metadataSymbols",
                config
                    .metadata_symbols
                    .map(|m| Json::Str(m.as_str().to_string())),
            ),
            ("properties", Json::props(&config.properties)),
            // The `styles` and `terminology` beans are never "empty" (§0).
            ("styles", Some(styles(&config.styles))),
            ("terminology", Some(terminology(&config.terminology))),
            // Insertion order (`List<String>`, §3/§5).
            (
                "themes",
                (!config.themes.is_empty()).then(|| Json::str_array(&config.themes)),
            ),
            (
                "viewSortOrder",
                config
                    .view_sort_order
                    .map(|o| Json::Str(o.as_str().to_string())),
            ),
        ])
    }

    /// The seven `View`-base entries every kind writes (§3: `name` computed,
    /// `order` primitive-always, `generatedKey` `NON_DEFAULT`).
    fn view_base(base: &ViewBase, name: &str) -> Vec<(&'static str, Option<Json>)> {
        vec![
            ("description", Json::opt_text(base.description.as_deref())),
            (
                "generatedKey",
                base.generated_key.then_some(Json::Bool(true)),
            ),
            ("key", Json::text(&base.key)),
            ("name", Json::text(name)),
            ("order", Some(Json::Num(i64::from(base.order)))),
            ("properties", Json::props(&base.properties)),
            ("title", Json::opt_text(base.title.as_deref())),
        ]
    }

    /// The `ModelView` content entries (§3), merged alphabetically.
    fn view_content(entries: &mut Vec<(&'static str, Option<Json>)>, content: &ViewContent) {
        insert_sorted(
            entries,
            (
                "automaticLayout",
                content.automatic_layout.map(automatic_layout),
            ),
        );
        insert_sorted(entries, ("dimensions", content.dimensions.map(dimensions)));
        insert_sorted(entries, ("elements", element_views(&content.elements)));
        insert_sorted(
            entries,
            (
                "paperSize",
                content
                    .paper_size
                    .map(|p| Json::Str(p.as_str().to_string())),
            ),
        );
        insert_sorted(
            entries,
            ("relationships", relationship_views(&content.relationships)),
        );
    }

    fn animations(steps: &[Animation]) -> Option<Json> {
        if steps.is_empty() {
            return None;
        }
        // Steps keep insertion order; id sets are TreeSet<String> UTF-16 (§5).
        Some(Json::Arr(
            steps
                .iter()
                .map(|a| {
                    Json::obj(vec![
                        ("elements", sorted_str_set(&a.element_ids)),
                        ("order", Some(Json::Num(i64::from(a.order)))),
                        ("relationships", sorted_str_set(&a.relationship_ids)),
                    ])
                })
                .collect(),
        ))
    }

    fn landscape_view(&self, v: &SystemLandscapeView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Landscape(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(
            &mut entries,
            (
                "enterpriseBoundaryVisible",
                Some(Json::Bool(v.enterprise_boundary_visible)),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn context_view(&self, v: &SystemContextView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Context(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(
            &mut entries,
            (
                "enterpriseBoundaryVisible",
                Some(Json::Bool(v.enterprise_boundary_visible)),
            ),
        );
        insert_sorted(
            &mut entries,
            (
                "softwareSystemId",
                Some(Json::Str(v.software_system_id.as_str().to_string())),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn container_view(&self, v: &ContainerView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Container(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(
            &mut entries,
            (
                "externalSoftwareSystemBoundariesVisible",
                Some(Json::Bool(v.external_software_system_boundaries_visible)),
            ),
        );
        insert_sorted(
            &mut entries,
            (
                "softwareSystemId",
                Some(Json::Str(v.software_system_id.as_str().to_string())),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn component_view(&self, v: &ComponentView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Component(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(
            &mut entries,
            (
                "containerId",
                Some(Json::Str(v.container_id.as_str().to_string())),
            ),
        );
        insert_sorted(
            &mut entries,
            (
                "externalContainerBoundariesVisible",
                Some(Json::Bool(v.external_container_boundaries_visible)),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn dynamic_view(&self, v: &DynamicView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Dynamic(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            (
                "elementId",
                v.element_id
                    .as_ref()
                    .map(|i| Json::Str(i.as_str().to_string())),
            ),
        );
        // `deploymentEnvironment` is invisible in JSON (§3) — never written.
        insert_sorted(
            &mut entries,
            (
                "externalBoundariesVisible",
                Some(Json::Bool(v.external_boundaries_visible)),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn deployment_view(&self, v: &DeploymentView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Deployment(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(&mut entries, ("environment", Json::text(&v.environment)));
        insert_sorted(
            &mut entries,
            (
                "softwareSystemId",
                v.software_system_id
                    .as_ref()
                    .map(|i| Json::Str(i.as_str().to_string())),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn custom_view(&self, v: &CustomView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Custom(v))?);
        Self::view_content(&mut entries, &v.content);
        insert_sorted(
            &mut entries,
            ("animations", Self::animations(&v.animations)),
        );
        insert_sorted(
            &mut entries,
            (
                "softwareSystemId",
                v.software_system_id
                    .as_ref()
                    .map(|i| Json::Str(i.as_str().to_string())),
            ),
        );
        Ok(Json::from_entries(&entries))
    }

    fn filtered_view(&self, v: &FilteredView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Filtered(v))?);
        insert_sorted(&mut entries, ("baseViewKey", Json::text(&v.base_view_key)));
        insert_sorted(
            &mut entries,
            ("mode", v.mode.map(|m| Json::Str(m.as_str().to_string()))),
        );
        insert_sorted(&mut entries, ("tags", sorted_str_set(&v.tags)));
        Ok(Json::from_entries(&entries))
    }

    fn image_view(&self, v: &ImageView) -> Result<Json, JsonError> {
        let mut entries = Self::view_base(&v.base, &self.view_name(View::Image(v))?);
        insert_sorted(
            &mut entries,
            ("content", Json::opt_text(v.content.as_deref())),
        );
        insert_sorted(
            &mut entries,
            ("contentDark", Json::opt_text(v.content_dark.as_deref())),
        );
        insert_sorted(
            &mut entries,
            ("contentLight", Json::opt_text(v.content_light.as_deref())),
        );
        insert_sorted(
            &mut entries,
            ("contentType", Json::opt_text(v.content_type.as_deref())),
        );
        insert_sorted(
            &mut entries,
            (
                "elementId",
                v.element_id
                    .as_ref()
                    .map(|i| Json::Str(i.as_str().to_string())),
            ),
        );
        Ok(Json::from_entries(&entries))
    }
}

fn automatic_layout(layout: AutomaticLayout) -> Json {
    // Presence enables layout; all five fields are primitives (always) (§3).
    Json::obj(vec![
        (
            "edgeSeparation",
            Some(Json::Num(i64::from(layout.edge_separation))),
        ),
        (
            "nodeSeparation",
            Some(Json::Num(i64::from(layout.node_separation))),
        ),
        (
            "rankDirection",
            layout
                .rank_direction
                .map(|d| Json::Str(d.as_str().to_string())),
        ),
        (
            "rankSeparation",
            Some(Json::Num(i64::from(layout.rank_separation))),
        ),
        ("vertices", Some(Json::Bool(layout.vertices))),
    ])
}

fn dimensions(d: Dimensions) -> Json {
    Json::obj(vec![
        ("height", Some(Json::Num(i64::from(d.height)))),
        ("width", Some(Json::Num(i64::from(d.width)))),
    ])
}

/// `elements[]`: numeric-id order (§5); `x`/`y` primitive-always (§3).
fn element_views(items: &[ElementView]) -> Option<Json> {
    set_list(
        items,
        |a, b| id_ord(&a.id, &b.id),
        |e| {
            Ok(Json::obj(vec![
                ("id", Some(Json::Str(e.id.as_str().to_string()))),
                ("x", Some(Json::Num(i64::from(e.x)))),
                ("y", Some(Json::Num(i64::from(e.y)))),
            ]))
        },
    )
    .unwrap_or(None) // infallible renderer
}

/// `relationships[]`: text `id + "/" + order` order; a null order
/// concatenates as the literal `"null"` (§5).
fn relationship_views(items: &[RelationshipView]) -> Option<Json> {
    set_list(
        items,
        |a, b| {
            java_ord(
                &format!("{}/{}", a.id, a.order.as_deref().unwrap_or("null")),
                &format!("{}/{}", b.id, b.order.as_deref().unwrap_or("null")),
            )
        },
        |r| Ok(relationship_view(r)),
    )
    .unwrap_or(None) // infallible renderer
}

fn relationship_view(r: &RelationshipView) -> Json {
    let mut entries = vec![
        ("description", Json::opt_text(r.description.as_deref())),
        ("id", Some(Json::Str(r.id.as_str().to_string()))),
        ("jump", r.jump.map(Json::Bool)),
        ("order", Json::opt_text(r.order.as_deref())),
        ("position", r.position.map(|p| Json::Num(i64::from(p)))),
        ("properties", Json::props(&r.properties)),
        // `response` is a NON_NULL Boolean — `false` IS written (V-2).
        ("response", r.response.map(Json::Bool)),
        (
            "routing",
            r.routing.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("url", Json::opt_text(r.url.as_deref())),
    ];
    if !r.vertices.is_empty() {
        insert_sorted(
            &mut entries,
            (
                "vertices",
                Some(Json::Arr(
                    r.vertices
                        .iter()
                        .map(|v| {
                            Json::obj(vec![
                                ("x", Some(Json::Num(i64::from(v.x)))),
                                ("y", Some(Json::Num(i64::from(v.y)))),
                            ])
                        })
                        .collect(),
                )),
            ),
        );
    }
    Json::obj(entries)
}

fn styles(styles: &Styles) -> Json {
    Json::obj(vec![
        (
            "elements",
            list(&styles.element_styles, |s| Ok(element_style(s))).unwrap_or(None),
        ),
        (
            "relationships",
            list(&styles.relationship_styles, |s| Ok(relationship_style(s))).unwrap_or(None),
        ),
    ])
}

/// Element-style fields are `@JsonInclude(NON_NULL)` — a SET value is
/// written even when `0`/`false`/`""` (V-2 note).
fn element_style(s: &ElementStyle) -> Json {
    Json::obj(vec![
        ("background", opt_nnn(s.background.as_deref())),
        (
            "border",
            s.border.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("color", opt_nnn(s.color.as_deref())),
        (
            "colorScheme",
            s.color_scheme.map(|c| Json::Str(c.as_str().to_string())),
        ),
        ("description", s.description.map(Json::Bool)),
        ("fontSize", s.font_size.map(|x| Json::Num(i64::from(x)))),
        ("height", s.height.map(|x| Json::Num(i64::from(x)))),
        ("icon", opt_nnn(s.icon.as_deref())),
        (
            "iconPosition",
            s.icon_position.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("metadata", s.metadata.map(Json::Bool)),
        ("opacity", s.opacity.map(|x| Json::Num(i64::from(x)))),
        ("properties", Json::props(&s.properties)),
        ("shape", s.shape.map(|x| Json::Str(x.as_str().to_string()))),
        ("stroke", opt_nnn(s.stroke.as_deref())),
        (
            "strokeWidth",
            s.stroke_width.map(|x| Json::Num(i64::from(x))),
        ),
        ("tag", Json::text(&s.tag)),
        ("width", s.width.map(|x| Json::Num(i64::from(x)))),
    ])
}

fn relationship_style(s: &RelationshipStyle) -> Json {
    Json::obj(vec![
        ("color", opt_nnn(s.color.as_deref())),
        (
            "colorScheme",
            s.color_scheme.map(|c| Json::Str(c.as_str().to_string())),
        ),
        ("dashed", s.dashed.map(Json::Bool)),
        ("description", s.description.map(Json::Bool)),
        ("fontSize", s.font_size.map(|x| Json::Num(i64::from(x)))),
        ("jump", s.jump.map(Json::Bool)),
        ("metadata", s.metadata.map(Json::Bool)),
        ("opacity", s.opacity.map(|x| Json::Num(i64::from(x)))),
        ("position", s.position.map(|x| Json::Num(i64::from(x)))),
        ("properties", Json::props(&s.properties)),
        (
            "routing",
            s.routing.map(|x| Json::Str(x.as_str().to_string())),
        ),
        ("style", s.style.map(|x| Json::Str(x.as_str().to_string()))),
        ("tag", Json::text(&s.tag)),
        ("thickness", s.thickness.map(|x| Json::Num(i64::from(x)))),
        ("width", s.width.map(|x| Json::Num(i64::from(x)))),
    ])
}

fn terminology(t: &Terminology) -> Json {
    // Alphabetical keys; ""-defaulted NON_EMPTY strings hide (§0).
    Json::obj(vec![
        ("code", Json::opt_text(t.code.as_deref())),
        ("component", Json::opt_text(t.component.as_deref())),
        ("container", Json::opt_text(t.container.as_deref())),
        (
            "deploymentNode",
            Json::opt_text(t.deployment_node.as_deref()),
        ),
        ("enterprise", Json::opt_text(t.enterprise.as_deref())),
        (
            "infrastructureNode",
            Json::opt_text(t.infrastructure_node.as_deref()),
        ),
        ("person", Json::opt_text(t.person.as_deref())),
        ("relationship", Json::opt_text(t.relationship.as_deref())),
        (
            "softwareSystem",
            Json::opt_text(t.software_system.as_deref()),
        ),
    ])
}
