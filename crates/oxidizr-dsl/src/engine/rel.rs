//! Ports of `ExplicitRelationshipParser`, `ImplicitRelationshipParser` and
//! `NoRelationshipParser` (`ad34036`). Sets are insertion-ordered `Vec`s —
//! the portable stand-in for the reference `HashSet`/`LinkedHashSet`
//! (see `plan/PROGRESS.md`); the creation loops keep the Java
//! source×destination nesting order.

use oxidizr_core::{Id, validate::ElementKind};

use super::ctx::Ctx;
use super::error::DslError;
use super::{Parser, Tokens, eqi, java_split_str, tok};

/// Optional positional text slots of a relationship statement
/// (`description`, `technology`, `tags`) as parsed by `positional`.
type PosTexts = (Option<String>, Option<String>, Option<String>);

/// The archetype snapshot a relationship statement reads (`Archetype` is
/// parsed-side parser state; cloning out of `archetype_list` keeps the
/// borrow checker away from the mutation trail).
pub(crate) struct RelArch {
    pub description: String,
    pub technology: String,
    /// `CustomElementArchetype` metadata (empty for the other kinds).
    pub metadata: String,
    pub tags: Vec<String>,
    pub properties: Vec<(String, String)>,
    pub perspectives: Vec<oxidizr_core::model::Perspective>,
}

impl Parser<'_> {
    /// Snapshot of `archetype_list[idx]` for the parser ports.
    pub(crate) fn arch_snapshot(&self, idx: usize) -> RelArch {
        let a = &self.archetype_list[idx];
        RelArch {
            description: a.description.clone(),
            technology: a.technology.clone(),
            metadata: a.metadata.clone(),
            tags: a.tags.clone(),
            properties: a.properties.clone(),
            perspectives: a.perspectives.clone(),
        }
    }
}

