//! `generate system-landscape` (§P4-M3 O-4, P4-D9): the port of the
//! reference `SystemLandscapeGenerator` (pinned monorepo `ad34036`,
//! `structurizr-core/src/main/java/com/structurizr/util/SystemLandscapeGenerator.java`),
//! oracle-witnessed 2026-11-21 (`$OXIDZR_STRUCTURIZR_BIN generate
//! system-landscape`). App-layer ONLY (Locked list: no view generation in
//! parser/library) — and the witness settled the open P4-D9 question: the
//! reference adds NO views, so neither do we.
//!
//! Witnessed contract (TZ=UTC, reference 2026.10.26):
//! * output workspace: name `"System Landscape"`, no description,
//!   `configuration.scope = Landscape`, `id` 0, no views;
//! * pass 1 clones people then software systems per workspace, merging by
//!   NAME: description and url are set UNCONDITIONALLY per pass (the LAST
//!   workspace wins, null included — witnessed: alpha's Alice url erased
//!   by beta); group is first-wins; stored tags union in add order;
//!   properties merge; perspectives merge by name;
//! * pass 2 re-derives the person/system-to-person/system relationships per
//!   workspace (`First`: one edge per source→destination pair; `All`: one
//!   per distinct description), technology/url/tags/properties ride the
//!   clone; then the FIRST container-bearing system of the workspace gets
//!   the `{workspace:<id>}/diagrams` hyperlink (file-loaded workspaces
//!   carry id 0; a JSON workspace with `id: 7` carries 7);
//! * zero workspaces is the reference `IllegalArgumentException`
//!   "Two or more workspaces must be provided" — ported verbatim, and note
//!   ONE workspace is legal there despite the wording.
//!
//! The `findElement` "Person or software system with name … not found."
//! warning path is ported too: pass 1 clones every person and system, so
//! it is UNREACHABLE through the public entry (mirroring the reference,
//! where it is likewise dead code); the reference warns per missing
//! endpoint, source first, and keeps looking.

use oxidizr_core::Id;
use oxidizr_core::configuration::{Workspace, WorkspaceConfiguration, WorkspaceScope};
use oxidizr_core::model::{ModelItem, Person, Relationship, SoftwareSystem};
use oxidizr_core::properties::Properties;
use oxidizr_core::tags::Tags;

/// The default `--include` filter (ported from the reference
/// `GenerateSystemLandscapeSubcommand`: logged as
/// `- include: .*workspace.(json|dsl)`), a full-path full match.
pub const DEFAULT_INCLUDE: &str = r".*workspace.(json|dsl)";

/// Which duplicate source→destination edges survive the merge
/// (`--relationships`, the reference `RelationshipsStrategy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RelationshipsStrategy {
    /// Keep the FIRST edge per (source, destination) pair.
    #[default]
    First,
    /// Keep one edge per distinct (source, destination, description).
    All,
}

/// The merged landscape plus the reference `log.warn` lines the port would
/// emit (empty through the public entry — see module docs).
#[derive(Debug, Clone)]
pub struct Landscape {
    /// The derived workspace (`oxidizr_json::to_vec` writes it like every
    /// other command output).
    pub workspace: Workspace,
    /// `Person or software system with name "…" not found.` lines in
    /// reference log order.
    pub warnings: Vec<String>,
}

/// Derive the system landscape of 1..N workspaces (reference
/// `SystemLandscapeGenerator.generate`).
#[must_use]
pub fn system_landscape(workspaces: &[Workspace], strategy: RelationshipsStrategy) -> Landscape {
    let mut out = Workspace {
        name: Some("System Landscape".into()),
        configuration: WorkspaceConfiguration {
            scope: Some(WorkspaceScope::Landscape),
            ..WorkspaceConfiguration::default()
        },
        ..Workspace::default()
    };
    let mut warnings = Vec::new();
    // The reference shares one id counter across both passes: pass-1
    // elements take 1..=n, pass-2 relationships continue (witnessed:
    // elements 1..3, relationships 4..5).
    let mut next_id = 1_u32;

    // Pass 1: all people, then all software systems, per workspace.
    for workspace in workspaces {
        for person in &workspace.model.people {
            clone_person(person, &mut out, &mut next_id);
        }
        for system in &workspace.model.software_systems {
            clone_software_system(system, &mut out, &mut next_id);
        }
    }
    // Pass 2: relationships, then the hyperlink, per workspace.
    for workspace in workspaces {
        clone_relationships(workspace, &mut out, strategy, &mut next_id, &mut warnings);
        hyperlink_first_system(workspace, &mut out, &mut warnings);
    }
    Landscape {
        workspace: out,
        warnings,
    }
}

