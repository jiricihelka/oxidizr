//! Add/remove content operations for [`ViewRef`] views: element adds with
//! the relationship cascade, relationship adds with the gates from the
//! sibling module, and the removal legs. The element gates live in
//! [`super::gates`], the default generation and the step legs in the
//! sibling modules; the shared free helpers stay in the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::builder::WorkspaceBuilder;
use crate::error::CoreError;
use crate::ids::Id;

use crate::validate::ElementKind;
use crate::views::ElementView;

use super::{
    ViewRef, collect_deployment_subtree, upsert_relationship_view, view_scope_is_context_system,
};

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // add / remove
    // ------------------------------------------------------------------

    /// Add an element to a view (with the optional relationship cascade).
    pub fn view_add_element(
        &mut self,
        view: ViewRef,
        element: &Id,
        add_relationships: bool,
    ) -> Result<(), CoreError> {
        self.check_element_can_be_added(view, element)?;
        {
            let content = self
                .content_mut(view)
                .ok_or_else(|| CoreError::ElementNotFound {
                    id: element.clone(),
                })?;
            if !content.elements.iter().any(|e| &e.id == element) {
                content.elements.push(ElementView {
                    id: element.clone(),
                    x: 0,
                    y: 0,
                });
            }
        }
        if add_relationships {
            self.view_add_relationships_for(view, element)?;
        }
        Ok(())
    }

    /// The add-element cascade: every relationship whose ends
    /// are BOTH in the view now gets a relationship view.
    fn view_add_relationships_for(&mut self, view: ViewRef, element: &Id) -> Result<(), CoreError> {
        let in_view = self.view_element_ids(view);
        let rel_ids: Vec<Id> = self
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| {
                (&r.source == element || &r.destination == element)
                    && in_view.contains(&r.source)
                    && in_view.contains(&r.destination)
            })
            .map(|r| r.id.clone())
            .collect();
        let content = self.model_content_mut(view)?;
        for id in rel_ids {
            upsert_relationship_view(content, id);
        }
        Ok(())
    }

    /// `View.addRelationship(relationship)`: add only when both ends are in
    /// the view; returns whether the relationship view exists afterwards.
    pub fn view_add_relationship(
        &mut self,
        view: ViewRef,
        relationship: &Id,
    ) -> Result<bool, CoreError> {
        let (source, destination) = self
            .relationship(relationship)
            .map(|r| (r.source.clone(), r.destination.clone()))
            .ok_or_else(|| CoreError::RelationshipNotFound {
                id: relationship.clone(),
            })?;
        if !self.view_contains_element(view, &source)
            || !self.view_contains_element(view, &destination)
        {
            return Ok(false);
        }
        let content = self.model_content_mut(view)?;
        upsert_relationship_view(content, relationship.clone());
        Ok(true)
    }

    /// Remove an element — for a deployment node, with the subtree
    /// cascade.
    pub fn view_remove_element(&mut self, view: ViewRef, element: &Id) -> Result<(), CoreError> {
        if view_scope_is_context_system(self, view, element) {
            return Err(CoreError::ElementCannotBeRemoved {
                name: self
                    .item(element)
                    .map(|i| i.name.clone())
                    .unwrap_or_default(),
            });
        }
        let subtree: Vec<Id> = if matches!(view, ViewRef::Deployment(_))
            && self.element_kind(element) == Some(ElementKind::DeploymentNode)
        {
            let mut ids = Vec::new();
            collect_deployment_subtree(self, element, &mut ids);
            ids
        } else {
            vec![element.clone()]
        };
        for id in subtree {
            let connected: Vec<Id> = self
                .workspace()
                .model
                .relationships
                .iter()
                .filter(|r| r.source == id || r.destination == id)
                .map(|r| r.id.clone())
                .collect();
            if let Some(content) = self.content_mut(view) {
                content.elements.retain(|e| e.id != id);
                for rel in connected {
                    content.relationships.retain(|r| r.id != rel);
                }
            }
        }
        Ok(())
    }

    /// Remove a relationship from the view.
    pub fn view_remove_relationship(&mut self, view: ViewRef, relationship: &Id) {
        if let Some(content) = self.content_mut(view) {
            content.relationships.retain(|r| &r.id != relationship);
        }
    }

    /// Drop relationship views no longer connected to the kept elements.
    pub fn view_remove_relationships_not_connected(&mut self, view: ViewRef, keep: &[Id]) {
        let connected: Vec<Id> = self
            .workspace()
            .model
            .relationships
            .iter()
            .filter(|r| keep.contains(&r.source) || keep.contains(&r.destination))
            .map(|r| r.id.clone())
            .collect();
        if let Some(content) = self.content_mut(view) {
            content.relationships.retain(|r| connected.contains(&r.id));
        }
    }
}
