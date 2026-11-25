//! Annotation setters of [`WorkspaceBuilder`]: the `set_*`/`add_*` legs that
//! attach perspectives, urls, properties, groups, technology, instances,
//! tags and descriptions to an existing element or relationship by id. The
//! item/relationship borrows they mutate stay private to the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::model::Perspective;

use super::{ElementMut, WorkspaceBuilder, java_split_csv};

impl WorkspaceBuilder {
    /// Attach a perspective to an element or relationship by id.
    pub fn add_perspective(&mut self, id: &Id, perspective: Perspective) -> Result<(), CoreError> {
        // Perspectives, object path — element, relationship and
        // instance equivalents — duplicate-name guard only
        // (the empty-name guard lives in the name-based add, which
        // the DSL inline/archetype-application paths run before calling
        // this).
        let duplicate = if let Some(item) = self.item_mut(id) {
            let dup = item.perspectives.iter().any(|p| p.name == perspective.name);
            if !dup {
                item.perspectives.push(perspective.clone());
            }
            dup
        } else if let Some(rel) = self.relationship_mut(id) {
            let dup = rel.perspectives.iter().any(|p| p.name == perspective.name);
            if !dup {
                rel.perspectives.push(perspective.clone());
            }
            dup
        } else {
            return Err(CoreError::ElementNotFound { id: id.clone() });
        };
        if duplicate {
            return Err(CoreError::DuplicatePerspective {
                name: perspective.name,
            });
        }
        Ok(())
    }

    /// Set the optional URL of an element or relationship.
    pub fn set_url(&mut self, id: &Id, url: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.url = Some(url.to_string());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.url = Some(url.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// Add a property to an element, relationship, or the model.
    pub fn add_item_property(
        &mut self,
        id: &Id,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.properties.insert(name.into(), value.into());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.properties.insert(name.into(), value.into());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `group "Name"` for any groupable element.
    pub fn set_group(&mut self, id: &Id, group: &str) -> Result<(), CoreError> {
        let mut element = self
            .element_mut(id)
            .ok_or_else(|| CoreError::ElementNotFound { id: id.clone() })?;
        element.set_group(Some(group.to_string()));
        Ok(())
    }

    /// `technology "…"` for containers/components/nodes/infrastructure nodes.
    pub fn set_technology(&mut self, id: &Id, technology: &str) -> Result<(), CoreError> {
        let mut element = self
            .element_mut(id)
            .ok_or_else(|| CoreError::ElementNotFound { id: id.clone() })?;
        element.set_technology(Some(technology.to_string()));
        Ok(())
    }

    /// `technology <technology>` for relationships — the
    /// fan-out arm.
    pub fn set_relationship_technology(
        &mut self,
        id: &Id,
        technology: &str,
    ) -> Result<(), CoreError> {
        if let Some(rel) = self.relationship_mut(id) {
            rel.technology = Some(technology.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `instances <number|range>` — `DeploymentNode.setInstances`: the
    /// the format stores the RAW string (numbers and ranges alike), no
    /// validation.
    pub fn set_instances(&mut self, id: &Id, instances: &str) -> Result<(), CoreError> {
        match self.element_mut(id) {
            Some(ElementMut::Node(n)) => {
                n.instances = instances.to_string();
                Ok(())
            }
            _ => Err(CoreError::ElementNotFound { id: id.clone() }),
        }
    }

    /// `tags "a,b"` — ADD tags (trimmed, first position).
    // Official-parity surface (the add-tags entry); the DSL statement uses
    // `add_tag_parts` (the varargs form) — kept per the parity surface,
    #[allow(dead_code)]
    pub fn add_tags(&mut self, id: &Id, csv: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            for t in java_split_csv(csv) {
                item.tags.add(t);
            }
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            for t in java_split_csv(csv) {
                rel.tags.add(t);
            }
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }

    /// `description "…"` for an element or relationship.
    pub fn set_description(&mut self, id: &Id, description: &str) -> Result<(), CoreError> {
        if let Some(item) = self.item_mut(id) {
            item.description = Some(description.to_string());
            return Ok(());
        }
        if let Some(rel) = self.relationship_mut(id) {
            rel.description = Some(description.to_string());
            return Ok(());
        }
        Err(CoreError::ElementNotFound { id: id.clone() })
    }
}
