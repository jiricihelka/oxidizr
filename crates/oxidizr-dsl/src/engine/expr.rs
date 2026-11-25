//! Filter-expression evaluation — the expressions used by
//! `include`/`exclude`, the `!element(s)`/`!relationship(s)` finders and
//! custom views.
//!
//! The official tooling collects results into hash sets. Its iteration
//! order cannot be reproduced deterministically, so every set here is an
//! insertion-ordered deduping `Vec` fed from ascending-numeric-id model
//! traversals — the closest portable equivalent.
//!
//! The `&&`/`||` handling is reproduced exactly, quirks included: only
//! the first two split parts are ever used (later clauses of a three-part
//! expression are silently IGNORED), and the split drops trailing empty
//! parts.

use oxidizr_core::Id;

use super::Parser;
use super::idents::RegItem;
use super::{DslError, java_index_or_oob, java_split_str, split_keeping_lead_dropping_trail};

/// Expression prefixes — verbatim grammar constants.
const ELEMENT_EQUALS: &str = "element==";
const ELEMENT_NOT_EQUALS: &str = "element!=";
const ELEMENT_TYPE_EQUALS: &str = "element.type==";
const ELEMENT_TAG_EQUALS: &str = "element.tag==";
const ELEMENT_TAG_NOT_EQUALS: &str = "element.tag!=";
const ELEMENT_TECHNOLOGY_EQUALS: &str = "element.technology==";
const ELEMENT_TECHNOLOGY_NOT_EQUALS: &str = "element.technology!=";
const ELEMENT_GROUP_EQUALS: &str = "element.group==";
const ELEMENT_PARENT_EQUALS: &str = "element.parent==";
const RELATIONSHIP: &str = "->";
const RELATIONSHIP_EQUALS: &str = "relationship==";
const RELATIONSHIP_TAG_EQUALS: &str = "relationship.tag==";
const RELATIONSHIP_TAG_NOT_EQUALS: &str = "relationship.tag!=";
const RELATIONSHIP_SOURCE_EQUALS: &str = "relationship.source==";
const RELATIONSHIP_DESTINATION_EQUALS: &str = "relationship.destination==";
const WILDCARD: &str = "*";

/// A model item: element or relationship. `Ord` is numeric-id order
/// so set complements are deterministic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mi {
    /// A model element.
    Element(Id),
    /// A model relationship.
    Rel(Id),
}

/// An ordered (insertion-order, deduped) model-item set.
pub(crate) type Items = Vec<Mi>;

/// Which evaluator dialect handles an expression. The view kind picks
/// the element-type whitelist, the deployment instance expansion and the
/// coupling class passes; `Model` is the base dialect used by the finders
/// and the model scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExprScope {
    /// Base dialect (`!elements`, `!relationships`, model).
    Model,
    /// Static-view dialect (landscape/context/container/component).
    Static,
    /// Deployment-view dialect.
    Deployment,
    /// Custom-view dialect.
    Custom,
}

/// Set union: add every item, deduping, keeping insertion order.
pub(crate) fn union(into: &mut Items, more: Items) {
    for item in more {
        if !into.contains(&item) {
            into.push(item);
        }
    }
}

/// Set intersection: keep only the shared items.
pub(crate) fn intersect(set: &mut Items, other: &Items) {
    set.retain(|item| other.contains(item));
}

/// Whole-string match for an element property expression:
/// a lowercase literal prefix, a `]` and a following `==`.
fn matches_element_property(expr: &str) -> bool {
    expr.starts_with("element.properties[") && expr.contains("]==")
}

/// Whole-string match for a relationship property expression.
fn matches_relationship_property(expr: &str) -> bool {
    expr.starts_with("relationship.properties[") && expr.contains("]==")
}

