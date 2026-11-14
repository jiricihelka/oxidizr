//! Tags: an insertion-ordered unique set with DERIVED default tags (Locked stack
//! table, `plan/FEATURES.md` §3).
//!
//! Reference model (`model/ModelItem.java`, verified @ `ad34036`):
//!
//! * The `tags` FIELD stores only what was explicitly set/added ("custom" tags);
//!   each kind contributes [`default_tags`] which are PREPENDED (deduped) at
//!   read time — `getTags() = LinkedHashSet(defaults ++ stored)`.
//! * `setTags(csv)` (JSON read path): CLEARS the stored set, then `csv.split(",")`
//!   WITHOUT trimming — `"a, b"` stores `["a", " b"]` (bug-compatible). The
//!   defaults therefore get re-stored on a JSON load and deduped again on write.
//! * `addTags(...)` (DSL / ctor path): TRIMS each tag, skips empties? (no: an
//!   empty/whitespace tag is stored as-is if non-null), keeps first position.
//! * Relationship defaults depend on state: a linked (implied/replicated)
//!   relationship has NO default tags at all; a normal one gets `Relationship`
//!   plus `Synchronous`/`Asynchronous` when an interaction style is set
//!   (`Relationship.getDefaultTags`, verified).
//! * `SoftwareSystemInstance`/`ContainerInstance` have NO derived defaults —
//!   their kind tag is stored by the constructor (the v5 claim that instances
//!   inherit the parent element's tags is GONE in v6).

/// Tag-name constants (`model/Tags.java`).
pub mod tag {
    pub const ELEMENT: &str = "Element";
    pub const RELATIONSHIP: &str = "Relationship";
    pub const PERSON: &str = "Person";
    pub const SOFTWARE_SYSTEM: &str = "Software System";
    pub const CONTAINER: &str = "Container";
    pub const COMPONENT: &str = "Component";
    pub const DEPLOYMENT_NODE: &str = "Deployment Node";
    pub const INFRASTRUCTURE_NODE: &str = "Infrastructure Node";
    pub const SOFTWARE_SYSTEM_INSTANCE: &str = "Software System Instance";
    pub const CONTAINER_INSTANCE: &str = "Container Instance";
    pub const SYNCHRONOUS: &str = "Synchronous";
    pub const ASYNCHRONOUS: &str = "Asynchronous";
}

/// The derived-default tag stack of each model item kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Person,
    SoftwareSystem,
    Container,
    Component,
    /// DSL `element` — default tags are just `Element` (kind comes from custom tags).
    CustomElement,
    DeploymentNode,
    InfrastructureNode,
    /// Tags come entirely from the stored set (ctor adds `Software System Instance`).
    SoftwareSystemInstance,
    /// Tags come entirely from the stored set (ctor adds `Container Instance`).
    ContainerInstance,
}

impl TagKind {
    /// The reference `getDefaultTags()` stack, in serialization order.
    #[must_use]
    pub const fn default_tags(self) -> &'static [&'static str] {
        use tag::{
            COMPONENT, CONTAINER, DEPLOYMENT_NODE, ELEMENT, INFRASTRUCTURE_NODE, PERSON,
            SOFTWARE_SYSTEM,
        };
        match self {
            TagKind::Person => &[ELEMENT, PERSON],
            TagKind::SoftwareSystem => &[ELEMENT, SOFTWARE_SYSTEM],
            TagKind::Container => &[ELEMENT, CONTAINER],
            TagKind::Component => &[ELEMENT, COMPONENT],
            TagKind::CustomElement => &[ELEMENT],
            TagKind::DeploymentNode => &[ELEMENT, DEPLOYMENT_NODE],
            TagKind::InfrastructureNode => &[ELEMENT, INFRASTRUCTURE_NODE],
            TagKind::SoftwareSystemInstance | TagKind::ContainerInstance => &[],
        }
    }
}

/// Ordered tag collection: the default tags are derived, the stored (custom)
/// tags keep insertion order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    stored: Vec<String>,
}

impl Tags {
    /// Empty tag set.
    #[must_use]
    pub fn new() -> Tags {
        Tags { stored: Vec::new() }
    }

    /// The stored (custom) tags, in insertion order.
    #[must_use]
    pub fn stored(&self) -> &[String] {
        &self.stored
    }

    /// JSON-load path: replace the stored set from a comma-separated string.
    /// Reference `setTags(String)`: `split(",")` with NO trim (a stored default
    /// tag simply re-dedups against the derived one on write).
    #[must_use]
    pub fn set_from_csv(csv: &str) -> Tags {
        let mut tags = Tags::new();
        for part in csv.split(',') {
            tags.push_unique(part);
        }
        tags
    }

    /// Add one explicitly-given tag (DSL `tags "a,b"` parts or `tag "x"`),
    /// trimming — reference `addTags(String...)` (`tag.trim()`, first position
    /// kept on duplicates).
    pub fn add(&mut self, tag: &str) {
        self.push_unique(tag.trim());
    }

    /// Whether the tag is stored (prefer [`Tags::has`] with defaults).
    #[must_use]
    pub fn contains_stored(&self, tag: &str) -> bool {
        self.stored.iter().any(|t| t == tag)
    }