/// The shared id counter (`IdGenerator`): allocate at creation time.
fn alloc(next: &mut u32) -> Id {
    let id = Id::from_num(*next);
    *next += 1;
    id
}

fn clone_person(source: &Person, out: &mut Workspace, next: &mut u32) {
    let index = if let Some(index) = out
        .model
        .people
        .iter()
        .position(|p| p.item.name == source.item.name)
    {
        index
    } else {
        out.model.people.push(Person {
            item: ModelItem::new(alloc(next), source.item.name.clone()),
            group: None,
            relationships: Vec::new(),
        });
        out.model.people.len() - 1
    };
    let target = &mut out.model.people[index];
    // Reference: description is SET every pass (the last workspace wins,
    // null included); the group is first-wins (`StringUtils.isNullOrEmpty`
    // — an empty group counts as unset).
    target.item.description.clone_from(&source.item.description);
    if target.group.as_deref().unwrap_or("").is_empty() {
        target.group.clone_from(&source.group);
    }
    clone_item(&source.item, &mut target.item);
}

fn clone_software_system(source: &SoftwareSystem, out: &mut Workspace, next: &mut u32) {
    let index = if let Some(index) = out
        .model
        .software_systems
        .iter()
        .position(|s| s.item.name == source.item.name)
    {
        index
    } else {
        out.model.software_systems.push(SoftwareSystem {
            item: ModelItem::new(alloc(next), source.item.name.clone()),
            group: None,
            containers: Vec::new(),
            documentation: oxidizr_core::docs::Documentation::default(),
            relationships: Vec::new(),
        });
        out.model.software_systems.len() - 1
    };
    let target = &mut out.model.software_systems[index];
    target.item.description.clone_from(&source.item.description);
    if target.group.as_deref().unwrap_or("").is_empty() {
        target.group.clone_from(&source.group);
    }
    clone_item(&source.item, &mut target.item);
}

/// `clone(ModelItem, ModelItem)`: url unconditionally, stored tags added
/// (`Tags::add` = trim + dedup, first position kept — Java `addTags`),
/// properties merged, perspectives merged by name. The kind-derived
/// default tags never enter the stored set here (same element kind on
/// both sides; the writer re-derives them).
fn clone_item(source: &ModelItem, target: &mut ModelItem) {
    target.url.clone_from(&source.url);
    for tag in source.tags.stored() {
        target.tags.add(tag);
    }
    for (name, value) in source.properties.iter() {
        target.properties.insert(name, value);
    }
    for perspective in &source.perspectives {
        if !target
            .perspectives
            .iter()
            .any(|p| p.name == perspective.name)
        {
            target.perspectives.push(perspective.clone());
        }
    }
}

/// The person/system name of an element id of `source`, or `None` when it
/// is not a person or software system (the reference filters the rest with
/// `instanceof` guards) or does not resolve.
fn landscape_endpoint<'a>(source: &'a Workspace, id: &Id) -> Option<&'a str> {
    source
        .model
        .people
        .iter()
        .find(|p| p.item.id == *id)
        .map(|p| p.item.name.as_str())
        .or_else(|| {
            source
                .model
                .software_systems
                .iter()
                .find(|s| s.item.id == *id)
                .map(|s| s.item.name.as_str())
        })
}