/// Whether a token is an expression (rather than a bare identifier).
pub(crate) fn is_expression(token: &str) -> bool {
    let token = token.to_lowercase();
    token.starts_with(ELEMENT_EQUALS)
        || token.starts_with(ELEMENT_NOT_EQUALS)
        || token.starts_with(ELEMENT_TYPE_EQUALS)
        || token.starts_with(ELEMENT_TAG_EQUALS)
        || token.starts_with(ELEMENT_TAG_NOT_EQUALS)
        || token.starts_with(ELEMENT_TECHNOLOGY_EQUALS)
        || token.starts_with(ELEMENT_TECHNOLOGY_NOT_EQUALS)
        || matches_element_property(&token)
        || token.starts_with(ELEMENT_GROUP_EQUALS)
        || token.starts_with(ELEMENT_PARENT_EQUALS)
        || token.contains(RELATIONSHIP)
        || token.starts_with(RELATIONSHIP_TAG_EQUALS)
        || token.starts_with(RELATIONSHIP_TAG_NOT_EQUALS)
        || matches_relationship_property(&token)
        || token.starts_with(RELATIONSHIP_SOURCE_EQUALS)
        || token.starts_with(RELATIONSHIP_DESTINATION_EQUALS)
        || token.starts_with(RELATIONSHIP_EQUALS)
}