/// `createRelationship` + the archetype property/perspective side effects.
/// The returned id carries no `linkedRelationshipId` — explicit DSL
/// relationships are never linked.
impl Parser<'_> {
    pub(crate) fn create_rel(
        &mut self,
        source: &Id,
        destination: &Id,
        description: Option<String>,
        technology: Option<String>,
        tags: &[String],
        arch: &RelArch,
    ) -> Result<Id, DslError> {
        let id = self
            .builder
            .dsl_uses(source, destination, description, technology)
            .map_err(|e| DslError::core(&e))?;
        // `createRelationship` passes the tags to `uses(...)`; the core
        // `dsl_uses` creates the relationship with an empty tag set, the
        // archetype + statement tags land right after (addTags semantics).
        self.builder.add_tag_parts(&id, tags);
        for (name, value) in &arch.properties {
            let _ = self.builder.add_item_property(&id, name, value);
        }
        for p in &arch.perspectives {
            // `relationship.addPerspectives(...)` delegates to the 3-arg
            // `addPerspective`, which rejects blank names first
            // (`Perspective.setName`), then the duplicate guard.
            if p.name.trim().is_empty() {
                return Err(DslError::core(
                    &oxidizr_core::CoreError::PerspectiveNameRequired,
                ));
            }
            self.builder
                .add_perspective(&id, p.clone())
                .map_err(|e| DslError::core(&e))?;
        }
        Ok(id)
    }

    /// Positional token extraction shared by the two parsers; every parser
    /// validates "too many"/"expected" BEFORE reading any token, exactly
    /// like the reference.
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unused_self)] // method symmetry with the other guards
    fn positional(
        &self,
        tokens: &Tokens,
        grammar: &str,
        required: usize,
        desc_ix: usize,
        tech_ix: Option<usize>,
        tags_ix: usize,
    ) -> Result<PosTexts, DslError> {
        if tokens.has_more_than(tags_ix) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {grammar}"
            )));
        }
        if !tokens.includes(required) {
            return Err(DslError::stmt(format!("Expected: {grammar}")));
        }
        let description = tokens.includes(desc_ix).then(|| tokens.get(desc_ix));
        let technology = tech_ix
            .filter(|&i| tokens.includes(i))
            .map(|i| tokens.get(i));
        let tags = tokens.includes(tags_ix).then(|| tokens.get(tags_ix));
        Ok((description, technology, tags))
    }

    /// `ExplicitRelationshipParser.findElement` — `context.getElement` plus
    /// the `this` fallback, active ONLY when the top context is an element
    /// context and no identifier named `this` resolves.
    fn explicit_endpoint(&mut self, identifier: &str) -> Option<Id> {
        if let Some(super::idents::RegItem::Element(id)) = self.get_element(identifier) {
            return Some(id);
        }
        if eqi(identifier, tok::THIS)
            && let Some(id) = self.top().and_then(Ctx::item_id)
        {
            return Some(id.clone());
        }
        None
    }

    /// The environment / deployment-node context's environment name
    /// (`ExplicitRelationshipParser.parse`).
    fn rel_environment(&self) -> Option<String> {
        match self.top()? {
            Ctx::Environment { env, .. } => Some(env.name.clone()),
            Ctx::NoRelInEnv { environment, .. } => Some(environment.clone()),
            Ctx::DeploymentNode { id, .. } => self.builder.element_environment(id),
            _ => None,
        }
    }

    /// `AbstractRelationshipParser.findSoftwareSystemInstances` /
    /// `findContainerInstances` — the static-structure fan-out used by the
    /// explicit (env/node contexts) and implicit (infrastructure-node
    /// source) parsers. Non-instances pass through unchanged.
    fn fan_out(&self, element: &Id, env: &str) -> Vec<Id> {
        match self.builder.element_kind(element) {
            Some(ElementKind::SoftwareSystem) => {
                self.builder.find_software_system_instances(element, env)
            }
            Some(ElementKind::Container) => self.builder.find_container_instances(element, env),
            _ => vec![element.clone()],
        }
    }

    /// `ExplicitRelationshipParser.parse(DslContext, …)` — the Model /
    /// environment / element-context arm.
    pub(crate) fn explicit_rel(
        &mut self,
        tokens: &Tokens,
        arch_idx: usize,
    ) -> Result<Vec<Id>, DslError> {
        const GRAMMAR: &str = "<identifier> -> <identifier> [description] [technology] [tags]";
        let (desc, tech, tags) = self.positional(tokens, GRAMMAR, 2, 3, Some(4), 5)?;

        let source_id = tokens.get(0);
        let source = self
            .explicit_endpoint(&source_id)
            .ok_or_else(|| missing_endpoint("source", &source_id))?;
        let destination_id = tokens.get(2);
        let destination = self
            .explicit_endpoint(&destination_id)
            .ok_or_else(|| missing_endpoint("destination", &destination_id))?;

        // Archetype defaults; inside a `NoRelationshipInDeploymentEnvironment`
        // context the LINKED relationship's values take over when the
        // statement omits them (the `DslContext` variant only — the
        // `ElementsDslContext` variant has no such fallback).
        let a = &self.archetype_list[arch_idx];
        let (has_desc, has_tech) = (desc.is_some(), tech.is_some());
        let mut description = desc.or_else(|| Some(a.description.clone()));
        let mut technology = tech.or_else(|| Some(a.technology.clone()));
        if let Some(Ctx::NoRelInEnv { linked, .. }) = self.top() {
            let linked = linked.clone();
            if let Some((ld, lt, _)) = self.builder.relationship_parts(&linked) {
                if !has_desc {
                    description = Some(ld);
                }
                if !has_tech {
                    technology = Some(lt);
                }
            }
        }
        let arch = self.arch_snapshot(arch_idx);
        let mut tags_v: Vec<String> = arch.tags.clone();
        if let Some(csv) = &tags {
            tags_v.extend(java_split_str(csv, ",").into_iter().map(str::to_string));
        }

        if let Some(env) = self.rel_environment() {
            let sources: Vec<Id> = self.fan_out(&source, &env);
            let destinations: Vec<Id> = self.fan_out(&destination, &env);
            let mut out = Vec::new();
            for s in &sources {
                for d in &destinations {
                    out.push(self.create_rel(
                        s,
                        d,
                        description.clone(),
                        technology.clone(),
                        &tags_v,
                        &arch,
                    )?);
                }
            }
            Ok(out)
        } else {
            Ok(vec![self.create_rel(
                &source,
                &destination,
                description,
                technology,
                &tags_v,
                &arch,
            )?])
        }
    }

    /// `ExplicitRelationshipParser.parse(ElementsDslContext, …)` — the
    /// fan-out variant (`this` = every context element; tags from the
    /// token REPLACE the archetype tags).
    pub(crate) fn explicit_rel_elements(
        &mut self,
        tokens: &Tokens,
        arch_idx: usize,
    ) -> Result<Vec<Id>, DslError> {
        const GRAMMAR: &str = "<identifier> -> <identifier> [description] [technology] [tags]";
        let (desc, tech, tags) = self.positional(tokens, GRAMMAR, 2, 3, Some(4), 5)?;
        let ctx_elements = match self.top() {
            Some(Ctx::Elements { items }) => items.clone(),
            _ => Vec::new(),
        };

        // `findElements`: a registered identifier wins; otherwise `this`
        // fans out to the context elements; otherwise the EMPTY set (the
        // caller throws with source/destination wording).
        let find = |p: &mut Parser<'_>, id: &str| -> Vec<Id> {
            match p.get_element(id) {
                Some(super::idents::RegItem::Element(e)) => vec![e],
                _ if eqi(id, tok::THIS) => ctx_elements.clone(),
                _ => Vec::new(),
            }
        };

        let source_id = tokens.get(0);
        let sources = find(self, &source_id);
        if sources.is_empty() {
            return Err(missing_endpoint("source", &source_id));
        }
        let destination_id = tokens.get(2);
        let destinations = find(self, &destination_id);
        if destinations.is_empty() {
            return Err(missing_endpoint("destination", &destination_id));
        }

        let a = &self.archetype_list[arch_idx];
        let description = desc.or_else(|| Some(a.description.clone()));
        let technology = tech.or_else(|| Some(a.technology.clone()));
        let arch = self.arch_snapshot(arch_idx);
        // Elements variant: a tag token REPLACES the archetype tags.
        let tags_v: Vec<String> = match &tags {
            Some(csv) => java_split_str(csv, ",")
                .into_iter()
                .map(str::to_string)
                .collect(),
            None => arch.tags.clone(),
        };

        let mut out = Vec::new();
        for s in &sources {
            for d in &destinations {
                out.push(self.create_rel(
                    s,
                    d,
                    description.clone(),
                    technology.clone(),
                    &tags_v,
                    &arch,
                )?);
            }
        }
        Ok(out)
    }

    /// `ImplicitRelationshipParser.parse(ElementDslContext, …)` — the
    /// source is the TOP context's element; only an infrastructure-node
    /// source fans the destination out to environment instances.
    pub(crate) fn implicit_rel(
        &mut self,
        tokens: &Tokens,
        arch_idx: usize,
    ) -> Result<Vec<Id>, DslError> {
        const GRAMMAR: &str = "-> <identifier> [description] [technology] [tags]";
        let (desc, tech, tags) = self.positional(tokens, GRAMMAR, 1, 2, Some(3), 4)?;

        let source = self
            .top()
            .and_then(Ctx::item_id)
            .cloned()
            .ok_or_else(|| DslError::stmt("Expected com.structurizr.dsl.ElementDslContext"))?;
        let destination_id = tokens.get(1);
        let Some(super::idents::RegItem::Element(destination)) = self.get_element(&destination_id)
        else {
            return Err(DslError::stmt(format!(
                "The destination element \"{destination_id}\" does not exist"
            )));
        };

        let a = &self.archetype_list[arch_idx];
        let description = desc.or_else(|| Some(a.description.clone()));
        let technology = tech.or_else(|| Some(a.technology.clone()));
        let arch = self.arch_snapshot(arch_idx);
        let mut tags_v: Vec<String> = arch.tags.clone();
        if let Some(csv) = &tags {
            tags_v.extend(java_split_str(csv, ",").into_iter().map(str::to_string));
        }

        if matches!(self.top(), Some(Ctx::InfrastructureNode { .. })) {
            let env = self
                .builder
                .element_environment(&source)
                .unwrap_or_default();
            let mut out = Vec::new();
            for d in self.fan_out(&destination, &env) {
                out.push(self.create_rel(
                    &source,
                    &d,
                    description.clone(),
                    technology.clone(),
                    &tags_v,
                    &arch,
                )?);
            }
            return Ok(out);
        }
        Ok(vec![self.create_rel(
            &source,
            &destination,
            description,
            technology,
            &tags_v,
            &arch,
        )?])
    }

    /// `ImplicitRelationshipParser.parse(ElementsDslContext, …)` — every
    /// context element becomes a source (the chain arm reads the archetype
    /// from `tokens.get(1)` — a reference quirk replicated at the call
    /// site; tags from a token REPLACE the archetype tags).
    pub(crate) fn implicit_rel_elements(
        &mut self,
        tokens: &Tokens,
        arch_idx: usize,
    ) -> Result<Vec<Id>, DslError> {
        const GRAMMAR: &str = "-> <identifier> [description] [technology] [tags]";
        let (desc, tech, tags) = self.positional(tokens, GRAMMAR, 1, 2, Some(3), 4)?;

        let sources = match self.top() {
            Some(Ctx::Elements { items }) => items.clone(),
            _ => Vec::new(),
        };
        let destination_id = tokens.get(1);
        let Some(super::idents::RegItem::Element(destination)) = self.get_element(&destination_id)
        else {
            return Err(DslError::stmt(format!(
                "The destination element \"{destination_id}\" does not exist"
            )));
        };

        let a = &self.archetype_list[arch_idx];
        let description = desc.or_else(|| Some(a.description.clone()));
        let technology = tech.or_else(|| Some(a.technology.clone()));
        let arch = self.arch_snapshot(arch_idx);
        let tags_v: Vec<String> = match &tags {
            Some(csv) => java_split_str(csv, ",")
                .into_iter()
                .map(str::to_string)
                .collect(),
            None => arch.tags.clone(),
        };

        let mut out = Vec::new();
        for s in &sources {
            out.push(self.create_rel(
                s,
                &destination,
                description.clone(),
                technology.clone(),
                &tags_v,
                &arch,
            )?);
        }
        Ok(out)
    }

    /// `NoRelationshipParser.parse` — removes the LINKED instance
    /// relationships between the two endpoints in the environment and
    /// returns them (first removal first).
    ///
    /// Erratum: the reference `do { … } while (relationship != null)`
    /// spins forever when the first found match is NOT linked
    /// (`linkedRelationshipId == null`); this port removes every linked
    /// match and stops — the closest terminating behavior (recorded in
    /// `plan/DSL-ERRATA.md`).
    pub(crate) fn no_relationship(&mut self, tokens: &Tokens) -> Result<Vec<Id>, DslError> {
        const GRAMMAR: &str = "<identifier> -/> <identifier> [description]";
        if tokens.has_more_than(3) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {GRAMMAR}"
            )));
        }
        if !tokens.includes(2) {
            return Err(DslError::stmt(format!(
                "Not enough tokens, expected: {GRAMMAR}"
            )));
        }
        let env = self.rel_environment().ok_or_else(|| {
            DslError::stmt("Expected com.structurizr.dsl.DeploymentEnvironmentDslContext")
        })?;

        let source_id = tokens.get(0);
        let Some(super::idents::RegItem::Element(source)) = self.get_element(&source_id) else {
            return Err(DslError::stmt(format!(
                "The source element \"{source_id}\" does not exist"
            )));
        };
        let sources = self.no_rel_endpoints(&source, &source_id, "source", &env)?;
        let destination_id = tokens.get(2);
        let Some(super::idents::RegItem::Element(destination)) = self.get_element(&destination_id)
        else {
            return Err(DslError::stmt(format!(
                "The destination element \"{destination_id}\" does not exist"
            )));
        };
        let destinations =
            self.no_rel_endpoints(&destination, &destination_id, "destination", &env)?;

        let description = tokens.includes(3).then(|| tokens.get(3));
        let mut removed: Vec<Id> = Vec::new();
        let mut count = 0usize;
        for s in &sources {
            for d in &destinations {
                loop {
                    let found = match &description {
                        Some(desc) => self.builder.efferent_to_with_description(s, d, desc),
                        None => self.builder.efferent_to(s, d),
                    };
                    let Some(rel) = found else { break };
                    let Some(linked) = self.builder.linked_relationship_id(&rel) else {
                        break; // un-linked match: the Java do-while hangs here
                    };
                    let _ = linked;
                    self.builder.remove_relationship(&rel);
                    removed.push(rel.clone());
                    count += 1;
                }
            }
        }
        if count == 0 {
            return Err(match &description {
                Some(desc) => DslError::stmt(format!(
                    "A relationship between \"{source_id}\" and \"{destination_id}\" with description \"{desc}\" does not exist"
                )),
                None => DslError::stmt(format!(
                    "A relationship between \"{source_id}\" and \"{destination_id}\" does not exist"
                )),
            });
        }
        Ok(removed)
    }

    /// `NoRelationshipParser` endpoint resolution: a software system /
    /// container fans out to its environment instances; a static-structure
    /// instance is itself; anything else is the reference's "not valid"
    /// throw. `what` selects the message's source/destination word.
    fn no_rel_endpoints(
        &self,
        element: &Id,
        token_id: &str,
        what: &str,
        env: &str,
    ) -> Result<Vec<Id>, DslError> {
        match self.builder.element_kind(element) {
            Some(ElementKind::SoftwareSystem) => {
                Ok(self.builder.find_software_system_instances(element, env))
            }
            Some(ElementKind::Container) => Ok(self.builder.find_container_instances(element, env)),
            Some(ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance) => {
                Ok(vec![element.clone()])
            }
            _ => Err(DslError::stmt(format!(
                "The {what} element \"{token_id}\" is not valid - expecting a software system, software system instance, container, or container instance"
            ))),
        }
    }
}

/// `The source element "x" does not exist` / `The destination element "x"
/// does not exist` — the exact wording of the two relationship parsers.
fn missing_endpoint(what: &str, identifier: &str) -> DslError {
    DslError::stmt(format!(
        "The {what} element \"{identifier}\" does not exist"
    ))
}