/// `findElement`: the person list is searched BEFORE the system list —
/// a same-named person shadows a system for the hyperlink and endpoint
/// resolution (reference order, ported). The miss warning path is
/// unreachable through the public entry (module docs).
fn find_element(out: &Workspace, name: &str, warnings: &mut Vec<String>) -> Option<Id> {
    if let Some(person) = out.model.people.iter().find(|p| p.item.name == name) {
        return Some(person.item.id.clone());
    }
    if let Some(system) = out
        .model
        .software_systems
        .iter()
        .find(|s| s.item.name == name)
    {
        return Some(system.item.id.clone());
    }
    warnings.push(format!(
        "Person or software system with name \"{name}\" not found."
    ));
    None
}

/// `findAndCloneRelationships` + `cloneRelationship`: person/system
/// endpoints only, guarded by the strategy, the clone carrying
/// description/technology (the reference creation call) then
/// url/tags/properties/perspectives (`clone(ModelItem)`).
fn clone_relationships(
    source: &Workspace,
    out: &mut Workspace,
    strategy: RelationshipsStrategy,
    next: &mut u32,
    warnings: &mut Vec<String>,
) {
    for relationship in &source.model.relationships {
        let (Some(source_name), Some(destination_name)) = (
            landscape_endpoint(source, &relationship.source),
            landscape_endpoint(source, &relationship.destination),
        ) else {
            continue;
        };
        // The reference resolves BOTH endpoints and warns per miss, then
        // bails when either is missing (`source != null && destination !=
        // null` — ported order).
        let source_id = find_element(out, source_name, warnings);
        let destination_id = find_element(out, destination_name, warnings);
        let (Some(source_id), Some(destination_id)) = (source_id, destination_id) else {
            continue;
        };
        let guarded = out.model.relationships.iter().any(|r| {
            r.source == source_id
                && r.destination == destination_id
                && match strategy {
                    // `hasEfferentRelationshipWith(destination)` — any
                    // description.
                    RelationshipsStrategy::First => true,
                    // `hasEfferentRelationshipWith(destination,
                    // description)` — same description only.
                    RelationshipsStrategy::All => r.description == relationship.description,
                }
        });
        if guarded {
            continue;
        }
        let mut tags = Tags::new();
        for tag in relationship.tags.stored() {
            tags.add(tag);
        }
        let mut properties = Properties::new();
        for (name, value) in relationship.properties.iter() {
            properties.insert(name, value);
        }
        let id = alloc(next);
        out.model.relationships.push(Relationship {
            id: id.clone(),
            source: source_id.clone(),
            destination: destination_id,
            description: relationship.description.clone(),
            technology: relationship.technology.clone(),
            tags,
            url: relationship.url.clone(),
            properties,
            perspectives: relationship.perspectives.clone(),
            interaction_style: None,
            linked_relationship_id: None,
        });
        if let Some(person) = out.model.people.iter_mut().find(|p| p.item.id == source_id) {
            person.relationships.push(id);
        } else if let Some(system) = out
            .model
            .software_systems
            .iter_mut()
            .find(|s| s.item.id == source_id)
        {
            system.relationships.push(id);
        }
    }
}

/// `hyperlinkSoftwareSystem`: the FIRST container-bearing system of the
/// source workspace gets `{workspace:<id>}/diagrams` — via `findElement`
/// again, so a same-named person in the landscape receives it (ported
/// reference quirk). The url is SET unconditionally (it overwrites a
/// cloned one; the witness confirms per-workspace order).
fn hyperlink_first_system(source: &Workspace, out: &mut Workspace, warnings: &mut Vec<String>) {
    let Some(system) = source
        .model
        .software_systems
        .iter()
        .find(|s| !s.containers.is_empty())
    else {
        return;
    };
    let url = format!("{{workspace:{}}}/diagrams", source.id);
    if let Some(person) = out
        .model
        .people
        .iter_mut()
        .find(|p| p.item.name == system.item.name)
    {
        person.item.url = Some(url);
        return;
    }
    if let Some(target) = out
        .model
        .software_systems
        .iter_mut()
        .find(|s| s.item.name == system.item.name)
    {
        target.item.url = Some(url);
        return;
    }
    warnings.push(format!(
        "Person or software system with name \"{}\" not found.",
        system.item.name
    ));
}