impl Parser<'_> {
    /// Expression entry point: split on ` && `/` || ` first, then evaluate.
    pub(crate) fn parse_expression(
        &mut self,
        expr: &str,
        scope: ExprScope,
    ) -> Result<Items, DslError> {
        if expr.contains(" && ") {
            let parts = java_split_str(expr, " && ");
            let first = java_index_or_oob(&parts, 0).map_err(DslError::stmt)?;
            let second = java_index_or_oob(&parts, 1).map_err(DslError::stmt)?;
            let mut items = self.evaluate(first, scope)?;
            let other = self.evaluate(second, scope)?;
            intersect(&mut items, &other);
            Ok(items)
        } else if expr.contains(" || ") {
            let parts = java_split_str(expr, " || ");
            let first = java_index_or_oob(&parts, 0).map_err(DslError::stmt)?;
            let second = java_index_or_oob(&parts, 1).map_err(DslError::stmt)?;
            let mut items = self.evaluate(first, scope)?;
            let other = self.evaluate(second, scope)?;
            union(&mut items, other);
            Ok(items)
        } else {
            self.evaluate(expr, scope)
        }
    }

    /// Evaluate a single expression — the arm
    /// order IS the compatibility contract (verbatim if-else chain).
    #[allow(clippy::too_many_lines)] // the chain arm order IS the contract
    fn evaluate(&mut self, expr: &str, scope: ExprScope) -> Result<Items, DslError> {
        let mut items: Items = Vec::new();

        if let Some(inner) = expr.strip_prefix(ELEMENT_EQUALS) {
            if is_expression(inner) {
                union(&mut items, self.evaluate(inner, scope)?);
            } else {
                union(&mut items, self.parse_identifier(inner, scope)?);
            }
        } else if let Some(inner) = expr.strip_prefix(ELEMENT_NOT_EQUALS) {
            let excluded = if is_expression(inner) {
                self.evaluate(inner, scope)?
            } else {
                self.parse_identifier(inner, scope)?
            };
            // all elements EXCEPT the excluded set (name order)
            for id in self.builder.all_element_ids() {
                let item = Mi::Element(id);
                if !excluded.contains(&item) {
                    items.push(item);
                }
            }
        } else if expr.starts_with(RELATIONSHIP_EQUALS) {
            let mut inner = expr
                .strip_prefix(RELATIONSHIP_EQUALS)
                .unwrap_or_default()
                .to_string();
            if inner == WILDCARD {
                inner = format!("{WILDCARD}{RELATIONSHIP}{WILDCARD}");
            }
            if is_expression(&inner) {
                union(&mut items, self.evaluate(&inner, scope)?);
            } else {
                union(&mut items, self.parse_identifier(&inner, scope)?);
            }
        } else if expr == RELATIONSHIP {
            return Err(self.stmt_err(format!("Unexpected identifier \"{expr}\"")));
        } else if expr.starts_with(RELATIONSHIP) || expr.ends_with(RELATIONSHIP) {
            // an element expression with coupling expansion:
            // ->identifier identifier-> ->identifier->
            let mut include_incoming = false;
            let mut include_outgoing = false;
            let mut identifier = expr;
            if identifier.starts_with(RELATIONSHIP) {
                include_incoming = true;
                identifier = identifier.strip_prefix(RELATIONSHIP).unwrap_or_default();
            }
            if identifier.ends_with(RELATIONSHIP) {
                include_outgoing = true;
                identifier = &identifier[..identifier.len() - RELATIONSHIP.len()];
            }
            let identifier = identifier.trim();

            let elements = if is_expression(identifier) {
                self.parse_expression(identifier, scope)?
                    .into_iter()
                    .filter_map(|mi| match mi {
                        Mi::Element(id) => Some(id),
                        Mi::Rel(_) => None,
                    })
                    .collect()
            } else {
                self.get_elements(identifier, scope)
            };
            if elements.is_empty() {
                return Err(self.stmt_err(format!("The element \"{identifier}\" does not exist")));
            }
            for element in elements {
                items.push(Mi::Element(element.clone()));
                if include_incoming {
                    union(&mut items, self.find_afferent_couplings(&element, scope));
                }
                if include_outgoing {
                    union(&mut items, self.find_efferent_couplings(&element, scope));
                }
            }
        } else if expr.contains(RELATIONSHIP) {
            // source->destination identifier pair
            let parts = java_split_str(expr, RELATIONSHIP);
            let source_identifier = parts.first().map_or("", |p| p.trim());
            // The official code indexes part 1 unconditionally; that
            // is unreachable here (the guard above matched a real `->`, and
            // an all-empty split makes the coupling arm error first).
            let destination_identifier = parts.get(1).map_or("", |p| p.trim());
            let source_expr = format!("{RELATIONSHIP_SOURCE_EQUALS}{source_identifier}");
            let destination_expr =
                format!("{RELATIONSHIP_DESTINATION_EQUALS}{destination_identifier}");

            if source_identifier == WILDCARD && destination_identifier == WILDCARD {
                items.extend(self.builder.all_relationship_ids().into_iter().map(Mi::Rel));
            } else if destination_identifier == WILDCARD {
                union(&mut items, self.parse_expression(&source_expr, scope)?);
            } else if source_identifier == WILDCARD {
                union(&mut items, self.parse_expression(&destination_expr, scope)?);
            } else {
                union(
                    &mut items,
                    self.parse_expression(&format!("{source_expr} && {destination_expr}"), scope)?,
                );
            }
        } else if expr.to_lowercase().starts_with(ELEMENT_PARENT_EQUALS) {
            let parent_identifier = expr.strip_prefix(ELEMENT_PARENT_EQUALS).unwrap_or_default();
            let Some(parent) = self.get_element(parent_identifier) else {
                return Err(self.stmt_err(format!(
                    "The parent element \"{parent_identifier}\" does not exist"
                )));
            };
            if let RegItem::Element(parent_id) = parent {
                for id in self.builder.all_element_ids() {
                    if self.builder.element_parent(&id).as_ref() == Some(&parent_id) {
                        items.push(Mi::Element(id));
                    }
                }
            }
        } else if expr.starts_with(ELEMENT_TYPE_EQUALS) {
            // exact-case prefix (the match lowercases but the
            // remainder is cut with the exact constant's length)
            items.extend(
                self.evaluate_element_type_expression(expr, scope)?
                    .into_iter()
                    .map(Mi::Element),
            );
        } else if expr.to_lowercase().starts_with(ELEMENT_TAG_EQUALS) {
            let tags = expr
                .strip_prefix(ELEMENT_TAG_EQUALS)
                .unwrap_or_default()
                .to_string();
            for id in self.builder.all_element_ids() {
                if self.has_all_tags(&Mi::Element(id.clone()), &tags) {
                    items.push(Mi::Element(id));
                }
            }
        } else if expr.to_lowercase().starts_with(ELEMENT_TAG_NOT_EQUALS) {
            let tags = expr
                .strip_prefix(ELEMENT_TAG_NOT_EQUALS)
                .unwrap_or_default()
                .to_string();
            for id in self.builder.all_element_ids() {
                if !self.has_all_tags(&Mi::Element(id.clone()), &tags) {
                    items.push(Mi::Element(id));
                }
            }
        } else if expr.to_lowercase().starts_with(ELEMENT_TECHNOLOGY_EQUALS) {
            let technology = expr
                .strip_prefix(ELEMENT_TECHNOLOGY_EQUALS)
                .unwrap_or_default();
            items.extend(self.elements_by_technology(technology, false));
        } else if expr
            .to_lowercase()
            .starts_with(ELEMENT_TECHNOLOGY_NOT_EQUALS)
        {
            let technology = expr
                .strip_prefix(ELEMENT_TECHNOLOGY_NOT_EQUALS)
                .unwrap_or_default();
            items.extend(self.elements_by_technology(technology, true));
        } else if matches_element_property(expr) {
            let (name, value) = property_pair(expr);
            for id in self.builder.all_element_ids() {
                if self.has_property(&Mi::Element(id.clone()), &name, &value) {
                    items.push(Mi::Element(id));
                }
            }
        } else if expr.to_lowercase().starts_with(ELEMENT_GROUP_EQUALS) {
            // the group name is cut at `==`, not at the constant
            let group_name = &expr[expr.find("==").unwrap_or(0) + 2..];
            for id in self.builder.all_element_ids() {
                if self.builder.element_group(&id).as_deref() == Some(group_name) {
                    items.push(Mi::Element(id));
                }
            }
        } else if expr.starts_with(RELATIONSHIP_TAG_EQUALS) {
            let tags = expr
                .strip_prefix(RELATIONSHIP_TAG_EQUALS)
                .unwrap_or_default()
                .to_string();
            for id in self.builder.all_relationship_ids() {
                if self.has_all_tags(&Mi::Rel(id.clone()), &tags) {
                    items.push(Mi::Rel(id));
                }
            }
        } else if expr.starts_with(RELATIONSHIP_TAG_NOT_EQUALS) {
            let tags = expr
                .strip_prefix(RELATIONSHIP_TAG_NOT_EQUALS)
                .unwrap_or_default()
                .to_string();
            for id in self.builder.all_relationship_ids() {
                if !self.has_all_tags(&Mi::Rel(id.clone()), &tags) {
                    items.push(Mi::Rel(id));
                }
            }
        } else if matches_relationship_property(expr) {
            let (name, value) = property_pair(expr);
            for id in self.builder.all_relationship_ids() {
                if self.has_property(&Mi::Rel(id.clone()), &name, &value) {
                    items.push(Mi::Rel(id));
                }
            }
        } else if let Some(identifier) = expr.strip_prefix(RELATIONSHIP_SOURCE_EQUALS) {
            let sources = self.resolve_endpoint(identifier, scope)?;
            for id in self.builder.all_relationship_ids() {
                if let Some(source) = self.rel_source(&id)
                    && sources.contains(&source)
                {
                    items.push(Mi::Rel(id));
                }
            }
        } else if let Some(identifier) = expr.strip_prefix(RELATIONSHIP_DESTINATION_EQUALS) {
            let destinations = self.resolve_endpoint(identifier, scope)?;
            for id in self.builder.all_relationship_ids() {
                let destination = self.rel_destination(&id);
                if destination.is_some_and(|d| destinations.contains(&d)) {
                    items.push(Mi::Rel(id));
                }
            }
        } else {
            // fallback: the expression is an identifier (EMPTY is OK here)
            items.extend(self.get_elements(expr, scope).into_iter().map(Mi::Element));
        }

        Ok(items)
    }

    /// `element.type==` — the base dialect handles
    /// all nine kinds (unknown → empty, no throw); the view dialects
    /// restrict the whitelist and throw for anything else, quoting the RAW
    /// (case-preserved) type token.
    fn evaluate_element_type_expression(
        &mut self,
        expr: &str,
        scope: ExprScope,
    ) -> Result<Vec<Id>, DslError> {
        use oxidizr_core::validate::ElementKind as K;
        let raw_type = expr
            .strip_prefix(ELEMENT_TYPE_EQUALS)
            .unwrap_or_default()
            .to_string();
        let type_token = raw_type.to_lowercase();
        let kind = match type_token.as_str() {
            "custom" => Some(K::CustomElement),
            "person" => Some(K::Person),
            "softwaresystem" => Some(K::SoftwareSystem),
            "container" => Some(K::Container),
            "component" => Some(K::Component),
            "deploymentnode" => Some(K::DeploymentNode),
            "infrastructurenode" => Some(K::InfrastructureNode),
            "softwaresysteminstance" => Some(K::SoftwareSystemInstance),
            "containerinstance" => Some(K::ContainerInstance),
            _ => None,
        };
        let allowed: &[K] = match scope {
            ExprScope::Model => &[
                K::CustomElement,
                K::Person,
                K::SoftwareSystem,
                K::Container,
                K::Component,
                K::DeploymentNode,
                K::InfrastructureNode,
                K::SoftwareSystemInstance,
                K::ContainerInstance,
            ],
            ExprScope::Static => &[
                K::CustomElement,
                K::Person,
                K::SoftwareSystem,
                K::Container,
                K::Component,
            ],
            ExprScope::Deployment => &[
                K::CustomElement,
                K::DeploymentNode,
                K::InfrastructureNode,
                K::SoftwareSystem,
                K::SoftwareSystemInstance,
                K::Container,
                K::ContainerInstance,
            ],
            ExprScope::Custom => &[K::CustomElement],
        };
        let Some(kind) = kind.filter(|k| allowed.contains(k)) else {
            if scope == ExprScope::Model {
                // the base dialect's catch-all adds nothing (no throw)
                return Ok(Vec::new());
            }
            return Err(self.stmt_err(format!(
                "The element type of \"{raw_type}\" is not valid for this view"
            )));
        };
        Ok(self
            .builder
            .all_element_ids()
            .into_iter()
            .filter(|id| self.builder.element_kind(id) == Some(kind))
            .collect())
    }

    /// The technology filters: containers, then components, deployment
    /// nodes, infrastructure nodes and container instances — in that
    /// output order; `negate` is `element.technology!=`.
    fn elements_by_technology(&self, technology: &str, negate: bool) -> Items {
        use oxidizr_core::validate::ElementKind as K;
        let mut out = Items::new();
        for want in [
            K::Container,
            K::Component,
            K::DeploymentNode,
            K::InfrastructureNode,
            K::ContainerInstance,
        ] {
            for id in self.builder.all_element_ids() {
                if self.builder.element_kind(&id) != Some(want) {
                    continue;
                }
                let has = self.builder.element_technology(&id) == technology;
                if has != negate {
                    out.push(Mi::Element(id));
                }
            }
        }
        out
    }

    /// Whether an item carries ALL the comma-separated tags — tag checks
    /// fall through to the static element of an instance or the linked
    /// relationship.
    fn has_all_tags(&self, item: &Mi, tags: &str) -> bool {
        // Comma split: trailing empty parts are dropped, and an empty tag
        // list yields one (empty) tag.
        for tag in split_keeping_lead_dropping_trail(tags) {
            let tag = tag.trim();
            let mut has = match item {
                Mi::Element(id) | Mi::Rel(id) => self.builder.item_has_tag(id, tag),
            };
            if !has {
                match item {
                    Mi::Element(id) => {
                        if let Some((static_id, _env, _groups)) = self.builder.instance_static(id) {
                            has = self.builder.item_has_tag(&static_id, tag);
                        }
                    }
                    Mi::Rel(id) => {
                        if let Some(linked) = self.builder.linked_relationship_id(id)
                            && self.builder.has_relationship(&linked)
                        {
                            has = self.builder.item_has_tag(&linked, tag);
                        }
                    }
                }
            }
            if !has {
                return false;
            }
        }
        true
    }

    /// Whether an item carries the property — same fall-through.
    fn has_property(&self, item: &Mi, name: &str, value: &str) -> bool {
        let mut has = match item {
            Mi::Element(id) | Mi::Rel(id) => self.builder.item_has_property(id, name, value),
        };
        if !has {
            match item {
                Mi::Element(id) => {
                    if let Some((static_id, _env, _groups)) = self.builder.instance_static(id) {
                        has = self.builder.item_has_property(&static_id, name, value);
                    }
                }
                Mi::Rel(id) => {
                    if let Some(linked) = self.builder.linked_relationship_id(id)
                        && self.builder.has_relationship(&linked)
                    {
                        has = self.builder.item_has_property(&linked, name, value);
                    }
                }
            }
        }
        has
    }

    /// Coupling expansion AT the element — sources pointing at it, in
    /// numeric-id relationship order. The view dialects make ONE PASS PER
    /// ELEMENT CLASS in their declared class order (class-grouped output),
    /// keeping only that class's matches; the base dialect is a single
    /// unfiltered pass.
    fn find_afferent_couplings(&self, element: &Id, scope: ExprScope) -> Items {
        use oxidizr_core::validate::ElementKind as K;
        let passes: &[&[K]] = match scope {
            ExprScope::Model => &[&[]],
            ExprScope::Static => &[&[
                K::CustomElement,
                K::Person,
                K::SoftwareSystem,
                K::Container,
                K::Component,
            ][..]],
            ExprScope::Deployment => &[&[
                K::CustomElement,
                K::DeploymentNode,
                K::InfrastructureNode,
                K::SoftwareSystemInstance,
                K::ContainerInstance,
            ][..]],
            ExprScope::Custom => &[&[K::CustomElement]],
        };
        let mut out = Items::new();
        for pass in passes {
            for rid in self.builder.all_relationship_ids() {
                if self.rel_destination(&rid).as_ref() == Some(element)
                    && let Some(source) = self.rel_source(&rid)
                {
                    if !pass.is_empty()
                        && !pass.contains(&self.builder.element_kind(&source).unwrap_or(K::Person))
                    {
                        continue;
                    }
                    let item = Mi::Element(source);
                    if !out.contains(&item) {
                        out.push(item);
                    }
                }
            }
        }
        out
    }

    /// Coupling expansion FROM the element — destinations it points
    /// AT; same per-class passes as [`Self::find_afferent_couplings`].
    fn find_efferent_couplings(&self, element: &Id, scope: ExprScope) -> Items {
        use oxidizr_core::validate::ElementKind as K;
        let passes: &[&[K]] = match scope {
            ExprScope::Model => &[&[]],
            ExprScope::Static => &[&[
                K::CustomElement,
                K::Person,
                K::SoftwareSystem,
                K::Container,
                K::Component,
            ][..]],
            ExprScope::Deployment => &[&[
                K::CustomElement,
                K::DeploymentNode,
                K::InfrastructureNode,
                K::SoftwareSystemInstance,
                K::ContainerInstance,
            ][..]],
            ExprScope::Custom => &[&[K::CustomElement]],
        };
        let mut out = Items::new();
        for pass in passes {
            for rid in self.builder.all_relationship_ids() {
                if self.rel_source(&rid).as_ref() == Some(element)
                    && let Some(destination) = self.rel_destination(&rid)
                {
                    if !pass.is_empty()
                        && !pass
                            .contains(&self.builder.element_kind(&destination).unwrap_or(K::Person))
                    {
                        continue;
                    }
                    let item = Mi::Element(destination);
                    if !out.contains(&item) {
                        out.push(item);
                    }
                }
            }
        }
        out
    }

    /// Identifier lookup across the element AND relationship
    /// namespaces; errors when neither resolves.
    pub(crate) fn parse_identifier(
        &mut self,
        identifier: &str,
        scope: ExprScope,
    ) -> Result<Items, DslError> {
        let mut items: Items = Vec::new();
        if self.get_element(identifier).is_some() {
            items.extend(
                self.get_elements(identifier, scope)
                    .into_iter()
                    .map(Mi::Element),
            );
        }
        if let Some(rel) = self.get_relationship(identifier) {
            items.push(Mi::Rel(rel.clone()));
            // plus every relationship linked to it (implied / replicated)
            for rid in self.builder.all_relationship_ids() {
                if self.builder.linked_relationship_id(&rid).as_ref() == Some(&rel) {
                    let item = Mi::Rel(rid);
                    if !items.contains(&item) {
                        items.push(item);
                    }
                }
            }
        }
        if items.is_empty() {
            return Err(self.stmt_err(format!(
                "The element/relationship \"{identifier}\" does not exist"
            )));
        }
        Ok(items)
    }

    /// A registered element, expanded through group member
    /// sets; EMPTY is not an error. The deployment dialect replaces a
    /// `SoftwareSystem`/`Container` with ALL of its instances — in EVERY
    /// environment (the current environment does not filter).
    fn get_elements(&self, identifier: &str, scope: ExprScope) -> Vec<Id> {
        use oxidizr_core::validate::ElementKind as K;
        let base = match self.get_element(identifier) {
            Some(RegItem::Element(id)) => vec![id],
            Some(RegItem::Pseudo { kind, uid, .. }) => {
                if matches!(kind, super::idents::PseudoKind::Group { .. }) {
                    self.group_members(uid)
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        if scope != ExprScope::Deployment {
            return base;
        }
        let mut out = Vec::new();
        for id in base {
            match self.builder.element_kind(&id) {
                Some(K::SoftwareSystem | K::Container) => {
                    for instance in self.builder.all_instances_of(&id) {
                        if !out.contains(&instance) {
                            out.push(instance);
                        }
                    }
                }
                _ => {
                    if !out.contains(&id) {
                        out.push(id);
                    }
                }
            }
        }
        out
    }

    /// The element set behind `relationship.source==`/`==destination`:
    /// an expression, a group expansion, the element itself, plus its
    /// instances when inside a deployment environment.
    fn resolve_endpoint(
        &mut self,
        identifier: &str,
        scope: ExprScope,
    ) -> Result<Vec<Id>, DslError> {
        let mut out: Vec<Id> = Vec::new();
        if is_expression(identifier) {
            for item in self.parse_expression(identifier, scope)? {
                if let Mi::Element(id) = item
                    && !out.contains(&id)
                {
                    out.push(id);
                }
            }
        } else {
            match self.get_element(identifier) {
                None => {
                    return Err(
                        self.stmt_err(format!("The element \"{identifier}\" does not exist"))
                    );
                }
                Some(RegItem::Element(id)) => {
                    out.push(id.clone());
                    // static-structure elements only — deployment nodes,
                    // infrastructure nodes and instances never fan out.
                    if matches!(
                        self.builder.element_kind(&id),
                        Some(
                            oxidizr_core::validate::ElementKind::Person
                                | oxidizr_core::validate::ElementKind::SoftwareSystem
                                | oxidizr_core::validate::ElementKind::Container
                                | oxidizr_core::validate::ElementKind::Component
                        )
                    ) && self.in_ctx(super::ctx::Ctx::is_environment)
                    {
                        for instance in self.builder.find_instances_of(&id, self.environment_name())
                        {
                            if !out.contains(&instance) {
                                out.push(instance);
                            }
                        }
                    }
                }
                Some(RegItem::Pseudo { kind, uid, .. }) => {
                    if matches!(kind, super::idents::PseudoKind::Group { .. }) {
                        out.extend(self.group_members(uid));
                    }
                }
                Some(RegItem::Relationship(_)) => {}
            }
        }
        Ok(out)
    }

    // --- small model accessors (relationship endpoints) -------------------

    fn rel_source(&self, id: &Id) -> Option<Id> {
        self.relationship_endpoint(id, true)
    }

    fn rel_destination(&self, id: &Id) -> Option<Id> {
        self.relationship_endpoint(id, false)
    }

    fn relationship_endpoint(&self, id: &Id, source: bool) -> Option<Id> {
        self.builder
            .workspace()
            .model
            .relationships
            .iter()
            .find(|r| &r.id == id)
            .map(|r| {
                if source {
                    r.source.clone()
                } else {
                    r.destination.clone()
                }
            })
    }

    /// The current deployment environment name for instance expansion;
    /// the top context IS an environment context at every call site.
    fn environment_name(&self) -> &str {
        match self.top() {
            Some(super::ctx::Ctx::Environment { env, .. }) => &env.name,
            Some(super::ctx::Ctx::NoRelInEnv { environment, .. }) => environment,
            _ => "",
        }
    }

    /// A statement-level error; the engine attaches file/line on capture.
    #[allow(clippy::unused_self)] // method symmetry with `Parser::stmt_err`
    fn stmt_err(&self, message: String) -> DslError {
        super::DslError::stmt(message)
    }
}

/// The `[name]` / `==value` halves of a property expression: cut at the
/// first `[`…`]` pair and the first `==`.
fn property_pair(expr: &str) -> (String, String) {
    let name = expr[expr.find('[').unwrap_or(0) + 1..expr.find(']').unwrap_or(0)].to_string();
    let value = expr[expr.find("==").unwrap_or(0) + 2..].to_string();
    (name, value)
}
