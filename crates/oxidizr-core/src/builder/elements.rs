//! Static-model element creation legs of [`WorkspaceBuilder`]: people,
//! software systems, custom elements, containers and components. The shared
//! `new_item` and duplicate-name helpers stay in the parent module.

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::model::{Component, Container, CustomElement, Person, SoftwareSystem};
use crate::validate::ElementKind;

use super::{WorkspaceBuilder, check_name};

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // elements
    // ------------------------------------------------------------------

    /// `person "Name" ["Desc"] ["Tags"]`.
    pub fn add_person(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name, ElementKind::Person)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.people.push(Person {
            item,
            group: None,
            relationships: Vec::new(),
        });
        Ok(id)
    }

    /// `softwareSystem "Name" ["Desc"] ["Tags"]`.
    pub fn add_software_system(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name, ElementKind::SoftwareSystem)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.software_systems.push(SoftwareSystem {
            item,
            group: None,
            containers: Vec::new(),
            documentation: crate::docs::Documentation::default(),
            relationships: Vec::new(),
        });
        Ok(id)
    }

    /// `element "Name" ["metadata"] ["Desc"] ["Tags"]` — custom element.
    ///
    /// Fresh DSL custom elements are stored in REVERSE creation
    /// order (witness: `A`..`F` export as `F`..`A`, and
    /// `Zulu`,`Alpha` as `Alpha`,`Zulu` — reverse insertion, not id sort,
    /// (the JSON-import path re-sorts ascending on load).
    /// JSON export is stored
    /// order, so insertion goes at the FRONT.
    pub fn add_custom_element(
        &mut self,
        name: &str,
        description: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        self.check_top_level_name(name, ElementKind::CustomElement)?;
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.workspace.model.custom_elements.insert(
            0,
            CustomElement {
                item,
                group: None,
                metadata: None,
                relationships: Vec::new(),
            },
        );
        Ok(id)
    }

    /// `container "Name" ["Desc"] ["Tech"] ["Tags"]` in `system`.
    pub fn add_container(
        &mut self,
        system: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        // One walk for the kind guard + duplicate-name check; the push
        // re-lookup surfaces the same `WrongElementKind` error instead of
        // the old `expect("checked above")` panic — nothing
        // changes the model between the two lookups, so the error arm is
        // unreachable in practice).
        let duplicate = {
            let s = self
                .system_ref(system)
                .ok_or_else(|| CoreError::WrongElementKind {
                    id: system.clone(),
                    expected: "software system",
                })?;
            s.containers.iter().any(|c| c.item.name == name)
        };
        if duplicate {
            return Err(CoreError::DuplicateContainerName {
                name: name.to_string(),
            });
        }
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.system_mut(system)
            .ok_or_else(|| CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            })?
            .containers
            .push(Container {
                item,
                group: None,
                technology,
                components: Vec::new(),
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(id)
    }

    /// `component "Name" ["Desc"] ["Tech"] ["Tags"]` in `container`.
    pub fn add_component(
        &mut self,
        container: &Id,
        name: &str,
        description: Option<String>,
        technology: Option<String>,
        tags_csv: Option<&str>,
    ) -> Result<Id, CoreError> {
        check_name(name)?;
        // Same single-walk guard + error-valued re-lookup as `add_container`.
        let duplicate = {
            let c = self
                .container_ref(container)
                .ok_or_else(|| CoreError::WrongElementKind {
                    id: container.clone(),
                    expected: "container",
                })?;
            c.components.iter().any(|k| k.item.name == name)
        };
        if duplicate {
            return Err(CoreError::DuplicateComponentName {
                name: name.to_string(),
            });
        }
        let item = self.new_item(name, description, tags_csv);
        let id = item.id.clone();
        self.container_mut(container)
            .ok_or_else(|| CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            })?
            .components
            .push(Component {
                item,
                group: None,
                technology,
                documentation: crate::docs::Documentation::default(),
                relationships: Vec::new(),
            });
        Ok(id)
    }
}
