//! Relationship creation and the implied-relationships engine for
//! [`WorkspaceBuilder`]: the add/insert legs, the
//! [`ImpliedRelationshipsStrategy`](super::ImpliedRelationshipsStrategy)
//! ancestor walk, instance replication and the efferent/lineage predicates
//! the official add contract needs. Canonical-name lookups and the
//! instance-find legs stay in the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::model::Relationship;
use crate::tags::Tags;
use crate::validate::ElementKind;

use super::{
    ImpliedRelationshipsStrategy, WorkspaceBuilder, all_deployment_groups, flatten_nodes,
    in_same_deployment_group, is_implied_kind, java_split_csv,
};

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // relationships
    // ------------------------------------------------------------------

    /// `[id = ]a -> b ["Description"] ["Technology"] ["Tags"]` (or the typed
    /// `a.uses(b, …)` DSL forms, all funnel here).
    ///
    /// The official add contract: parent/child pairs are rejected,
    /// an efferent source→destination relationship with the same
    /// (normalized) description is a duplicate, and the
    /// configured [`ImpliedRelationshipsStrategy`] runs for static↔static
    /// pairs exactly like the official tooling.
    pub fn add_relationship(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        if !self.has_element(source) {
            return Err(CoreError::ElementNotFound { id: source.clone() });
        }
        if !self.has_element(destination) {
            return Err(CoreError::ElementNotFound {
                id: destination.clone(),
            });
        }
        if self.is_child_of(source, destination) || self.is_child_of(destination, source) {
            return Err(CoreError::ParentChildRelationship);
        }
        // Duplicate check: same destination AND same description
        // (technology is NOT part of the check).
        if self.has_efferent(source, destination, description.as_deref().unwrap_or("")) {
            return Err(CoreError::DuplicateRelationship {
                source: self.canonical_name(source),
                destination: self.canonical_name(destination),
            });
        }
        let mut tags = Tags::new();
        if let Some(csv) = tags_csv {
            // DSL tags arrive TRIMMED (unlike
            // the JSON CSV path).
            for t in java_split_csv(csv) {
                tags.add(t);
            }
        }
        let id = self.insert_relationship(source, destination, description, technology, tags, None);
        // The strategy runs only when BOTH ends are
        // static-structure-ish (custom/person/system/container/component).
        if self.implied_strategy != ImpliedRelationshipsStrategy::Default
            && is_implied_kind(self.element_kind(source))
            && is_implied_kind(self.element_kind(destination))
        {
            self.create_implied_relationships(&id);
        }
        Ok(id)
    }

    /// Raw relationship insert (shared id counter, `source.relationships`
    /// index) — the raw insert without
    /// the guards; used by the implied and replication paths, which pre-check
    /// duplicates exactly like the official tooling.
    pub(crate) fn insert_relationship(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
        tags: Tags,
        linked_relationship_id: Option<&Id>,
    ) -> Id {
        let id = self.alloc_id();
        self.workspace.model.relationships.push(Relationship {
            id: id.clone(),
            source: source.clone(),
            destination: destination.clone(),
            description,
            technology,
            tags,
            url: None,
            properties: crate::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: linked_relationship_id.cloned(),
        });
        if let Some(rels) = self.source_relationships_mut(source) {
            rels.push(id.clone());
        }
        id
    }

    /// `AbstractImpliedRelationshipsStrategy` ancestor walk: for every
    /// (source-ancestor, destination-ancestor) pair allowed by
    /// `impliedRelationshipIsAllowed`, create the implied copy unless the
    /// strategy's efferent check says otherwise. Order matches the official
    /// loops (outer: source chain; inner: destination chain reset per step).
    pub(super) fn create_implied_relationships(&mut self, base_id: &Id) {
        // The base was inserted by the caller moments ago; if it were not
        // there, there is nothing to imply (no panic).
        let Some(base) = self.relationship(base_id) else {
            return;
        };
        let (src, dst, desc, tech) = (
            base.source.clone(),
            base.destination.clone(),
            base.description.clone(),
            base.technology.clone(),
        );
        let unless_same =
            self.implied_strategy == ImpliedRelationshipsStrategy::UnlessSameRelationshipExists;
        let mut source = Some(src);
        while let Some(s) = source {
            let mut destination = Some(dst.clone());
            while let Some(d) = destination {
                if self.implied_relationship_is_allowed(&s, &d) {
                    let create = if unless_same {
                        !self.has_efferent(&s, &d, desc.as_deref().unwrap_or(""))
                    } else {
                        !self.has_any_efferent(&s, &d)
                    };
                    if create {
                        self.insert_relationship(
                            &s,
                            &d,
                            desc.clone(),
                            tech.clone(),
                            Tags::new(),
                            Some(base_id),
                        );
                    }
                }
                destination = self.element_parent(&d);
            }
            source = self.element_parent(&s);
        }
    }

    /// `AbstractImpliedRelationshipsStrategy.impliedRelationshipIsAllowed`:
    /// not equal, and no ancestor/descendant pair (persons exempt).
    fn implied_relationship_is_allowed(&self, source: &Id, destination: &Id) -> bool {
        source != destination
            && !self.is_child_of(source, destination)
            && !self.is_child_of(destination, source)
    }

    /// Replication rule: when a
    /// software-system/container instance joins the model, replicate matching
    /// static-model relationships between every instance in the SAME
    /// environment and deployment group. Replicated relationships carry EMPTY
    /// tags and a `linkedRelationshipId`.
    pub(crate) fn replicate_element_relationships(&mut self, instance_id: &Id) {
        let Some((my_static, my_env, my_groups)) = self.instance_profile(instance_id) else {
            return;
        };
        // Official iteration is ordered by numeric id == insertion order.
        let others: Vec<(Id, Id, String, Vec<String>)> = self
            .all_instances()
            .into_iter()
            .filter(|(_, _, env, _)| *env == my_env)
            .filter(|(_, _, _, groups)| in_same_deployment_group(&my_groups, groups))
            .collect();
        for (other_id, other_static, _, _) in others {
            // element (new instance's static element) -> other static element
            let fwd_ids: Vec<Id> = self
                .workspace
                .model
                .relationships
                .iter()
                .filter(|r| r.source == my_static && r.destination == other_static)
                .map(|r| r.id.clone())
                .collect();
            for base_id in fwd_ids {
                // `base_id` was just collected from the same arena; if it
                // were gone there would be nothing to replicate (control flow,
                // not a panic).
                let Some((desc, tech)) = self
                    .relationship(&base_id)
                    .map(|b| (b.description.clone(), b.technology.clone()))
                else {
                    continue;
                };
                if self.has_efferent(instance_id, &other_id, desc.as_deref().unwrap_or("")) {
                    continue; // Java: addRelationship returns null → skipped
                }
                self.insert_relationship(
                    instance_id,
                    &other_id,
                    desc,
                    tech,
                    Tags::new(),
                    Some(&base_id),
                );
            }
            // other static element -> element
            let rev_ids: Vec<Id> = self
                .workspace
                .model
                .relationships
                .iter()
                .filter(|r| r.source == other_static && r.destination == my_static)
                .map(|r| r.id.clone())
                .collect();
            for base_id in rev_ids {
                // `base_id` was just collected from the same arena; if it
                // were gone there would be nothing to replicate (control flow,
                // not a panic).
                let Some((desc, tech)) = self
                    .relationship(&base_id)
                    .map(|b| (b.description.clone(), b.technology.clone()))
                else {
                    continue;
                };
                if self.has_efferent(&other_id, instance_id, desc.as_deref().unwrap_or("")) {
                    continue;
                }
                self.insert_relationship(
                    &other_id,
                    instance_id,
                    desc,
                    tech,
                    Tags::new(),
                    Some(&base_id),
                );
            }
        }
    }

    /// `(static element id, environment, all deployment groups)` of an
    /// instance id, or `None` when it is not an instance.
    pub(super) fn instance_profile(&self, id: &Id) -> Option<(Id, String, Vec<String>)> {
        for (node, inherited) in flatten_nodes(&self.workspace.model.deployment_nodes) {
            for s in &node.software_system_instances {
                if &s.item.id == id {
                    return Some((
                        s.software_system_id.clone(),
                        s.environment.clone(),
                        all_deployment_groups(&inherited, &s.deployment_groups),
                    ));
                }
            }
            for c in &node.container_instances {
                if &c.item.id == id {
                    return Some((
                        c.container_id.clone(),
                        c.environment.clone(),
                        all_deployment_groups(&inherited, &c.deployment_groups),
                    ));
                }
            }
        }
        None
    }

    /// Every software-system and container instance as
    /// `(instance id, static element id, environment, groups)` in id order.
    fn all_instances(&self) -> Vec<(Id, Id, String, Vec<String>)> {
        let mut out = Vec::new();
        for (node, inherited) in flatten_nodes(&self.workspace.model.deployment_nodes) {
            for s in &node.software_system_instances {
                out.push((
                    s.item.id.clone(),
                    s.software_system_id.clone(),
                    s.environment.clone(),
                    all_deployment_groups(&inherited, &s.deployment_groups),
                ));
            }
            for c in &node.container_instances {
                out.push((
                    c.item.id.clone(),
                    c.container_id.clone(),
                    c.environment.clone(),
                    all_deployment_groups(&inherited, &c.deployment_groups),
                ));
            }
        }
        out
    }

    /// Whether an efferent relationship to `destination` exists
    /// (`description` normalized to `""` on both sides).
    pub(crate) fn has_efferent(&self, source: &Id, destination: &Id, description: &str) -> bool {
        self.workspace.model.relationships.iter().any(|r| {
            &r.source == source
                && &r.destination == destination
                && r.description.as_deref().unwrap_or("") == description
        })
    }

    /// Whether an efferent relationship to `destination` exists (any
    /// description).
    pub(crate) fn has_any_efferent(&self, source: &Id, destination: &Id) -> bool {
        self.workspace
            .model
            .relationships
            .iter()
            .any(|r| &r.source == source && &r.destination == destination)
    }

    /// Is `e1` a (strict) descendant of `e2`?
    /// Persons have no lineage.
    pub(crate) fn is_child_of(&self, e1: &Id, e2: &Id) -> bool {
        if self.element_kind(e1) == Some(ElementKind::Person)
            || self.element_kind(e2) == Some(ElementKind::Person)
        {
            return false;
        }
        let mut parent = self.element_parent(e2);
        while let Some(p) = parent {
            if &p == e1 {
                return true;
            }
            parent = self.element_parent(&p);
        }
        false
    }
}
