//! The exporters' property cascade — the single typed read of view/view-set
//! properties (P2-D8; `agents/plan/build/EXPORT-PROPERTIES.md` §2).
//!
//! The reference reads every diagram property through
//! `AbstractDiagramExporter.getViewOrViewSetProperty` (848-line file,
//! 762-770, pinned `ad34036`):
//!
//! ```java
//! view.getProperties().getOrDefault(name,
//!     views.getConfiguration().getProperties().getOrDefault(name, defaultValue));
//! ```
//!
//! Two levels plus a default — the VIEW property wins; on absence the
//! `views.configuration` property; on absence again the call-site default.
//! Model and workspace properties are NOT consulted (catalog §2). The Java
//! `getOrDefault` semantics matter: a PRESENT entry is returned even when its
//! value is the empty string — only absence cascades (so `plantuml.title` set
//! to `""` on the view suppresses a view-set title rather than falling back
//! to it). Match case-sensitivity differs per property and lives with each
//! call site (catalog §4), never here.

use oxidizr_core::views::{ViewBase, ViewSet};

/// `getViewOrViewSetProperty`: the value of `name` on `view`, else on
/// `views.configuration.properties`, else `default`. Present-but-empty
/// values are returned verbatim — emptyness never cascades.
#[must_use]
pub fn export_property<'a>(
    view: &'a ViewBase,
    views: &'a ViewSet,
    name: &str,
    default: &'a str,
) -> &'a str {
    view.properties
        .get(name)
        .or_else(|| views.configuration.properties.get(name))
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::export_property;
    use oxidizr_core::views::{ViewBase, ViewSet};

    fn view(k: &str, name: &str, value: &str) -> ViewBase {
        let mut base = ViewBase::new(k, 1);
        base.properties.insert(name, value);
        base
    }

    fn views(name: &str, value: &str) -> ViewSet {
        let mut vs = ViewSet::default();
        vs.configuration.properties.insert(name, value);
        vs
    }

    #[test]
    fn view_level_property_wins() {
        let v = view("k", "plantuml.skinparams", "view-value");
        let vs = views("plantuml.skinparams", "viewset-value");
        assert_eq!(
            export_property(&v, &vs, "plantuml.skinparams", "default"),
            "view-value"
        );
    }

    #[test]
    fn viewset_level_fallback_on_absence() {
        let v = ViewBase::new("k", 1);
        let vs = views("plantuml.skinparams", "viewset-value");
        assert_eq!(
            export_property(&v, &vs, "plantuml.skinparams", "default"),
            "viewset-value"
        );
    }

    #[test]
    fn default_when_neither_level_has_the_key() {
        let v = view("k", "other", "x");
        let vs = views("also-other", "y");
        assert_eq!(export_property(&v, &vs, "plantuml.title", ""), "");
        assert_eq!(export_property(&v, &vs, "x", "d"), "d");
    }

    #[test]
    fn present_but_empty_value_is_returned_not_cascaded() {
        // Java `getOrDefault` returns present entries verbatim: an empty
        // view value suppresses the view-set value (and vice versa: an
        // empty view-set value suppresses the default).
        let v = view("k", "plantuml.title", "");
        let vs = views("plantuml.title", "viewset-title");
        assert_eq!(export_property(&v, &vs, "plantuml.title", "default"), "");
        let v = ViewBase::new("k", 1);
        let vs = views("plantuml.animation", "");
        assert_eq!(export_property(&v, &vs, "plantuml.animation", "true"), "");
    }
}
