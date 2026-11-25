//! Property and perspective application legs of the parse driver: the
//! holder fan-out (`!property` / `!perspective` across workspace, model,
//! items, archetypes, views and styles) and the perspective append. The
//! section arms and the context-stack helpers call these through
//! `pub(super)`; the parse driver itself stays in the parent module.

use super::{DslError, Holder, Parser};

impl Parser<'_> {
    // ------------------------------------------------------------------
    // properties / perspectives holders
    // ------------------------------------------------------------------

    pub(super) fn apply_property(&mut self, holder: &Holder, name: &str, value: &str) {
        match holder {
            Holder::Workspace => self.builder.set_property(name, value),
            Holder::Model => {
                self.builder.model_mut().properties.insert(name, value);
            }
            Holder::Item(id) => {
                let _ = self.builder.add_item_property(id, name, value);
            }
            Holder::Items(ids) => {
                for id in ids {
                    let _ = self.builder.add_item_property(id, name, value);
                }
            }
            Holder::Archetype { idx } => {
                // `LinkedHashMap.put` semantics: first-insert order,
                // overwrite value.
                let a = &mut self.archetype_list[*idx];
                match a.properties.iter_mut().find(|(n, _)| n == name) {
                    Some(slot) => slot.1 = value.to_string(),
                    None => a.properties.push((name.to_string(), value.to_string())),
                }
            }
            Holder::View(view) => {
                if let Some(v) = self.builder.view_base_mut(*view) {
                    v.properties.insert(name, value);
                }
            }
            Holder::DynamicRel { view, rel } => {
                if let Some(rv) = self.relationship_view_mut(*view, rel) {
                    rv.properties.insert(name, value);
                }
            }
            Holder::ElementStyle { idx } => {
                if let Some(s) = self
                    .builder
                    .views_mut()
                    .configuration
                    .styles
                    .element_styles
                    .get_mut(*idx)
                {
                    s.properties.insert(name, value);
                }
            }
            Holder::RelationshipStyle { idx } => {
                if let Some(s) = self
                    .builder
                    .views_mut()
                    .configuration
                    .styles
                    .relationship_styles
                    .get_mut(*idx)
                {
                    s.properties.insert(name, value);
                }
            }
            // `views { properties … }` — the official parser's views context is
            // a PropertyHolder over the view-set configuration.
            Holder::ViewSet => {
                self.builder
                    .views_mut()
                    .configuration
                    .properties
                    .insert(name, value);
            }
        }
    }

    /// The plural perspectives statement form / closing a perspective
    /// — add to EVERY holder of the context.
    // One clone per holder (the official parser builds a fresh perspective per
    // holder too); the loop shape mirrors the official parser and keeps the
    // `?`-early-exit semantics identical.
    pub(super) fn add_perspective_to_holders(
        &mut self,
        holders: &[Holder],
        perspective: &oxidizr_core::model::Perspective,
    ) -> Result<(), DslError> {
        for h in holders {
            self.add_perspective_to_holder(h, perspective.clone())?;
        }
        Ok(())
    }

    fn add_perspective_to_holder(
        &mut self,
        holder: &Holder,
        perspective: oxidizr_core::model::Perspective,
    ) -> Result<(), DslError> {
        match holder {
            Holder::Item(id) => {
                self.builder
                    .add_perspective(id, perspective)
                    .map_err(|e| DslError::core(&e))?;
                Ok(())
            }
            Holder::Items(ids) => {
                // The official loop applies holders in order; a duplicate
                // on a later one leaves the earlier ones MUTATED (faithful).
                for id in ids {
                    self.builder
                        .add_perspective(id, perspective.clone())
                        .map_err(|e| DslError::core(&e))?;
                }
                Ok(())
            }
            Holder::Archetype { idx } => {
                // The archetype's perspective add — the same
                // duplicate guard.
                let dup = self.archetype_list[*idx]
                    .perspectives
                    .iter()
                    .any(|p| p.name == perspective.name);
                if dup {
                    return Err(DslError::stmt(format!(
                        "A perspective named \"{}\" already exists.",
                        perspective.name
                    )));
                }
                self.archetype_list[*idx].perspectives.push(perspective);
                Ok(())
            }
            // The official perspective universe covers model
            // items and archetypes only; a perspective elsewhere is parsed
            // and dropped (the context has no holder to append to).
            _ => Ok(()),
        }
    }
}
