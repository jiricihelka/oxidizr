//! Step legs of the view content: the animation-step append and the dynamic
//! view's relationship / element-pair steps (including the reverse lookup
//! that produces `response: true`). The gates and the add/remove legs live
//! in the sibling modules; the shared free helpers stay in the parent.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;
use crate::views::{Animation, RelationshipView};

use super::{ViewRef, find_rel, find_rel_last};

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // animation steps
    // ------------------------------------------------------------------

    /// The animation-step add rule: elements must be in the view and not
    /// already revealed by an earlier step; a relationship joins the step
    /// when exactly one of its ends is revealed now (the other having been
    /// revealed earlier).
    pub fn view_add_animation_step(
        &mut self,
        view: ViewRef,
        elements: &[Id],
    ) -> Result<(), CoreError> {
        let mut previous: Vec<String> = Vec::new();
        if let Some(animations) = self.animations_mut(view) {
            for step in animations.iter() {
                previous.extend(step.element_ids.iter().cloned());
            }
        } else {
            return Ok(());
        }
        let mut this_step: Vec<Id> = Vec::new();
        let deployment = matches!(view, ViewRef::Deployment(_));
        for id in elements {
            if self.view_contains_element(view, id) {
                let s = id.as_str().to_string();
                if !previous.contains(&s) {
                    previous.push(s);
                    this_step.push(id.clone());
                    if deployment {
                        // Deployment-view animation steps reveal the
                        // element's deployment-node ancestors too.
                        let mut parent = self.element_parent(id);
                        while let Some(p) = parent {
                            let ps = p.as_str().to_string();
                            if !previous.contains(&ps) {
                                previous.push(ps);
                                this_step.push(p.clone());
                            }
                            parent = self.element_parent(&p);
                        }
                    }
                }
            }
        }
        if this_step.is_empty() {
            return Err(CoreError::NoAnimatableElements);
        }
        let rels = self.animation_step_relationships(view, &this_step, &previous);
        let order = i32::try_from(
            self.animations_mut(view)
                .map_or(0, |a| a.len())
                .saturating_add(1),
        )
        .unwrap_or(i32::MAX);
        if let Some(animations) = self.animations_mut(view) {
            animations.push(Animation {
                order,
                element_ids: this_step.iter().map(|i| i.as_str().to_string()).collect(),
                relationship_ids: rels,
            });
        }
        Ok(())
    }

    /// The relationship ids revealed by one animation step: a view
    /// relationship-view joins when exactly one of its model endpoints is in
    /// this step and the other was revealed by an earlier one (same id
    /// spellings as the stored step; the add-side rule is cited on
    /// [`Self::view_add_animation_step`]). Empty for a view without content.
    fn animation_step_relationships(
        &self,
        view: ViewRef,
        this_step: &[Id],
        previous: &[String],
    ) -> Vec<String> {
        self.content_ref(view)
            .map(|content| {
                content
                    .relationships
                    .iter()
                    .filter(|rv| {
                        let model = self.relationship(&rv.id);
                        let Some(model) = model else {
                            return false;
                        };
                        let src_now = this_step.contains(&model.source);
                        let dst_now = this_step.contains(&model.destination);
                        // membership by `&str` — `previous` holds the id
                        // spellings, so no per-comparison String is needed.
                        let src_prev = previous.iter().any(|p| p == model.source.as_str());
                        let dst_prev = previous.iter().any(|p| p == model.destination.as_str());
                        (src_now && dst_prev) || (src_prev && dst_now)
                    })
                    .map(|rv| rv.id.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // dynamic view steps
    // ------------------------------------------------------------------

    /// The `<relationshipIdentifier> [description]` step form. The step
    /// number (`order`) comes from the DSL engine's sequence counter.
    ///
    /// Every successful add INSERTS: the step set is keyed by
    /// `id + "/" + order`, so a re-add with a fresh sequence order is a
    /// NEW entry — duplicate
    /// relationship ids inside one dynamic view are the whole point of the
    /// step form. The `description` is stored verbatim (the DSL parser
    /// passes `""` when absent; the JSON writer omits empties).
    pub fn dynamic_add_relationship(
        &mut self,
        view: ViewRef,
        relationship: &Id,
        description: &str,
        order: &str,
    ) -> Result<Id, CoreError> {
        let (source, destination) = self
            .relationship(relationship)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .ok_or_else(|| CoreError::RelationshipNotFound {
                id: relationship.clone(),
            })?;
        self.check_element_can_be_added(view, &source)?;
        self.check_element_can_be_added(view, &destination)?;
        self.view_add_element(view, &source, false)?;
        self.view_add_element(view, &destination, false)?;
        let content = self.model_content_mut(view)?;
        content.relationships.push(RelationshipView {
            id: relationship.clone(),
            description: Some(description.to_string()),
            url: None,
            order: Some(order.to_string()),
            response: Some(false),
            properties: crate::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
        Ok(relationship.clone())
    }

    /// The
    /// `a -> b [description] [technology]` step form. The model relationship
    /// must exist: with no technology the lookup prefers the relationship
    /// whose (normalized) description equals the given one and falls back to
    /// any source→destination relationship; with a technology it's an exact
    /// technology match. A reverse relationship ALREADY IN THE VIEW is added
    /// as `response: true`; otherwise [`CoreError::DynamicRelationshipMissing`].
    /// Like [`Self::dynamic_add_relationship`] every successful add INSERTS
    /// (the step set is keyed by id AND order), and the resolved
    /// relationship id is returned.
    pub fn dynamic_add_element_pair(
        &mut self,
        view: ViewRef,
        source: &Id,
        description: &str,
        technology: &str,
        destination: &Id,
        order: &str,
    ) -> Result<Id, CoreError> {
        self.check_element_can_be_added(view, source)?;
        self.check_element_can_be_added(view, destination)?;
        // All lookups complete before the first mutation below, so they run
        // on a shared borrow of the relationship arena — no need to project
        // every relationship (incl. description/technology Strings) into a
        // snapshot Vec first.
        let (rel_id, response) = {
            let rels = &self.workspace().model.relationships;
            // Forward lookup: no technology →
            // the description-matched efferent lookup — an EXACT
            // normalized-description match ("" matches ""!) — falling back
            // to any efferent source→destination relationship; with a
            // technology the loop runs WITHOUT break, so the LAST
            // tech match wins.
            let forward = if technology.is_empty() {
                find_rel(rels, source, destination, Some(description), "")
                    .or_else(|| find_rel(rels, source, destination, None, ""))
            } else {
                find_rel_last(rels, source, destination, technology)
            };
            if let Some(id) = forward {
                (id, false)
            } else {
                // Reverse lookup; usable only when that relationship is
                // ALREADY in this view (`response: true`).
                let reverse = if technology.is_empty() {
                    find_rel(rels, destination, source, None, "")
                } else {
                    find_rel_last(rels, destination, source, technology)
                };
                match reverse {
                    Some(id) if self.view_contains_relationship(view, &id) => (id, true),
                    _ => {
                        return Err(CoreError::DynamicRelationshipMissing {
                            source: self
                                .item(source)
                                .map(|i| i.name.clone())
                                .unwrap_or_default(),
                            destination: self
                                .item(destination)
                                .map(|i| i.name.clone())
                                .unwrap_or_default(),
                            technology: (!technology.is_empty()).then(|| technology.to_string()),
                        });
                    }
                }
            }
        };
        self.view_add_element(view, source, false)?;
        self.view_add_element(view, destination, false)?;
        let content = self.model_content_mut(view)?;
        content.relationships.push(RelationshipView {
            id: rel_id.clone(),
            description: Some(description.to_string()),
            url: None,
            order: Some(order.to_string()),
            response: Some(response),
            properties: crate::properties::Properties::new(),
            vertices: Vec::new(),
            routing: None,
            jump: None,
            position: None,
        });
        Ok(rel_id)
    }
}
