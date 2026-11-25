//! Query-side legs of the shared diagram engine: the view element/
//! relationship orders, the kind filters, the visibility walk and the
//! official `typeOf`/`breakText`/`canonicalName` helpers. The write legs
//! live in `write.rs`, the entry legs and free text helpers in the parent.

use std::collections::HashMap;

use oxidizr_core::model::Relationship;
use oxidizr_core::views::{ElementView, MetadataSymbols, RelationshipView, Terminology, View};

use crate::error::ExportError;
use crate::plantuml::java_cmp_str;
use crate::plantuml::model::{ElemRef, element_by_id};

use super::{
    Engine, Frame, content_of, dedup_by_key, element_view_cmp, is_null_or_empty, is_value,
    relationship_view_key, utf16_len,
};

impl<'a> Engine<'a> {
    /// The view’s element views — `TreeSet` order, deduped by
    /// comparator key (first insertion wins, as in a `TreeSet`).
    #[must_use]
    pub fn view_elements(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        Self::view_element_views(view)
            .iter()
            .filter_map(|ev| element_by_id(self.model(), &ev.id))
            .collect()
    }

    /// The raw sorted element views (dangling ids kept — resolution
    /// happens per call site so a dangling id can abort where the official tooling would).
    pub(super) fn view_element_views(view: View<'a>) -> Vec<&'a ElementView> {
        let Some(content) = content_of(view) else {
            return Vec::new();
        };
        let mut evs: Vec<&'a ElementView> = content.elements.iter().collect();
        evs.sort_by(|a, b| element_view_cmp(a, b));
        dedup_by_key(&mut evs, |e| e.id.as_str());
        evs
    }

    /// The view’s relationship views — `TreeSet` order
    /// (`id + "/" + order`, a null order joining as the literal `null`),
    /// deduped by key (first insertion wins). Dynamic views apply the
    /// `DynamicView.getRelationships()` override: a stable re-sort by numeric
    /// `order` when EVERY order parses as a double, else by `order` text —
    /// stability preserves the `10/3, 11/3` tie (verified on `parallel1`),
    /// because the official `List.sort` (`TimSort`) and Rust's
    /// `sort_by` are both stable.
    #[must_use]
    pub fn view_relationships(&self, view: View<'a>) -> Vec<&'a RelationshipView> {
        let Some(content) = content_of(view) else {
            return Vec::new();
        };
        let mut rvs: Vec<&'a RelationshipView> = content.relationships.iter().collect();
        rvs.sort_by(|a, b| java_cmp_str(&relationship_view_key(a), &relationship_view_key(b)));
        dedup_by_key(&mut rvs, |r| relationship_view_key(r));
        if matches!(view, View::Dynamic(_)) {
            // `Double.parseDouble(null)` throws, so a missing order forces
            // the text branch — where the official tooling crashes comparing nulls;
            // oxidizr sorts a missing order first as the empty string
            // (never-crash policy, same as the JSON writer).
            let numeric = rvs.iter().all(|rv| {
                rv.order
                    .as_deref()
                    .is_some_and(|o| o.parse::<f64>().is_ok())
            });
            if numeric {
                rvs.sort_by(|a, b| {
                    let x = a.order.as_deref().unwrap_or_default().parse::<f64>();
                    let y = b.order.as_deref().unwrap_or_default().parse::<f64>();
                    x.unwrap_or(0.0).total_cmp(&y.unwrap_or(0.0))
                });
            } else {
                rvs.sort_by(|a, b| {
                    java_cmp_str(
                        a.order.as_deref().unwrap_or_default(),
                        b.order.as_deref().unwrap_or_default(),
                    )
                });
            }
        }
        rvs
    }

    /// `getBoundarySoftwareSystems` — the distinct owners of the view's
    /// containers, id-string sorted.
    pub(crate) fn get_boundary_systems(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        let mut systems: Vec<&oxidizr_core::Id> = Vec::new();
        for element in self.view_elements(view) {
            if let Some(system) = element.system() {
                let id = &system.item.id;
                if !systems.contains(&id) {
                    systems.push(id);
                }
            }
        }
        systems.sort_by(|a, b| java_cmp_str(a.as_str(), b.as_str()));
        systems
            .into_iter()
            .filter_map(|id| element_by_id(self.model(), id))
            .collect()
    }

    /// `getCustomElements` (distinct first-seen, id-string sorted).
    pub(super) fn get_custom_elements(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::CustomElement)
    }

    /// `getPeople`.
    pub(super) fn get_people(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::Person)
    }

