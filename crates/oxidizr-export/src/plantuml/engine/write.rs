//! Write-side legs of the shared diagram engine: the per-kind `body_*`
//! dispatch, the element/relationship writers, the boundary walkers and the
//! dynamic-sequence document. The entry legs, the query legs and the free
//! text helpers stay in the parent module; the dialect hooks live in
//! [`super::Dialect`].

use oxidizr_core::views::{DynamicView, RelationshipView, View};

use crate::diagram::Diagram;
use crate::error::ExportError;
use crate::indent::IndentingWriter;
use crate::plantuml::java_cmp_str;
use crate::plantuml::model::{ElemRef, element_by_id};

use super::{Dialect, Engine, is_null_or_empty, is_true_ci, is_value, java_split_len};

impl<'a> Engine<'a> {
    /// The per-kind private `export(view, animationStep)` bodies —
    /// dispatch only (one `body_*` fn per view-kind overload,
    /// each citing its own; the
    /// `too_many_lines` allow this dispatch shape replaces is gone).
    pub(super) fn body<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
    ) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        d.write_header(self, view, &mut w)?;
        match view {
            View::Custom(_) | View::Landscape(_) | View::Context(_) => {
                self.body_flat(d, view, &mut w)?;
            }
            View::Container(_) => self.body_container(d, view, &mut w)?,
            View::Component(_) => self.body_component(d, view, &mut w)?,
            View::Dynamic(dv) => self.body_dynamic(d, view, dv, &mut w)?,
            View::Deployment(_) => self.body_deployment(d, view, &mut w)?,
            View::Filtered(_) | View::Image(_) => {
                return Err(ExportError::ReferenceAbort(
                    "filtered/image views are not exportable (Java throws)",
                ));
            }
        }
        d.write_footer(self, view, &mut w);
        Ok(d.create_diagram(self, view, w.to_string()))
    }

    /// `export(view)` for the Custom/Landscape/Context overloads: the
    /// grouped elements, a blank line iff any were written, relationships.
    fn body_flat<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements = self.get_groupable_elements(view, None)?;
        self.write_elements(d, view, &mut elements, w)?;
        if !elements.is_empty() {
            w.write_line(None);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Container overload: the non-container
    /// (system) elements, a blank line iff any, then one
    /// software-system boundary per boundary-system.
    fn body_container<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements_written = false;
        for element in self.view_elements(view) {
            if element.kind() != oxidizr_core::validate::ElementKind::Container {
                d.write_element(self, view, element, w)?;
                elements_written = true;
            }
        }
        if elements_written {
            w.write_line(None);
        }
        for system in self.get_boundary_systems(view) {
            d.start_software_system_boundary(self, view, system, w);
            let mut scoped = self.get_groupable_elements(view, Some(system.id()))?;
            self.write_elements(d, view, &mut scoped, w)?;
            d.end_software_system_boundary(self, view, w);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Component overload: the flat custom/people/
    /// systems legs, the boundary + container legs, relationships.
    fn body_component<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let customs = self.get_custom_elements(view);
        self.write_flat_leg(d, view, &customs, w)?;
        let people = self.get_people(view);
        self.write_flat_leg(d, view, &people, w)?;
        let systems = self.get_systems(view);
        self.write_flat_leg(d, view, &systems, w)?;
        self.write_boundary_container_legs(d, view, w)?;
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Dynamic overload: the elements scoped by the
    /// dynamic `elementId` (none → grouped; `SoftwareSystem` → boundaries
    /// then rootless; `Container` → the component legs then
    /// non-components), a blank line iff any were written, relationships.
    fn body_dynamic<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        dv: &'a DynamicView,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements_written = false;
        let scope = dv
            .element_id
            .as_ref()
            .and_then(|id| element_by_id(self.model(), id));
        match scope.map(ElemRef::kind) {
            None => {
                let mut elements = self.get_groupable_elements(view, None)?;
                self.write_elements(d, view, &mut elements, w)?;
                elements_written = !elements.is_empty();
            }
            Some(oxidizr_core::validate::ElementKind::SoftwareSystem) => {
                for system in self.get_boundary_systems(view) {
                    d.start_software_system_boundary(self, view, system, w);
                    let mut scoped = self.get_groupable_elements(view, Some(system.id()))?;
                    self.write_elements(d, view, &mut scoped, w)?;
                    d.end_software_system_boundary(self, view, w);
                }
                for element in self.view_elements(view) {
                    if element.parent_id().is_none() {
                        d.write_element(self, view, element, w)?;
                        elements_written = true;
                    }
                }
            }
            Some(oxidizr_core::validate::ElementKind::Container) => {
                let customs = self.get_custom_elements(view);
                self.write_flat_leg(d, view, &customs, w)?;
                let people = self.get_people(view);
                self.write_flat_leg(d, view, &people, w)?;
                let systems = self.get_systems(view);
                self.write_flat_leg(d, view, &systems, w)?;
                self.write_boundary_container_legs(d, view, w)?;
                for element in self.view_elements(view) {
                    // The official check: parent NOT a container — the
                    // kinds with a Container parent are components.
                    if element.kind() != oxidizr_core::validate::ElementKind::Component {
                        d.write_element(self, view, element, w)?;
                        elements_written = true;
                    }
                }
            }
            Some(_) => {}
        }
        if elements_written {
            w.write_line(None);
        }
        self.write_relationships(d, view, w)
    }

    /// `export(view)` for the Deployment overload: grouped elements then
    /// relationships — NO blank-line leg between them.
    fn body_deployment<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut elements = self.get_groupable_elements(view, None)?;
        self.write_elements(d, view, &mut elements, w)?;
        self.write_relationships(d, view, w)
    }

    /// A custom/people/systems leg: every element, then a blank line iff
    /// the list was non-empty (the `export(ComponentView)` pattern).
    fn write_flat_leg<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &[ElemRef<'a>],
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        for element in elements {
            d.write_element(self, view, *element, w)?;
        }
        if !elements.is_empty() {
            w.write_line(None);
        }
        Ok(())
    }

    /// The component-view boundary sequence (also the container-scoped
    /// dynamic leg): boundary containers id-sorted, their owners deduped
    /// in first-seen order; per owner the container boundaries with their
    /// scoped components, then the owner's view containers written flat.
    fn write_boundary_container_legs<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let boundary_containers = self.get_boundary_containers(view);
        let mut owners: Vec<&oxidizr_core::Id> = Vec::new();
        for container in &boundary_containers {
            let owner = container.owner_system_id();
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        let containers = self.get_containers(view);
        for owner in owners {
            let owner_ref = element_by_id(self.model(), owner)
                .ok_or(ExportError::ReferenceAbort("dangling container owner id"))?;
            d.start_software_system_boundary(self, view, owner_ref, w);
            for container in &boundary_containers {
                if container.owner_system_id() == owner {
                    d.start_container_boundary(self, view, *container, w);
                    let mut scoped = self.get_groupable_elements(view, Some(container.id()))?;
                    self.write_elements(d, view, &mut scoped, w)?;
                    d.end_container_boundary(self, view, w);
                }
            }
            for container in &containers {
                if container.owner_system_id() == owner {
                    d.write_element(self, view, *container, w)?;
                }
            }
            d.end_software_system_boundary(self, view, w);
        }
        Ok(())
    }

    /// `AbstractPlantUMLExporter.export(DynamicView)` sequence-diagram body
    /// (the export catalog): header, element set + boundary stack (or flat), blank
    /// line iff elements, relationships, footer, then the legend LAST.
    // Mirrors the single official method; the boundary-stack walk stays inline.
    /// the two inner blocks of this walk are the
    /// `sequence_write_order` / `write_sequence_boundaries` siblings;
    /// `export_dynamic_sequence` itself is cited by catalog name, so the
    /// fn and its name stay.
    pub(super) fn export_dynamic_sequence<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        dv: &'a DynamicView,
    ) -> Result<Diagram, ExportError> {
        let mut w = IndentingWriter::new();
        d.write_header(self, view, &mut w)?;

        // LinkedHashSet: source-then-destination per TreeSet relationship.
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        for rv in self.view_relationships(view) {
            let relationship = self.relationship(rv)?;
            for id in [&relationship.source, &relationship.destination] {
                let element = element_by_id(self.model(), id).ok_or(
                    ExportError::ReferenceAbort("dangling relationship endpoint"),
                )?;
                if !elements.iter().any(|e| e.id() == element.id()) {
                    elements.push(element);
                }
            }
        }

        let boundaries = is_true_ci(self.property(
            &dv.base,
            crate::plantuml::PLANTUML_BOUNDARIES_PROPERTY,
            "true",
        ));
        if boundaries {
            // the ordering and the box walk of this leg are
            // the `sequence_write_order` + `write_sequence_boundaries`
            // siblings (the write-order map walk, then
            // the boundary open/close stack).
            let order = self.sequence_write_order(&elements)?;
            self.write_sequence_boundaries(d, view, &elements, &order, &mut w)?;
        } else {
            for element in &elements {
                d.write_element(self, view, *element, &mut w)?;
            }
        }

        if !elements.is_empty() {
            w.write_line(None);
        }
        self.write_relationships(d, view, &mut w)?;
        d.write_footer(self, view, &mut w);
        let mut diagram = d.create_diagram(self, view, w.to_string());
        if let Some(legend) = d.create_legend(self, view) {
            diagram.set_legend(legend);
        }
        Ok(diagram)
    }

    /// The write-order map walk of the dynamic-sequence
    /// boundaries leg (split of `export_dynamic_sequence`):
    /// rootless elements first-seen; else the parent chain (root-last
    /// reversed) appended, then the element inserted after its last
    /// canonical-name "sibling" (the parent name appears after the element’s
    /// own `://` cut) or after its parent when there
    /// are none.
    fn sequence_write_order(
        &self,
        elements: &[ElemRef<'a>],
    ) -> Result<Vec<ElemRef<'a>>, ExportError> {
        let mut order: Vec<ElemRef<'a>> = Vec::new();
        for element in elements {
            let Some(parent_id) = element.parent_id().cloned() else {
                order.push(*element);
                continue;
            };
            let mut parents: Vec<ElemRef<'a>> = Vec::new();
            let mut current = element_by_id(self.model(), &parent_id)
                .ok_or(ExportError::ReferenceAbort("dangling parent id"))?;
            loop {
                parents.push(current);
                let Some(next) = current.parent_id().cloned() else {
                    break;
                };
                current = element_by_id(self.model(), &next)
                    .ok_or(ExportError::ReferenceAbort("dangling parent id"))?;
            }
            for p in parents.iter().rev() {
                if !order.iter().any(|e| e.id() == p.id()) {
                    order.push(*p);
                }
            }
            let needle = parents.first().map_or(String::new(), |p| {
                self.canonical_name(*p)
                    .split_once("://")
                    .map_or(String::new(), |(_, rest)| format!("://{rest}"))
            });
            let siblings: Vec<usize> = order
                .iter()
                .enumerate()
                .filter(|(_, e)| self.canonical_name(**e).contains(&needle))
                .map(|(i, _)| i)
                .collect();
            let index = if let Some(last) = siblings.last() {
                *last + 1
            } else {
                order
                    .iter()
                    .position(|e| e.id() == &parent_id)
                    .ok_or(ExportError::ReferenceAbort("dangling parent id"))?
                    + 1
            };
            order.insert(index, *element);
        }
        Ok(order)
    }

    /// The boundary open/close stack of the dynamic-sequence boundaries leg
    /// (split of `export_dynamic_sequence`): walk
    /// `sequence_write_order` — a view element inside the current box (or
    /// with no box open) writes directly, anything else unwinds the stack
    /// to the matching parent first; non-view elements open boundary boxes
    /// (`startBoundary`/`endBoundary`), drained at the end.
    fn write_sequence_boundaries<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &[ElemRef<'a>],
        order: &[ElemRef<'a>],
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let mut stack: Vec<ElemRef<'a>> = Vec::new();
        for element in order {
            let at_parent = |stack: &Vec<ElemRef<'a>>| {
                stack
                    .last()
                    .is_some_and(|top| element.parent_id() == Some(top.id()))
            };
            if elements.iter().any(|e| e.id() == element.id()) {
                if stack.is_empty() || at_parent(&stack) {
                    d.write_element(self, view, *element, w)?;
                } else {
                    while !stack.is_empty() && !at_parent(&stack) {
                        stack.pop();
                        d.end_boundary(self, view, w);
                    }
                    d.write_element(self, view, *element, w)?;
                }
            } else {
                while !stack.is_empty() && !at_parent(&stack) {
                    stack.pop();
                    d.end_boundary(self, view, w);
                }
                d.start_boundary(self, view, *element, w);
                stack.push(*element);
            }
        }
        while stack.pop().is_some() {
            d.end_boundary(self, view, w);
        }
        Ok(())
    }

    /// `writeElements` — the grouped pass (`findGroups` has already sorted
    /// `elements` IN PLACE by id string; the ungrouped pass sees that
    /// order), then the ungrouped elements.
    fn write_elements<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        elements: &mut Vec<ElemRef<'a>>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let groups = self.find_groups(elements);
        if !groups.is_empty() {
            let separator = self.group_separator();
            if is_value(&separator) {
                // Nested mode — the upstream context-tracking quirks are
                // mirrored EXACTLY (the export catalog).
                let mut context = String::new();
                for group in &groups {
                    let group_count = java_split_len(group, &separator);
                    let context_count = java_split_len(&context, &separator);
                    match group_count.cmp(&context_count) {
                        std::cmp::Ordering::Greater => w.indent(),
                        std::cmp::Ordering::Equal => {
                            // Upstream quirk: a ONE-character context never
                            // closes the previous group (UTF-16 length > 1).
                            if context.encode_utf16().count() > 1 {
                                d.end_group_boundary(self, view, w);
                            }
                        }
                        std::cmp::Ordering::Less => {
                            for _ in 0..(context_count - group_count) {
                                d.end_group_boundary(self, view, w);
                                w.outdent();
                            }
                            d.end_group_boundary(self, view, w);
                        }
                    }
                    d.start_group_boundary(self, view, group, w)?;
                    for element in elements.iter().copied() {
                        if element.group() == Some(group.as_str()) {
                            self.write(d, view, element, w)?;
                        }
                    }
                    context.clone_from(group);
                }
                let context_count = java_split_len(&context, &separator);
                for i in 0..context_count {
                    d.end_group_boundary(self, view, w);
                    if i < context_count - 1 {
                        w.outdent();
                    }
                }
            } else {
                for group in &groups {
                    d.start_group_boundary(self, view, group, w)?;
                    for element in elements.iter().copied() {
                        if element.group() == Some(group.as_str()) {
                            self.write(d, view, element, w)?;
                        }
                    }
                    d.end_group_boundary(self, view, w);
                }
            }
        }

        for element in elements.iter().copied() {
            if is_null_or_empty(element.group()) {
                self.write(d, view, element, w)?;
            }
        }
        Ok(())
    }

    /// The private `write`: deployment nodes recurse their boundary.
    fn write<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        element: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        if matches!(view, View::Deployment(_))
            && element.kind() == oxidizr_core::validate::ElementKind::DeploymentNode
        {
            self.write_deployment_node(d, view, element, w)?;
        } else {
            d.write_element(self, view, element, w)?;
        }
        Ok(())
    }

    /// The `writeDeploymentNode` recursion: children/infrastructure/
    /// instances each NAME-sorted (stable), filtered to view members,
    /// concatenated — `writeElements` then re-sorts the combined list by
    /// id string (and honors groups per node).
    fn write_deployment_node<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        node: ElemRef<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        d.start_deployment_node_boundary(self, view, node, w)?;
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        if let ElemRef::DeploymentNode { elem, .. } = node {
            let mut children: Vec<ElemRef<'a>> = elem
                .children
                .iter()
                .map(|c| ElemRef::DeploymentNode {
                    elem: c,
                    parent: Some(elem),
                })
                .collect();
            children.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                children
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut infra: Vec<ElemRef<'a>> = elem
                .infrastructure_nodes
                .iter()
                .map(|i| ElemRef::InfrastructureNode {
                    elem: i,
                    node: elem,
                })
                .collect();
            infra.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                infra
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut system_instances: Vec<ElemRef<'a>> = elem
                .software_system_instances
                .iter()
                .map(|si| ElemRef::SystemInstance {
                    elem: si,
                    node: elem,
                })
                .collect();
            system_instances.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                system_instances
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
            let mut container_instances: Vec<ElemRef<'a>> = elem
                .container_instances
                .iter()
                .map(|ci| ElemRef::ContainerInstance {
                    elem: ci,
                    node: elem,
                })
                .collect();
            container_instances.sort_by(|a, b| java_cmp_str(a.name(), b.name()));
            elements.extend(
                container_instances
                    .into_iter()
                    .filter(|e| self.is_element_in_view(view, e.id())),
            );
        }
        self.write_elements(d, view, &mut elements, w)?;
        d.end_deployment_node_boundary(self, view, w);
        Ok(())
    }

    /// `findGroups`: sorts `elements` in place by id STRING; collects the
    /// distinct groups (+ every nested ancestor prefix when a separator is
    /// set); returns them natural-string (UTF-16) sorted.
    fn find_groups(&self, elements: &mut [ElemRef<'a>]) -> Vec<String> {
        elements.sort_by(|a, b| java_cmp_str(a.id().as_str(), b.id().as_str()));
        let separator = self.group_separator();
        let nested = is_value(&separator);
        let mut groups: Vec<String> = Vec::new();
        let push = |groups: &mut Vec<String>, group: &str| {
            if !groups.iter().any(|g| g == group) {
                groups.push(group.to_string());
            }
        };
        for element in elements.iter() {
            let group = element.group().unwrap_or_default();
            if is_value(group) {
                push(&mut groups, group);
                if nested {
                    let mut prefix = group.to_string();
                    while prefix.contains(separator.as_str()) {
                        let cut = prefix.rfind(separator.as_str()).unwrap_or(0);
                        prefix.truncate(cut);
                        push(&mut groups, &prefix);
                    }
                }
            }
        }
        groups.sort_by(|a, b| java_cmp_str(a, b));
        groups
    }

    /// `Model.GROUP_SEPARATOR_PROPERTY_NAME` — the raw model property
    /// (NOT the view cascade).
    #[must_use]
    pub(crate) fn group_separator(&self) -> String {
        self.workspace
            .model
            .properties
            .get("structurizr.groupSeparator")
            .map(str::to_owned)
            .unwrap_or_default()
    }

    /// The relationships-in-view order: dynamic → `TreeSet` order; else the same
    /// set sorted by relationship-id string.
    fn get_relationships_in_view(&self, view: View<'a>) -> Vec<&'a RelationshipView> {
        let mut rels = self.view_relationships(view);
        if !matches!(view, View::Dynamic(_)) {
            rels.sort_by(|a, b| java_cmp_str(a.id.as_str(), b.id.as_str()));
        }
        rels
    }

    /// `writeRelationships` — ALL relationship views (never filtered by
    /// `isVisible`), + a trailing blank line iff non-empty.
    fn write_relationships<D: Dialect>(
        &mut self,
        d: &mut D,
        view: View<'a>,
        w: &mut IndentingWriter,
    ) -> Result<(), ExportError> {
        let rels = self.get_relationships_in_view(view);
        // the emptiness test is hoisted before the loop, so the
        // vec moves into the iterator — no `clone()` for a trailing
        // `is_empty()` (the official check runs on the same list).
        let non_empty = !rels.is_empty();
        for rv in rels {
            d.write_relationship(self, view, rv, w)?;
        }
        if non_empty {
            w.write_line(None);
        }
        Ok(())
    }

    /// The groupable-elements per-kind matrix, catalog-documented.
    /// The official cast-aborts
    /// are surfaced as [`ExportError::ReferenceAbort`].
    fn get_groupable_elements(
        &self,
        view: View<'a>,
        parent: Option<&oxidizr_core::Id>,
    ) -> Result<Vec<ElemRef<'a>>, ExportError> {
        use oxidizr_core::validate::ElementKind as K;
        let mut elements: Vec<ElemRef<'a>> = Vec::new();
        for ev in Self::view_element_views(view) {
            let element = element_by_id(self.model(), &ev.id)
                .ok_or(ExportError::ReferenceAbort("dangling view element id"))?;
            let k = element.kind();
            match view {
                View::Custom(_) => {
                    if k != K::CustomElement {
                        return Err(ExportError::ReferenceAbort(
                            "custom view contains a non-custom element (Java ClassCastException)",
                        ));
                    }
                    elements.push(element);
                }
                View::Landscape(_) | View::Context(_) => elements.push(element),
                View::Container(_) => {
                    if k == K::Container {
                        elements.push(element);
                    }
                }
                View::Component(_) => {
                    if k == K::Component {
                        elements.push(element);
                    }
                }
                View::Dynamic(dv) => {
                    let scope = dv
                        .element_id
                        .as_ref()
                        .and_then(|id| element_by_id(self.model(), id));
                    match scope.map(ElemRef::kind) {
                        None => {
                            if !(k == K::Person
                                || k == K::SoftwareSystem
                                || k == K::Container
                                || k == K::Component)
                            {
                                return Err(ExportError::ReferenceAbort(
                                    "dynamic view (no scope) contains a non-static-structure element (Java ClassCastException)",
                                ));
                            }
                            elements.push(element);
                        }
                        Some(K::SoftwareSystem) => {
                            if k == K::Container {
                                elements.push(element);
                            }
                        }
                        Some(K::Container) => {
                            if k == K::Component {
                                elements.push(element);
                            }
                        }
                        Some(_) => {}
                    }
                }
                View::Deployment(_) => {
                    if k == K::DeploymentNode && element.parent_id().is_none() {
                        elements.push(element);
                    }
                }
                View::Filtered(_) | View::Image(_) => {}
            }
        }
        if let Some(parent) = parent {
            elements.retain(|e| e.parent_id() == Some(parent));
        }
        Ok(elements)
    }
}