// ---------------------------------------------------------------------------
// Path filters (the `--include`/`--exclude` legs)
// ---------------------------------------------------------------------------

/// The std-only full-match pattern engine behind `--include`/`--exclude`
/// (§P4-M3 O-4). The reference compiles the option with
/// `java.util.regex.Pattern` and applies `matcher.matches()` — an
/// IMPLICIT FULL MATCH over the FULL path (probes: `.*alpha.*` matched
/// the directory component; a bare `workspace` matched nothing; the
/// default include only ever passes files named `workspace.json` /
/// `workspace.dsl`). This engine covers the documented subset:
/// literals, `.`, the classes `\d \D \w \W \s \S` (also inside `[...]`),
/// custom `[...]` classes with ranges and `^` negation, the groups `(...)`
/// and `(?:...)`, the alternation `|`, the greedy and lazy quantifiers
/// `* + ?` (`*?` …), and the `^` / `$` anchor assertions (no-ops under the
/// full match, accepted like `Pattern.matches` does). Unsupported
/// constructs — `{m,n}` quantifiers, named/capturing-group side effects,
/// lookaround, `\p{…}`, inline flags — are an OXZ-5010 diagnostic, never
/// a silent widening or narrowing of the user's pattern.
pub mod pathfilter {
    /// A compiled pattern; [`PathFilter::is_match`] is the full match.
    #[derive(Debug, Clone)]
    pub struct PathFilter(Node);

    impl PathFilter {
        /// Compile, or hand back the reason the pattern is outside the
        /// supported subset (the caller decorates it into OXZ-5010).
        ///
        /// # Errors
        ///
        /// Returns the human reason when `pattern` uses a construct
        /// outside the documented subset (`{m,n}` quantifiers,
        /// lookaround, `\p{…}`, named groups, inline flags, dangling
        /// meta characters, unbalanced groups/classes).
        pub fn compile(pattern: &str) -> Result<PathFilter, String> {
            let mut parser = Parser {
                chars: pattern.chars().collect(),
                pos: 0,
                depth: 0,
            };
            let node = parser.parse_alt()?;
            if parser.pos != parser.chars.len() {
                return Err(format!("unexpected `{}`", parser.chars[parser.pos]));
            }
            Ok(PathFilter(node))
        }

        /// `Pattern.matcher(text).matches()` — the whole text must match.
        #[must_use]
        pub fn is_match(&self, text: &str) -> bool {
            let chars: Vec<char> = text.chars().collect();
            self.0.matches(&chars, 0, &|end| end == chars.len())
        }
    }

    #[derive(Debug, Clone)]
    enum Node {
        Char(char),
        /// `.` — any character (the reference does not run in DOTALL;
        /// paths do not carry newlines in practice, kept simple).
        AnyChar,
        Class {
            ranges: Vec<(char, char)>,
            negated: bool,
        },
        Start,
        End,
        Concat(Vec<Node>),
        Alt(Vec<Node>),
        /// `* + ?` quantifiers (`min = 1` is `+`, `infinite = false` is
        /// `?`). The lazy spellings (`*? +? ??`) parse but share the node:
        /// under a BOOLEAN full match greediness chooses which match is
        /// found, never whether one exists.
        Repeat {
            sub: Box<Node>,
            min: u8,
            infinite: bool,
        },
    }