    /// `getSoftwareSystems`.
    pub(super) fn get_systems(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::SoftwareSystem)
    }

    /// `getContainers` — the view's containers, id-string sorted.
    pub(super) fn get_containers(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        self.filter_kind_sorted(view, oxidizr_core::validate::ElementKind::Container)
    }

    /// `getBoundaryContainers` — the distinct containers owning the view's
    /// components, id-string sorted.
    pub(crate) fn get_boundary_containers(&self, view: View<'a>) -> Vec<ElemRef<'a>> {
        let mut out: Vec<ElemRef<'a>> = Vec::new();
        for element in self.view_elements(view) {
            if element.kind() == oxidizr_core::validate::ElementKind::Component
                && let Some(parent_id) = element.parent_id()
                && let Some(container) = element_by_id(self.model(), parent_id)
                && !out.iter().any(|c| c.id() == container.id())
            {
                out.push(container);
            }
        }
        out.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        out
    }

    fn filter_kind_sorted(
        &self,
        view: View<'a>,
        kind: oxidizr_core::validate::ElementKind,
    ) -> Vec<ElemRef<'a>> {
        let mut out: Vec<ElemRef<'a>> = Vec::new();
        for element in self.view_elements(view) {
            if element.kind() == kind && !out.iter().any(|e| e.id() == element.id()) {
                out.push(element);
            }
        }
        out.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        out
    }

    /// `view.isElementInView(element)` — membership by id.
    #[must_use]
    pub fn is_element_in_view(&self, view: View<'_>, id: &oxidizr_core::Id) -> bool {
        view.content()
            .is_some_and(|c| c.elements.iter().any(|e| &e.id == id))
    }

    /// The model relationship behind a relationship view (abort on a
    /// dangling id, where the official tooling would null-pointer crash).
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] when the id references a missing
    /// relationship.
    pub fn relationship(&self, rv: &RelationshipView) -> Result<&'a Relationship, ExportError> {
        // lazy `HashMap` — the old linear scan ran per
        // relationship view AND per `is_visible` leg (the official tooling reads
        // its object graph instead of re-scanning). First id wins, matching
        // the `find` semantics on a duplicate id.
        let index = self.rel_index.get_or_init(|| {
            let mut index = HashMap::with_capacity(self.model().relationships.len());
            for rel in &self.model().relationships {
                index.entry(rel.id.as_str()).or_insert(rel);
            }
            index
        });
        index
            .get(rv.id.as_str())
            .copied()
            .ok_or(ExportError::ReferenceAbort(
                "relationship view references a missing relationship",
            ))
    }

    /// `isVisible(view, element)` — animation-frame visibility ONLY (the
    /// JSON `ElementView.visible` flag is never consulted.
    ///
    /// # Errors
    ///
    /// [`ExportError::ReferenceAbort`] on a frame/view kind mismatch (the
    /// the untyped-`frame` cast failing).
    pub fn is_visible(&self, view: View<'_>, element: ElemRef<'_>) -> Result<bool, ExportError> {
        // borrow the frame — the old shape cloned the whole
        // Frame per element×frame call (the official tooling field read is a the official tooling).
        let Some(frame) = self.frame.as_ref() else {
            return Ok(true);
        };
        match frame {
            Frame::Step(step) => {
                let step = *step;
                match view {
                    View::Landscape(_)
                    | View::Context(_)
                    | View::Container(_)
                    | View::Component(_) => {
                        if step > 0 {
                            Ok(Self::revealed_by_step(view, step, element.id().as_str()))
                        } else {
                            Ok(true)
                        }
                    }
                    View::Deployment(v) => {
                        if step > 0 {
                            Ok(v.animations
                                .iter()
                                .filter(|a| a.order <= step)
                                .flat_map(|a| a.element_ids.iter())
                                .any(|id| id == element.id().as_str()))
                        } else {
                            Ok(true)
                        }
                    }
                    View::Custom(_) => Ok(true),
                    View::Dynamic(_) | View::Filtered(_) | View::Image(_) => {
                        Err(ExportError::ReferenceAbort(
                            "integer frame on a non-static view (Java ClassCastException)",
                        ))
                    }
                }
            }
            Frame::Order(order) => match view {
                View::Dynamic(dv) => {
                    let mut ids: Vec<&str> = Vec::new();
                    for rv in &dv.content.relationships {
                        if rv.order.as_deref() == Some(order.as_str()) {
                            let rel = self.relationship(rv)?;
                            ids.push(rel.source.as_str());
                            ids.push(rel.destination.as_str());
                        }
                    }
                    Ok(ids.contains(&element.id().as_str()))
                }
                _ => Err(ExportError::ReferenceAbort(
                    "string frame on a non-dynamic view (Java ClassCastException)",
                )),
            },
        }
    }

    fn revealed_by_step(view: View<'_>, step: i32, id: &str) -> bool {
        let animations = match view {
            View::Landscape(v) => &v.animations,
            View::Context(v) => &v.animations,
            View::Container(v) => &v.animations,
            View::Component(v) => &v.animations,
            _ => return false,
        };
        animations
            .iter()
            .filter(|a| a.order <= step)
            .flat_map(|a| a.element_ids.iter())
            .any(|e| e == id)
    }

    /// `AbstractExporter.typeOf` — the terminology + metadata-symbols type
    /// string: custom elements report their raw `metadata` (terminology
    /// never applies); instances get no branch and render empty; the
    /// `: <technology>` suffix is container/component/DN/IN only
    /// (the export catalog).
    #[must_use]
    pub fn type_of(&self, element: ElemRef<'a>, include_metadata_symbols: bool) -> String {
        use oxidizr_core::validate::ElementKind as K;
        let config = self.configuration();
        let terminology = |element: ElemRef<'_>| -> String {
            let t: &Terminology = &config.terminology;
            let (value, fallback): (&Option<String>, &str) = match element.kind() {
                K::Person => (&t.person, "Person"),
                K::SoftwareSystem => (&t.software_system, "Software System"),
                K::Container => (&t.container, "Container"),
                K::Component => (&t.component, "Component"),
                K::DeploymentNode => (&t.deployment_node, "Deployment Node"),
                K::InfrastructureNode => (&t.infrastructure_node, "Infrastructure Node"),
                _ => return String::new(),
            };
            value
                .as_deref()
                .filter(|s| is_value(s))
                .unwrap_or(fallback)
                .to_string()
        };
        let type_ = match element.kind() {
            K::Person | K::SoftwareSystem => terminology(element),
            K::Container | K::Component | K::DeploymentNode | K::InfrastructureNode => {
                let tech = element.technology().filter(|t| is_value(t));
                match tech {
                    Some(t) => format!("{}: {t}", terminology(element)),
                    None => terminology(element),
                }
            }
            K::CustomElement => element.metadata().unwrap_or("").to_string(),
            K::SoftwareSystemInstance | K::ContainerInstance => String::new(),
        };
        if is_null_or_empty(Some(&type_)) {
            return type_;
        }
        if include_metadata_symbols {
            // A NULL metadataSymbols setting is MUTATED to SquareBrackets
            // in the official tooling; here the default read is equivalent.
            match config
                .metadata_symbols
                .unwrap_or(MetadataSymbols::SquareBrackets)
            {
                MetadataSymbols::RoundBrackets => format!("({type_})"),
                MetadataSymbols::CurlyBrackets => format!("{{{type_}}}"),
                MetadataSymbols::AngleBrackets => format!("<{type_}>"),
                MetadataSymbols::DoubleAngleBrackets => format!("<<{type_}>>"),
                MetadataSymbols::None => type_,
                MetadataSymbols::SquareBrackets => format!("[{type_}]"),
            }
        } else {
            type_
        }
    }

    /// `AbstractExporter.breakText`: `maxCharacters = (int)(maxWidth /
    /// (fontSize x 0.6))`, unchanged when `length < maxCharacters` (UTF-16),
    /// word-wrap with `<br />` joins; null/trim-empty → `""`.
    // The f64→i64 conversion below mirrors the official integer truncation.
    #[allow(clippy::cast_possible_truncation)]
    #[must_use]
    pub fn break_text(&self, max_width: i32, font_size: i32, text: &str) -> String {
        if is_null_or_empty(Some(text)) {
            return String::new();
        }
        let character_width = f64::from(font_size) * 0.6;
        let max_characters = (f64::from(max_width) / character_width) as i64; // Java (int) truncates
        if utf16_len(text) < max_characters {
            return text.to_string();
        }
        // The official space split: single-space separator, trailing empties dropped.
        let mut words: Vec<&str> = text.split(' ').collect();
        while words.last() == Some(&"") {
            words.pop();
        }
        let mut buf = String::new();
        let mut line: Option<String> = None;
        for word in words {
            match line.take() {
                None => line = Some(word.to_string()),
                Some(current) => {
                    if utf16_len(&current) + utf16_len(word) + 1 < max_characters {
                        line = Some(format!("{current} {word}"));
                    } else {
                        buf.push_str(&current);
                        buf.push_str("<br />");
                        line = Some(word.to_string());
                    }
                }
            }
        }
        if let Some(line) = line {
            buf.push_str(&line);
        }
        buf
    }

    /// `Element.getCanonicalName` (`model/CanonicalNameGenerator`):
    /// `<Kind>://<name path>` with `.` and `/` stripped from every name;
    /// deployment chains are `/`-joined, static chains `.`-joined
    /// (the sibling matching).
    ///
    /// memoized per engine (id → name). The workspace is
    /// immutable for `'a`, so the name is a pure function of the id; the
    /// sequence legs and the sibling matching re-probe the same elements
    /// and the deployment leg recursed per ancestor without sharing work.
    #[must_use]
    pub fn canonical_name(&self, element: ElemRef<'a>) -> String {
        let id = element.id().as_str();
        if let Some(name) = self.name_index.borrow().get(id) {
            return name.clone();
        }
        let name = self.canonical_name_uncached(element);
        // The borrow ends before the insert's own statement — `node_tail`
        // recursion re-enters this wrapper, never holds the borrow.
        self.name_index.borrow_mut().insert(id, name.clone());
        name
    }

    fn canonical_name_uncached(&self, element: ElemRef<'a>) -> String {
        use oxidizr_core::validate::ElementKind as K;
        let format_name = |name: &str| name.replace(['.', '/'], "");
        let by_id =
            |id: Option<&'a oxidizr_core::Id>| id.and_then(|id| element_by_id(self.model(), id));
        let node_tail = |node: ElemRef<'a>| -> String {
            self.canonical_name(node)
                .strip_prefix("DeploymentNode://")
                .unwrap_or_default()
                .to_string()
        };
        match element.kind() {
            K::CustomElement => format!("Custom://{}", format_name(element.name())),
            K::Person => format!("Person://{}", format_name(element.name())),
            K::SoftwareSystem => format!("SoftwareSystem://{}", format_name(element.name())),
            K::Container => {
                let system = element.system().map_or("", |s| s.item.name.as_str());
                format!(
                    "Container://{}.{}",
                    format_name(system),
                    format_name(element.name())
                )
            }
            K::Component => {
                let container = by_id(element.parent_id());
                let (system_name, container_name) =
                    container.map_or((String::new(), element.name().to_string()), |c| {
                        let s = c.system().map_or("", |s| s.item.name.as_str());
                        (s.to_string(), c.name().to_string())
                    });
                format!(
                    "Component://{}.{}.{}",
                    format_name(&system_name),
                    format_name(&container_name),
                    format_name(element.name())
                )
            }
            K::DeploymentNode => {
                let mut chain: Vec<String> = Vec::new();
                let mut current = element;
                while let Some(parent) = by_id(current.parent_id()) {
                    chain.push(format_name(parent.name()));
                    current = parent;
                }
                chain.reverse();
                let env = element.environment().unwrap_or("");
                // one `join` instead of the per-name push loop.
                // The trailing slash belongs to the chain, so an EMPTY chain
                // yields `env/<name>` (no `env//<name>`) — the byte shape of
                // the old prefix-then-append loop, kept exactly.
                let names = if chain.is_empty() {
                    String::new()
                } else {
                    format!("{}/", chain.join("/"))
                };
                format!(
                    "DeploymentNode://{}/{names}{}",
                    format_name(env),
                    format_name(element.name())
                )
            }
            K::InfrastructureNode => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                format!(
                    "InfrastructureNode://{}/{}",
                    tail,
                    format_name(element.name())
                )
            }
            K::SoftwareSystemInstance => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                let referent = element.referent(self.model());
                format!(
                    "SoftwareSystemInstance://{}/{}[{}]",
                    tail,
                    format_name(referent.name()),
                    element.instance_id().unwrap_or(0)
                )
            }
            K::ContainerInstance => {
                let tail = by_id(element.parent_id()).map_or(String::new(), node_tail);
                let referent = element.referent(self.model());
                let system_name = by_id(referent.parent_id())
                    .and_then(|c| c.system().map(|s| s.item.name.clone()))
                    .unwrap_or_default();
                format!(
                    "ContainerInstance://{}/{}.{}[{}]",
                    tail,
                    format_name(&system_name),
                    format_name(referent.name()),
                    element.instance_id().unwrap_or(0)
                )
            }
        }
    }
}
