//! View definitions for [`WorkspaceBuilder`]: the `create_*_view` family and
//! view-key generation. Content operations for these kinds live in
//! [`crate::viewops`].

#![allow(clippy::missing_errors_doc, clippy::missing_panics_doc)]

use crate::error::CoreError;
use crate::ids::Id;
use crate::viewops::ViewRef;
use crate::views::{
    ContainerView, CustomView, DeploymentView, DynamicView, FilteredView, ImageView,
    SystemContextView, SystemLandscapeView, ViewBase, ViewContent,
};

use super::WorkspaceBuilder;

impl WorkspaceBuilder {
    // ------------------------------------------------------------------
    // views
    // ------------------------------------------------------------------
    //
    // Content operations for these live in [`crate::viewops`] (the
    // view-content operations for the model-view kinds).

    /// Create a system landscape view.
    pub fn create_landscape_view(&mut self, key: &str) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "SystemLandscape")?;
        self.workspace
            .views
            .system_landscape_views
            .push(SystemLandscapeView {
                base,
                content: ViewContent::default(),
                // The format default is `enterpriseBoundaryVisible = true`.
                enterprise_boundary_visible: true,
                animations: Vec::new(),
            });
        Ok(ViewRef::Landscape(
            self.workspace.views.system_landscape_views.len() - 1,
        ))
    }

    /// Create a system context view scoped to `system`. Like the official
    /// constructor, the scoped system is added to the
    /// view immediately (with its relationships).
    pub fn create_context_view(&mut self, system: &Id, key: &str) -> Result<ViewRef, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        let base = self.new_view_base(key, "SystemContext")?;
        self.workspace
            .views
            .system_context_views
            .push(SystemContextView {
                base,
                content: ViewContent::default(),
                software_system_id: system.clone(),
                // The format default is `enterpriseBoundaryVisible = true`.
                enterprise_boundary_visible: true,
                animations: Vec::new(),
            });
        let view = ViewRef::Context(self.workspace.views.system_context_views.len() - 1);
        // `SystemContextView()` ctor: `addElement(softwareSystem, true)`.
        self.view_add_element(view, system, true)?;
        Ok(view)
    }

    /// Create a container view scoped to `system`.
    pub fn create_container_view(&mut self, system: &Id, key: &str) -> Result<ViewRef, CoreError> {
        if !self.is_system(system) {
            return Err(CoreError::WrongElementKind {
                id: system.clone(),
                expected: "software system",
            });
        }
        let base = self.new_view_base(key, "Container")?;
        self.workspace.views.container_views.push(ContainerView {
            base,
            content: ViewContent::default(),
            software_system_id: system.clone(),
            external_software_system_boundaries_visible: false,
            animations: Vec::new(),
        });
        Ok(ViewRef::Container(
            self.workspace.views.container_views.len() - 1,
        ))
    }

    /// Create a component view scoped to `container`.
    pub fn create_component_view(
        &mut self,
        container: &Id,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        if !self.is_container(container) {
            return Err(CoreError::WrongElementKind {
                id: container.clone(),
                expected: "container",
            });
        }
        let base = self.new_view_base(key, "Component")?;
        self.workspace
            .views
            .component_views
            .push(crate::views::ComponentView {
                base,
                content: ViewContent::default(),
                container_id: container.clone(),
                external_container_boundaries_visible: false,
                animations: Vec::new(),
            });
        Ok(ViewRef::Component(
            self.workspace.views.component_views.len() - 1,
        ))
    }

    /// Create a dynamic view (element-scoped or workspace-wide).
    pub fn create_dynamic_view(
        &mut self,
        element: Option<Id>,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        if let Some(id) = &element
            && !self.has_element(id)
        {
            return Err(CoreError::ElementNotFound { id: id.clone() });
        }
        let base = self.new_view_base(key, "Dynamic")?;
        self.workspace.views.dynamic_views.push(DynamicView {
            base,
            content: ViewContent::default(),
            element_id: element,
            deployment_environment: None,
            external_boundaries_visible: false,
        });
        Ok(ViewRef::Dynamic(
            self.workspace.views.dynamic_views.len() - 1,
        ))
    }

    /// Create a deployment view for `environment`.
    pub fn create_deployment_view(
        &mut self,
        environment: &str,
        key: &str,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Deployment")?;
        self.workspace.views.deployment_views.push(DeploymentView {
            base,
            content: ViewContent::default(),
            environment: environment.to_string(),
            software_system_id: None,
            animations: Vec::new(),
        });
        Ok(ViewRef::Deployment(
            self.workspace.views.deployment_views.len() - 1,
        ))
    }

    /// Create a filtered view over `base_key` (existence checked by
    /// [`mod@crate::validate`], as in the official validation step).
    pub fn create_filtered_view(
        &mut self,
        base_key: &str,
        key: &str,
        mode: crate::views::FilterMode,
        tags: Vec<String>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Filtered")?;
        self.workspace.views.filtered_views.push(FilteredView {
            base,
            base_view_key: base_key.to_string(),
            mode: Some(mode),
            tags,
        });
        Ok(ViewRef::Filtered(
            self.workspace.views.filtered_views.len() - 1,
        ))
    }

    /// Create an image view (DSL `image` / `plantuml`-style content views).
    pub fn create_image_view(
        &mut self,
        element: Option<Id>,
        key: &str,
        content: Option<String>,
        content_type: Option<String>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Image")?;
        self.workspace.views.image_views.push(ImageView {
            base,
            element_id: element,
            content,
            content_light: None,
            content_dark: None,
            content_type,
        });
        Ok(ViewRef::Image(self.workspace.views.image_views.len() - 1))
    }

    /// Create a custom view (`custom <name> [description] [key]`).
    pub fn create_custom_view(
        &mut self,
        key: &str,
        software_system: Option<Id>,
    ) -> Result<ViewRef, CoreError> {
        let base = self.new_view_base(key, "Custom")?;
        self.workspace.views.custom_views.push(CustomView {
            base,
            content: ViewContent::default(),
            animations: Vec::new(),
            software_system_id: software_system,
        });
        Ok(ViewRef::Custom(self.workspace.views.custom_views.len() - 1))
    }

    /// Build a view base applying the official view-set rules: an empty key
    /// triggers key generation: `<Prefix>-<counter:000>` scanning
    /// from 1 (marks `generatedKey`); a given key must be unique across ALL
    /// views; `order` = max existing order + 1 across the whole view set.
    /// The view set is walked ONCE (rebuilding and rescanning `all_views()`
    /// per key-candidate was wasted work; behaviour is identical).
    fn new_view_base(&mut self, key: &str, prefix: &str) -> Result<ViewBase, CoreError> {
        let views = self.workspace.views.all_views();
        let order = views
            .iter()
            .map(|v| v.order())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let (key, generated) = if key.is_empty() {
            (Self::generate_view_key(&views, prefix), true)
        } else {
            if views.iter().any(|v| v.key() == key) {
                return Err(CoreError::DuplicateViewKey {
                    key: key.to_string(),
                });
            }
            (key.to_string(), false)
        };
        let mut base = ViewBase::new(key, order);
        base.generated_key = generated;
        Ok(base)
    }

    /// Generated view key: 3-digit zero-padded scan from 1 over a
    /// once-collected key set.
    fn generate_view_key(views: &[crate::views::View<'_>], prefix: &str) -> String {
        let taken: std::collections::HashSet<&str> = views.iter().map(|v| v.key()).collect();
        let mut n = 1_u32;
        loop {
            let candidate = format!("{prefix}-{n:03}");
            if !taken.contains(candidate.as_str()) {
                return candidate;
            }
            n += 1;
        }
    }
}