    impl Node {
        /// Continuation-passing backtracking match: `k(end)` decides the
        /// remainder. Patterns here are tiny path filters; the reference's
        /// own backtracking engine shares the worst case, so none is
        /// engineered against.
        fn matches(&self, text: &[char], i: usize, k: &dyn Fn(usize) -> bool) -> bool {
            match self {
                Node::Char(c) => text.get(i) == Some(c) && k(i + 1),
                Node::AnyChar => i < text.len() && k(i + 1),
                Node::Class { ranges, negated } => {
                    text.get(i).is_some_and(|c| {
                        ranges.iter().any(|(a, b)| *a <= *c && *c <= *b) != *negated
                    }) && k(i + 1)
                }
                Node::Start => i == 0 && k(i),
                Node::End => i == text.len() && k(i),
                Node::Alt(alts) => alts.iter().any(|alt| alt.matches(text, i, k)),
                Node::Concat(parts) => Self::concat(parts, 0, text, i, k),
                Node::Repeat { sub, min, infinite } => match (*min, *infinite) {
                    // `*` / `*?` (the lazy spelling shares the node).
                    (0, true) => Self::star(sub, text, i, k),
                    // `+` / `+?`: one sub match, then the star.
                    (1, true) => sub.matches(text, i, &|j| Self::star(sub, text, j, k)),
                    // `?` / `??`.
                    (0, false) => sub.matches(text, i, k) || k(i),
                    // `(1, false)` = `{1,1}`: not constructible here
                    // (`{m,n}` is OXZ-5010); false keeps the match total.
                    _ => false,
                },
            }
        }

        fn concat(
            parts: &[Node],
            index: usize,
            text: &[char],
            i: usize,
            k: &dyn Fn(usize) -> bool,
        ) -> bool {
            match parts.get(index) {
                None => k(i),
                Some(head) => {
                    head.matches(text, i, &|j| Self::concat(parts, index + 1, text, j, k))
                }
            }
        }

        /// The `*` closure: the continuation is tried at every position
        /// the sub-expression can reach by NON-EMPTY iterations AND at the
        /// zero-iteration position `i` (without that leg `.*x` could never
        /// match text starting at `x`). The `j > i` guard rejects empty
        /// iterations (the reference engine guards the same way), so a
        /// sub-expression that can only match empty never loops.
        fn star(sub: &Node, text: &[char], i: usize, k: &dyn Fn(usize) -> bool) -> bool {
            sub.matches(text, i, &|j| (j > i && Self::star(sub, text, j, k)) || k(j)) || k(i)
        }
    }

    struct Parser {
        chars: Vec<char>,
        pos: usize,
        depth: usize,
    }

    /// Group-nesting cap (stack safety for the recursive matcher).
    const MAX_DEPTH: usize = 64;

    impl Parser {
        fn peek(&self) -> Option<char> {
            self.chars.get(self.pos).copied()
        }

        fn parse_alt(&mut self) -> Result<Node, String> {
            let mut branches = vec![self.parse_concat()?];
            while self.peek() == Some('|') {
                self.pos += 1;
                branches.push(self.parse_concat()?);
            }
            Ok(if branches.len() == 1 {
                branches.remove(0)
            } else {
                Node::Alt(branches)
            })
        }

        fn parse_concat(&mut self) -> Result<Node, String> {
            let mut parts = Vec::new();
            while !matches!(self.peek(), None | Some('|' | ')')) {
                parts.push(self.parse_repeat()?);
            }
            Ok(if parts.len() == 1 {
                parts.remove(0)
            } else {
                Node::Concat(parts)
            })
        }

        fn parse_repeat(&mut self) -> Result<Node, String> {
            let mut node = self.parse_atom()?;
            if let Some(c) = self.peek() {
                if matches!(c, '*' | '+' | '?') {
                    self.pos += 1;
                    // The lazy spelling (`*?`, `+?`, `??`) parses; under a
                    // boolean full match greediness selects WHICH match is
                    // found, never whether one exists, so it is accepted
                    // and ignored.
                    if self.peek() == Some('?') {
                        self.pos += 1;
                    }
                    if matches!(self.peek(), Some('*' | '+' | '?')) {
                        return Err(format!("dangling quantifier `{}`", self.chars[self.pos]));
                    }
                    node = Node::Repeat {
                        sub: Box::new(node),
                        min: u8::from(c != '*'),
                        infinite: c != '?',
                    };
                } else if c == '{' {
                    return Err(
                        "unsupported construct `{` (`{m,n}` quantifiers are not supported)".into(),
                    );
                }
            }
            Ok(node)
        }