    /// Remove a stored tag; derived defaults cannot be removed (the reference
    /// `removeTag` CAN remove a re-stored default copy — mirrored: it just
    /// removes what is stored).
    pub fn remove(&mut self, tag: &str) -> bool {
        if let Some(pos) = self.stored.iter().position(|t| t == tag) {
            self.stored.remove(pos);
            true
        } else {
            false
        }
    }

    /// `getTagsAsSet().contains(tag)`: defaults first, then stored.
    #[must_use]
    pub fn has(&self, kind: TagKind, tag: &str) -> bool {
        self.has_against(kind.default_tags(), tag)
    }

    /// [`Tags::has`] with explicitly supplied defaults (relationship stacks).
    #[must_use]
    pub fn has_against(&self, defaults: &[&str], tag: &str) -> bool {
        defaults.contains(&tag) || self.contains_stored(tag)
    }

    /// `getTags()` for an element kind (derived stack from [`TagKind`]).
    #[must_use]
    pub fn serialized_for(&self, kind: TagKind) -> String {
        self.serialized(kind.default_tags())
    }

    /// `getTags()`: `defaults ++ stored`, deduped keeping first position, joined
    /// with `,`.
    #[must_use]
    pub fn serialized(&self, defaults: &[&str]) -> String {
        self.serialized_view(defaults).join(",")
    }

    fn serialized_view(&self, defaults: &[&str]) -> Vec<String> {
        let mut out: Vec<String> = Vec::with_capacity(self.stored.len() + defaults.len());
        let push = |t: &str, out: &mut Vec<String>| {
            if !out.iter().any(|s| s == t) {
                out.push(t.to_string());
            }
        };
        for d in defaults {
            push(d, &mut out);
        }
        for t in &self.stored {
            push(t, &mut out);
        }
        out
    }

    fn push_unique(&mut self, tag: &str) {
        if !self.stored.iter().any(|t| t == tag) {
            self.stored.push(tag.to_string());
        }
    }
}

/// Relationship interaction style (`model/InteractionStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionStyle {
    Synchronous,
    Asynchronous,
}

impl InteractionStyle {
    /// JSON spelling (the Java constant name).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            InteractionStyle::Synchronous => "Synchronous",
            InteractionStyle::Asynchronous => "Asynchronous",
        }
    }

    /// Parse the JSON spelling (exact case).
    #[must_use]
    pub fn parse(s: &str) -> Option<InteractionStyle> {
        match s {
            "Synchronous" => Some(InteractionStyle::Synchronous),
            "Asynchronous" => Some(InteractionStyle::Asynchronous),
            _ => None,
        }
    }
}

/// `Relationship.getDefaultTags()`: a linked (implied / instance-replicated)
/// relationship has NO default tags at all; a normal one gets `Relationship`,
/// plus the interaction style tag when a style is set.
#[must_use]
pub fn relationship_defaults(linked: bool, style: Option<InteractionStyle>) -> Vec<&'static str> {
    if linked {
        return Vec::new();
    }
    let mut out = vec![tag::RELATIONSHIP];
    match style {
        Some(InteractionStyle::Synchronous) => out.push(tag::SYNCHRONOUS),
        Some(InteractionStyle::Asynchronous) => out.push(tag::ASYNCHRONOUS),
        None => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_prepend_and_dedup() {
        let mut t = Tags::new();
        t.add("Foo");
        assert_eq!(t.serialized_for(TagKind::Person), "Element,Person,Foo");
        // A re-stored "Element" (from a JSON round-trip) dedups in place.
        let t2 = Tags::set_from_csv("Element,Person,Foo");
        assert_eq!(t2.serialized_for(TagKind::Person), "Element,Person,Foo");
    }

    #[test]
    fn set_from_csv_does_not_trim() {
        // Reference `split(",")` keeps the space: " b" is a distinct stored tag.
        let t = Tags::set_from_csv("a, b");
        assert_eq!(t.stored(), ["a".to_string(), " b".to_string()]);
        assert!(t.contains_stored(" b"));
        assert!(!t.contains_stored("b"));
    }

    #[test]
    fn add_trims_and_keeps_first_position() {
        let mut t = Tags::new();
        t.add("  Foo ");
        t.add("Bar");
        t.add("Foo");
        assert_eq!(t.stored(), ["Foo".to_string(), "Bar".to_string()]);
    }

    #[test]
    fn custom_element_default_is_element_only() {
        let t = Tags::set_from_csv("Mine,Element");
        assert_eq!(t.serialized_for(TagKind::CustomElement), "Element,Mine");
    }

    #[test]
    fn instances_have_no_derived_defaults() {
        let mut t = Tags::new();
        t.add(tag::SOFTWARE_SYSTEM_INSTANCE);
        assert_eq!(
            t.serialized_for(TagKind::SoftwareSystemInstance),
            tag::SOFTWARE_SYSTEM_INSTANCE
        );
    }

    #[test]
    fn relationship_defaults_are_stateful() {
        assert_eq!(relationship_defaults(false, None), [tag::RELATIONSHIP]);
        assert_eq!(
            relationship_defaults(false, Some(InteractionStyle::Asynchronous)),
            [tag::RELATIONSHIP, tag::ASYNCHRONOUS]
        );
        assert_eq!(
            relationship_defaults(true, Some(InteractionStyle::Synchronous)),
            Vec::<&'static str>::new()
        );
        let t = Tags::set_from_csv("Synchronous,Relationship");
        assert_eq!(
            t.serialized(&relationship_defaults(true, None)),
            "Synchronous,Relationship"
        );
    }
}
