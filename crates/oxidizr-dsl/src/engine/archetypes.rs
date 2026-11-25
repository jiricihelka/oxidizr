//! The archetype registry of the parse driver: keyword-or-archetype token
//! recognition and the `!archetype` extend/add bookkeeping. The section arms
//! call these through `pub(super)`; the parse driver itself stays in the
//! parent module.

use super::{Archetype, Parser, eqi, split_keeping_lead_dropping_trail, tok};

impl Parser<'_> {
    // ------------------------------------------------------------------
    // archetype registry
    // ------------------------------------------------------------------

    pub(super) fn is_element_keyword_or_archetype(&self, token: &str, keyword: &str) -> bool {
        eqi(token, keyword)
            || self
                .archetypes
                .get(keyword)
                .is_some_and(|m| m.contains_key(&token.to_lowercase()))
    }

    /// The relationship keyword-or-archetype test. Known corner: the token
    /// `-->` satisfies both prefix and suffix, and the official
    /// tail check THROWS (`String index out of range`) - here it
    /// evaluates as no-match (recorded in a documented deviation; no corpus file
    /// uses `-->`).
    pub(super) fn is_relationship_keyword_or_archetype(&self, token: &str) -> bool {
        if eqi(token, tok::RELATIONSHIP) {
            return true;
        }
        if token.len() >= 4
            && token.starts_with(tok::RELATIONSHIP_ARCHETYPE_PREFIX)
            && token.ends_with(tok::RELATIONSHIP_ARCHETYPE_SUFFIX)
        {
            let inner = &token[2..token.len() - 2];
            return self
                .archetypes
                .get(tok::RELATIONSHIP)
                .is_some_and(|m| m.contains_key(&inner.to_lowercase()));
        }
        false
    }

    /// The archetype lookup by (type, name) - the registered archetype index, or a
    /// fresh EMPTY (unregistered) archetype for an unknown name (the chain
    /// guarantees the token was a keyword/archetype check hit; the keyword
    /// itself is NOT registered, so `person …` with no archetype gets an
    /// empty throwaway exactly like the official parser).
    pub(super) fn get_archetype(&mut self, kind: &str, name: &str) -> usize {
        let mut name = name.to_string();
        if kind == tok::RELATIONSHIP
            && name.len() >= 4
            && name.starts_with(tok::RELATIONSHIP_ARCHETYPE_PREFIX)
            && name.ends_with(tok::RELATIONSHIP_ARCHETYPE_SUFFIX)
        {
            name = name[2..name.len() - 2].to_string();
        }
        let key = name.to_lowercase();
        if let Some(&idx) = self.archetypes.get(kind).and_then(|m| m.get(&key)) {
            return idx;
        }
        self.archetype_list.push(Archetype::new(Some(&name), kind));
        self.archetype_list.len() - 1
    }

    /// `addArchetype` — register the (context's) archetype.
    pub(super) fn add_archetype(&mut self, idx: usize) {
        let a = &self.archetype_list[idx];
        let (kind, name) = (a.kind.clone(), a.name.clone());
        self.archetypes.entry(kind).or_default().insert(name, idx);
    }

    /// `extendArchetype`: copy metadata/description/technology/tags from
    /// the (possibly freshly created, empty) parent archetype.
    pub(super) fn extend_archetype(&mut self, idx: usize, archetype_name: &str) {
        let kind = self.archetype_list[idx].kind.clone();
        let parent = self.get_archetype(&kind, archetype_name);
        if parent == idx {
            return; // extending itself: copying its own values is a no-op
        }
        let p = self.archetype_list[parent].clone();
        let a = &mut self.archetype_list[idx];
        a.metadata = p.metadata;
        a.description = p.description;
        a.technology = p.technology;
        for t in p.tags {
            // Parent tags are already trimmed; this is only the add's
            // deduplication into the child ordered set.
            Self::add_archetype_tag(a, &t);
        }
    }

    /// One value of the archetype tag add: the value is
    /// TRIMMED and added to the ordered set — an empty trimmed value IS
    /// added (the add is unconditional);
    /// duplicates are ignored.
    pub(super) fn add_archetype_tag(a: &mut Archetype, tag: &str) {
        let tag = tag.trim();
        if !a.tags.iter().any(|t| t == tag) {
            a.tags.push(tag.to_string());
        }
    }

    /// The `tags <tags> [tags]` statement: the comma-split tags applied per
    /// token — the official split DROPS trailing empty strings; leading/embedded
    /// empties are kept and reach `addTags` (they land as empty tags).
    pub(super) fn add_archetype_tags(a: &mut Archetype, tags: &str) {
        for t in split_keeping_lead_dropping_trail(tags) {
            Self::add_archetype_tag(a, t);
        }
    }
}