        fn parse_atom(&mut self) -> Result<Node, String> {
            let Some(c) = self.peek() else {
                return Err("unexpected end of pattern".into());
            };
            self.pos += 1;
            match c {
                '(' => {
                    self.depth += 1;
                    if self.depth > MAX_DEPTH {
                        return Err("pattern nested too deeply".into());
                    }
                    if self.peek() == Some('?') {
                        // Only the non-capturing form is accepted: capture
                        // groups have no semantic use under `matches()`.
                        if self.chars.get(self.pos + 1) != Some(&':') {
                            return Err(
                                "unsupported group `(?…` (only `(?:…)` is supported)".into()
                            );
                        }
                        self.pos += 2;
                    }
                    let inner = self.parse_alt()?;
                    if self.peek() != Some(')') {
                        return Err("missing `)`".into());
                    }
                    self.pos += 1;
                    self.depth -= 1;
                    Ok(inner)
                }
                '[' => self.parse_class(),
                '.' => Ok(Node::AnyChar),
                '^' => Ok(Node::Start),
                '$' => Ok(Node::End),
                '|' | ')' => Err(format!("dangling meta character `{c}`")),
                '*' | '+' | '?' => Err(format!("dangling quantifier `{c}`")),
                '}' => Err(format!("unsupported construct `{c}`")),
                '\\' => self.parse_escape(),
                c => Ok(Node::Char(c)),
            }
        }

        fn parse_escape(&mut self) -> Result<Node, String> {
            let Some(c) = self.peek() else {
                return Err("trailing `\\`".into());
            };
            self.pos += 1;
            if let Some(ranges) = shorthand(c) {
                return Ok(Node::Class {
                    ranges,
                    negated: c.is_uppercase(),
                });
            }
            if c.is_alphabetic() {
                return Err(format!("unsupported escape `\\{c}`"));
            }
            Ok(Node::Char(c))
        }

        /// `[...]`: literals, the shorthands (ONLY as `\d`-style escapes —
        /// `[d]` is the literal letter, matching the reference), ranges,
        /// and `^` negation. A leading `]` is a literal (like the
        /// reference).
        fn parse_class(&mut self) -> Result<Node, String> {
            let negated = self.peek() == Some('^');
            if negated {
                self.pos += 1;
            }
            let mut ranges = Vec::new();
            let mut first = true;
            loop {
                let Some(c) = self.peek() else {
                    return Err("missing `]`".into());
                };
                self.pos += 1;
                if c == ']' && !first {
                    break;
                }
                first = false;
                // An escaped shorthand (`\d` etc.) expands; anything else
                // resolves to one literal character.
                let literal = |c: char| -> Result<char, String> {
                    if c.is_alphabetic() {
                        return Err(format!("unsupported escape `\\{c}`"));
                    }
                    Ok(c)
                };
                let start = if c == '\\' {
                    let Some(e) = self.peek() else {
                        return Err("trailing `\\`".into());
                    };
                    self.pos += 1;
                    match shorthand(e) {
                        Some(sh) => {
                            ranges.extend(sh);
                            continue;
                        }
                        None => literal(e)?,
                    }
                } else {
                    c
                };
                if self.peek() == Some('-')
                    && self.chars.get(self.pos + 1).is_some_and(|&n| n != ']')
                {
                    self.pos += 1;
                    let Some(end) = self.peek() else {
                        return Err("missing `]`".into());
                    };
                    self.pos += 1;
                    let end = if end == '\\' {
                        let Some(e) = self.peek() else {
                            return Err("trailing `\\`".into());
                        };
                        self.pos += 1;
                        if shorthand(e).is_some() {
                            return Err(format!("unsupported range endpoint `\\{e}`"));
                        }
                        literal(e)?
                    } else {
                        end
                    };
                    if end < start {
                        return Err(format!("bad character range `{start}-{end}`"));
                    }
                    ranges.push((start, end));
                } else {
                    ranges.push((start, start));
                }
            }
            Ok(Node::Class { ranges, negated })
        }
    }

    /// The `\d \D \w \W \s \S` shorthands.
    fn shorthand(c: char) -> Option<Vec<(char, char)>> {
        match c {
            'd' | 'D' => Some(vec![('0', '9')]),
            'w' | 'W' => Some(vec![('0', '9'), ('A', 'Z'), ('_', '_'), ('a', 'z')]),
            's' | 'S' => Some(vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')]),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::PathFilter;

        fn ok(pattern: &str, text: &str) -> bool {
            PathFilter::compile(pattern).unwrap().is_match(text)
        }

        /// The witnessed probe matrix (oracle session 2026-11-21): the
        /// reference filters FULL PATHS with an implicit full match.
        #[test]
        fn default_include_behaves_like_the_reference() {
            let include = crate::generate::DEFAULT_INCLUDE;
            assert!(ok(include, "/tmp/gen-ws/./alpha/workspace.dsl"));
            assert!(ok(include, "/x/workspace.json"));
            assert!(!ok(include, "/tmp/nix-shell.Ncoehj/lonely.json"));
            assert!(!ok(include, "workspace"));
            assert!(ok(include, "workspacexjson")); // the `.` is any char
            assert!(!ok(include, "workspaceXxml"));
        }

        #[test]
        fn directory_component_match_as_witnessed() {
            assert!(ok(r".*alpha.*", "/tmp/gen-ws/./alpha/workspace.dsl"));
            assert!(!ok(r".*gamma.*", "/tmp/gen-ws/./alpha/workspace.dsl"));
        }

        #[test]
        fn classes_groups_anchors_quantifiers() {
            assert!(ok("[a-z]+", "abc"));
            assert!(!ok("[a-z]+", "aBc"));
            assert!(ok(r"[^/]+", "workspace"));
            assert!(!ok(r"[^/]+", "a/b"));
            assert!(ok(r"workspace\.dsl", "workspace.dsl"));
            assert!(!ok(r"workspace\.dsl", "workspacexdsl"));
            assert!(ok(r"(?:json|dsl)$", "json"));
            assert!(ok(r".*(?:json|dsl)$", "x.json"));
            // the IMPLICIT full match admits no prefix without `.*`.
            assert!(!ok(r"(?:json|dsl)$", "x.json"));
            assert!(ok(r"^a.*z", "abcz"));
            assert!(ok("a+?b", "aaab"));
            assert!(ok(r"\d+", "123"));
            assert!(!ok(r"\d+", "12a"));
            assert!(ok(r"\w\w\W", "ab!"));
            assert!(ok("", ""));
            assert!(!ok("", "x"));
            assert!(ok(r"a|ab", "ab"));
        }

        #[test]
        fn unsupported_constructs_are_errors_not_silent_widening() {
            for pattern in [
                r"a{2,3}", r"(?=x)", r"(?<n>x)", r"(?i)ab", r"\p{L}", r"a**", r")x",
            ] {
                assert!(
                    PathFilter::compile(pattern).is_err(),
                    "must reject `{pattern}`"
                );
            }
            assert!(PathFilter::compile(r"a\q").is_err());
            assert!(PathFilter::compile("(a").is_err());
            assert!(PathFilter::compile("[a").is_err());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RelationshipsStrategy, system_landscape};
    use oxidizr_core::builder::WorkspaceBuilder;
    use oxidizr_core::configuration::{Workspace, WorkspaceScope};

    /// Two alpha/beta-shaped workspaces built through the builder (the
    /// witness fixture semantics without file IO).
    fn fixtures() -> (Workspace, Workspace) {
        let mut alpha = WorkspaceBuilder::new();
        let alice = alpha
            .add_person("Alice", Some("Alpha Alice".to_string()), Some("Staff"))
            .unwrap();
        let billing = alpha
            .add_software_system(
                "Billing",
                Some("Alpha billing description".to_string()),
                None,
            )
            .unwrap();
        let crm = alpha
            .add_software_system("CRM", Some("Alpha CRM description".to_string()), None)
            .unwrap();
        alpha
            .add_container(&billing, "Billing API", None, None, None)
            .unwrap();
        alpha
            .add_relationship(
                &alice,
                &billing,
                Some("Uses".to_string()),
                Some("HTTP".to_string()),
                Some("CustomRel"),
            )
            .unwrap();
        alpha
            .add_relationship(&alice, &crm, Some("Uses".to_string()), None, None)
            .unwrap();
        let mut beta = WorkspaceBuilder::new();
        let alice = beta
            .add_person("Alice", Some("Beta Alice".to_string()), Some("External"))
            .unwrap();
        let crm = beta
            .add_software_system("CRM", Some("Beta CRM description".to_string()), None)
            .unwrap();
        let warehouse = beta
            .add_software_system("Warehouse", Some("Beta warehouse".to_string()), None)
            .unwrap();
        beta.add_container(&crm, "CRM UI", None, None, None)
            .unwrap();
        beta.add_relationship(&alice, &crm, Some("Uses".to_string()), None, None)
            .unwrap();
        beta.add_relationship(&alice, &crm, Some("Reports on".to_string()), None, None)
            .unwrap();
        beta.add_relationship(
            &crm,
            &warehouse,
            Some("Pushes orders to".to_string()),
            None,
            None,
        )
        .unwrap();
        (alpha.build(), beta.build())
    }

    #[test]
    fn landscape_shape_and_merge_semantics() {
        let (alpha, beta) = fixtures();
        let out = system_landscape(&[alpha, beta], RelationshipsStrategy::First);
        let ws = &out.workspace;
        assert_eq!(ws.name.as_deref(), Some("System Landscape"));
        assert_eq!(ws.configuration.scope, Some(WorkspaceScope::Landscape));
        assert_eq!(ws.id, 0);
        assert!(out.warnings.is_empty(), "no misses: {:?}", out.warnings);
        // Name-first merge: ONE Alice, LAST description wins (null nukes
        // the url — alpha's url is gone; the tags union in add order).
        assert_eq!(ws.model.people.len(), 1);
        let alice = &ws.model.people[0];
        assert_eq!(alice.item.description.as_deref(), Some("Beta Alice"));
        assert_eq!(alice.item.url, None);
        assert_eq!(
            alice
                .item
                .tags
                .serialized_for(oxidizr_core::tags::TagKind::Person),
            "Element,Person,Staff,External"
        );
        assert_eq!(alice.item.id.as_str(), "1");
        // Systems in clone order with pass-1 ids; containers NEVER ride.
        let names: Vec<&str> = ws
            .model
            .software_systems
            .iter()
            .map(|s| s.item.name.as_str())
            .collect();
        assert_eq!(names, ["Billing", "CRM", "Warehouse"]);
        // First strategy: Alice's second "Uses" (beta→CRM) dropped,
        // "Reports on" never considered.
        let rels: Vec<(String, Option<String>)> = ws
            .model
            .relationships
            .iter()
            .map(|r| (r.destination.as_str().to_string(), r.description.clone()))
            .collect();
        assert_eq!(rels.len(), 3, "Uses→Billing, Uses→CRM, Pushes orders to");
        // Hyperlinks: alpha's first container-bearing system is Billing,
        // beta's is CRM — both carry the {workspace:0} url.
        for name in ["Billing", "CRM"] {
            let system = ws
                .model
                .software_systems
                .iter()
                .find(|s| s.item.name == name)
                .unwrap();
            assert_eq!(
                system.item.url.as_deref(),
                Some("{workspace:0}/diagrams"),
                "{name}"
            );
        }
    }

    #[test]
    fn all_strategy_keeps_distinct_descriptions() {
        let (alpha, beta) = fixtures();
        let out = system_landscape(&[alpha, beta], RelationshipsStrategy::All);
        // Same "Uses"→CRM is still dropped; "Reports on" survives.
        let descriptions: Vec<&str> = out
            .workspace
            .model
            .relationships
            .iter()
            .filter(|r| r.description.as_deref() == Some("Reports on"))
            .collect::<Vec<_>>()
            .iter()
            .map(|_| "Reports on")
            .collect();
        assert_eq!(descriptions.len(), 1);
        assert_eq!(out.workspace.model.relationships.len(), 4);
    }
}
