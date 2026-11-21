//! `textDocument/hover` (§P4-LSP L3, item L-5).
//!
//! Two legs, one pure entry point ([`hover`]):
//!
//! - **Model hover** over the LAST SUCCESSFUL parse of the document: the
//!   token under the cursor (a quoted string, a registered identifier, or
//!   a name) is matched against the model and rendered as a card — kind,
//!   reference canonical name ([`oxidizr_core::canonical`]), technology,
//!   stored tags, relationship counts. A buffer mid-typing still hovers:
//!   the server keeps the last good [`Workspace`] per document
//!   (`server::Server::last_good`) — a failed reparse NEVER replaces it.
//! - **Keyword hover**: the embedded table below — one line per DSL
//!   statement keyword and directive, so `model` or `!identifiers` explain
//!   themselves even in a file that has never parsed.
//!
//! Model wins over keyword (an identifier named `model` is information,
//! the keyword tooltip is not).

use std::fmt::Write;

use lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position, Range};
use oxidizr_core::canonical;
use oxidizr_core::model::{DeploymentNode, ModelItem};
use oxidizr_core::{DEFAULT_ENVIRONMENT, Workspace};

use crate::completion::position_to_offset;

/// One hoverable element flattened for lookup and rendering.
struct ElementRef<'a> {
    /// Human kind label ("Person", "Container", "Deployment node", …).
    kind: &'static str,
    /// The reference canonical name (`Container://S.C`).
    canonical: String,
    /// Shared fields (name, description, tags).
    item: &'a ModelItem,
    /// Technology when the kind carries one.
    technology: Option<&'a str>,
}

/// The keyword/directive documentation table: exact token → one-liner.
/// Keys are the tokens `oxidizr-dsl` dispatches on; the texts are our
/// editor-facing prose (the reference ships no hover docs).
const KEYWORD_DOCS: &[(&str, &str)] = &[
    (
        "workspace",
        "Top level: metadata, `model`, `views`, configuration.",
    ),
    (
        "extends",
        "workspace extends <file.json> — layer the file over a JSON base.",
    ),
    (
        "enterprise",
        "enterprise \"name\" \"id\" — workspace ownership metadata.",
    ),
    ("name", "name \"…\" — the workspace (or element) name."),
    ("description", "description \"…\" — free text."),
    ("url", "url \"…\" — a link shown in views and docs."),
    (
        "properties",
        "properties { key \"value\" } — free-form properties.",
    ),
    ("metadata", "metadata \"…\" — custom element kind metadata."),
    (
        "model",
        "The static model: elements, relationships, deployment, archetypes.",
    ),
    (
        "archetypes",
        "archetypes { … } — named metadata bundles applied with `has`.",
    ),
    (
        "group",
        "group \"Name\" { … } — a view-grouped run of sibling elements.",
    ),
    (
        "element",
        "element \"name\" \"kind\" — a custom element type.",
    ),
    ("person", "person \"name\" — a human user or role."),
    (
        "softwareSystem",
        "softwareSystem \"name\" — a system in scope.",
    ),
    (
        "container",
        "container \"name\" \"technology\" — inside a software system; also the container view.",
    ),
    (
        "component",
        "component \"name\" \"technology\" — inside a container; also the component view.",
    ),
    (
        "relationship",
        "Archetype definitions for relationships (model scope).",
    ),
    ("tags", "tags \"A,B\" — stored (custom) tags."),
    (
        "technology",
        "technology \"…\" — container/component/node technology.",
    ),
    (
        "deploymentEnvironment",
        "deploymentEnvironment \"name\" — a deployment scope.",
    ),
    (
        "deploymentGroup",
        "deploymentGroup \"name\" — groups deployment nodes in deployment views.",
    ),
    (
        "deploymentNode",
        "deploymentNode \"name\" — infrastructure hosting instances.",
    ),
    (
        "infrastructureNode",
        "infrastructureNode \"name\" — middleware/database on a node.",
    ),
    (
        "softwareSystemInstance",
        "softwareSystemInstance \"system\" — a deployed system.",
    ),
    (
        "containerInstance",
        "containerInstance \"system:container\" — a deployed container.",
    ),
    (
        "instanceOf",
        "instanceOf <id> — deploy an existing element.",
    ),
    (
        "healthCheck",
        "healthCheck { … } — URL/interval for a deployed instance.",
    ),
    (
        "instances",
        "instances \"N\" — the deployment element count.",
    ),
    (
        "scope",
        "scope hierarchical|flat — how child identifiers nest.",
    ),
    (
        "styles",
        "styles { … } — element/relationship styling rules.",
    ),
    (
        "systemLandscape",
        "systemLandscape \"key\" \"description\" — all systems.",
    ),
    (
        "systemContext",
        "systemContext <system> \"key\" \"description\" — a system's context.",
    ),
    (
        "dynamic",
        "dynamic { … } — an ordered flow over elements/relationships.",
    ),
    (
        "deployment",
        "deployment <system> \"key\" — a deployment view.",
    ),
    (
        "filtered",
        "filtered <baseViewKey> \"includes\" \"excludes\" \"key\" — a saved filter.",
    ),
    ("image", "image \"…\" — a pre-generated PNG/SVG view."),
    (
        "custom",
        "custom \"key\" \"description\" — a view of no fixed type.",
    ),
    (
        "include",
        "include \"selector\" (views) / !include <file> (top of a line).",
    ),
    ("exclude", "exclude \"selector\" inside filtered views."),
    (
        "animation",
        "animation { animationStep … } — a step-through of a view.",
    ),
    (
        "autolayout",
        "autolayout [rankDirection …] — engine layout settings for a view.",
    ),
    ("title", "title \"…\" — the view title."),
    (
        "perspectives",
        "perspectives { perspective … } — JSON-consumed annotations.",
    ),
    (
        "!include",
        "!include <file|directory> — splice other DSL sources.",
    ),
    (
        "!docs",
        "!docs <directory> — load Markdown/ADR documentation.",
    ),
    (
        "!adrs",
        "!adrs <directory> — load architecture decision records.",
    ),
    ("!decisions", "!decisions <directory> — alias of !adrs."),
    ("!constant", "!constant NAME value — a `${NAME}` constant."),
    ("!const", "!const NAME value — alias of !constant."),
    ("!var", "!var NAME value — a `${NAME}` variable."),
    (
        "!identifiers",
        "!identifiers hierarchical|flat — identifier scoping for later statements.",
    ),
    (
        "!impliedRelationships",
        "!impliedRelationships true|false — implied relationships for later elements.",
    ),
    ("!ref", "!ref <file|url> — a live external model reference."),
    (
        "!extend",
        "!extend <file|url> — extend with elements/relationships.",
    ),
    ("!element", "!element <selector> — find one element."),
    ("!elements", "!elements <selector> — find elements."),
    (
        "!relationship",
        "!relationship <selector> — find one relationship.",
    ),
    (
        "!relationships",
        "!relationships <selector> — find relationships.",
    ),
    (
        "!components",
        "!components <id> — auto-add a container's components to a view.",
    ),
];

/// The token under the cursor: the quoted string CONTAINING it, else the
/// word run touching it (a leading `!` joins so directives match the doc
/// table). `None` at whitespace/delimiters.
fn token_at(text: &str, offset: usize) -> Option<((usize, usize), Token<'_>)> {
    let offset = offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let tail = &text[line_start..];
    let line_end = line_start + tail.find('\n').unwrap_or(tail.len());

    // Quoted-string leg: pair the unescaped `"` from the line start; the
    // cursor offset (a position BETWEEN chars) must fall STRICTLY inside
    // a pair — or past an opening quote the line never closes
    // (mid-typing).
    let mut quotes: Vec<usize> = Vec::new();
    let mut escaped = false;
    let mut at = line_start;
    while at < line_end {
        let ch = text[at..].chars().next()?;
        match ch {
            '\\' => escaped = !escaped,
            '"' => {
                if escaped {
                    escaped = false; // this quote was escaped — content
                } else {
                    quotes.push(at);
                }
            }
            _ => escaped = false,
        }
        at += ch.len_utf8();
    }
    for pair in quotes.chunks(2) {
        let open = pair[0];
        match pair.get(1) {
            Some(&close) => {
                if offset > open && offset < close {
                    return Some(((open, close + 1), Token::Quoted(&text[open + 1..close])));
                }
            }
            None => {
                if offset > open {
                    return Some(((open, line_end), Token::Quoted(&text[open + 1..line_end])));
                }
            }
        }
    }

    // Word leg: [A-Za-z0-9_] around the cursor, `!` prefix included.
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut start = offset;
    while start > line_start {
        let prev = text[line_start..start].chars().next_back()?;
        if is_word(prev) {
            start -= prev.len_utf8();
        } else {
            break;
        }
    }
    // `ends_with` on the whole prefix: slicing `text[start - 1..]` could
    // land mid-character after a multi-byte neighbour and panic.
    if start > line_start && text[line_start..start].ends_with('!') {
        start -= 1;
    }
    let mut end = offset;
    while end < line_end {
        let next = text[end..line_end].chars().next()?;
        if !is_word(next) {
            break;
        }
        end += next.len_utf8();
    }
    if start == end {
        return None;
    }
    Some(((start, end), Token::Word(&text[start..end])))
}

enum Token<'a> {
    /// A bare word (keyword, identifier, unquoted name fragment).
    Word(&'a str),
    /// The CONTENTS of the quoted string containing the cursor.
    Quoted(&'a str),
}

/// The hover for `text` at `position`. `last_good` is the document's last
/// SUCCESSFUL parse (`None` before the first one). `None` ⇒ the wire's
/// null hover result.
#[must_use]
pub fn hover(text: &str, position: Position, last_good: Option<&Workspace>) -> Option<Hover> {
    let offset = position_to_offset(text, position);
    let ((start, end), token) = token_at(text, offset)?;
    let value = match &token {
        Token::Quoted(name) => last_good.and_then(|ws| model_card(ws, name)),
        Token::Word(word) => last_good
            .and_then(|ws| model_card(ws, word))
            .or_else(|| keyword_card(word)),
    }?;
    Some(Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value,
        }),
        // The `position` PARAMETER shadows the byte→LSP mapper — the
        // range ends go through the module path.
        range: Some(Range::new(
            crate::diagnostics::position(text, start),
            crate::diagnostics::position(text, end),
        )),
    })
}

/// The keyword/directive tooltip (exact token match; the DSL dispatches
/// keywords case-sensitively, directives case-insensitively — the table
/// mirrors the canonical spellings, so directives compare case-free).
fn keyword_card(word: &str) -> Option<String> {
    let bang = word.starts_with('!');
    KEYWORD_DOCS.iter().find_map(|(token, doc)| {
        let hit = if bang {
            token.eq_ignore_ascii_case(word)
        } else {
            *token == word
        };
        hit.then(|| format!("`{word}` — {doc}"))
    })
}

/// The model card for an identifier or name, or `None` when the workspace
/// knows neither. Identifier match (the `structurizr.dsl.identifier`
/// property the parser records) wins over the name match.
fn model_card(ws: &Workspace, needle: &str) -> Option<String> {
    let elements = elements(ws);
    let id_hit = elements.iter().find(|e| {
        e.item
            .properties
            .get("structurizr.dsl.identifier")
            .is_some_and(|id| id == needle)
    });
    let e = id_hit.or_else(|| elements.iter().find(|e| e.item.name == needle))?;
    let id = &e.item.id;
    let out = ws
        .model
        .relationships
        .iter()
        .filter(|r| &r.source == id)
        .count();
    let inbound = ws
        .model
        .relationships
        .iter()
        .filter(|r| &r.destination == id)
        .count();
    let mut md = format!("**{}** — `{}`\n", e.kind, e.canonical);
    if let Some(description) = &e.item.description {
        let _ = writeln!(md, "\n{description}");
    }
    if let Some(technology) = e.technology {
        let _ = writeln!(md, "\n- Technology: {technology}");
    }
    if !e.item.tags.stored().is_empty() {
        let _ = writeln!(md, "- Tags: {}", e.item.tags.stored().join(", "));
    }
    let _ = writeln!(md, "- Relationships: {out} out, {inbound} in");
    Some(md)
}

/// Every element, reference `getElements` order (custom, people,
/// systems/containers/components, then the deployment trees).
fn elements(ws: &Workspace) -> Vec<ElementRef<'_>> {
    let m = &ws.model;
    let mut out = Vec::new();
    out.extend(m.custom_elements.iter().map(|e| ElementRef {
        kind: "Custom element",
        canonical: canonical::custom_element(&e.item.name),
        item: &e.item,
        technology: None,
    }));
    out.extend(m.people.iter().map(|p| ElementRef {
        kind: "Person",
        canonical: canonical::person(&p.item.name),
        item: &p.item,
        technology: None,
    }));
    for s in &m.software_systems {
        out.push(ElementRef {
            kind: "Software system",
            canonical: canonical::software_system(&s.item.name),
            item: &s.item,
            technology: None,
        });
        for c in &s.containers {
            out.push(ElementRef {
                kind: "Container",
                canonical: canonical::container(&s.item.name, &c.item.name),
                item: &c.item,
                technology: c.technology.as_deref(),
            });
            out.extend(c.components.iter().map(|k| ElementRef {
                kind: "Component",
                canonical: canonical::component(&s.item.name, &c.item.name, &k.item.name),
                item: &k.item,
                technology: k.technology.as_deref(),
            }));
        }
    }
    for n in &m.deployment_nodes {
        walk_node(ws, n, &mut Vec::new(), &mut out);
    }
    out
}

/// Deployment recursion; `chain` holds the ANCESTOR node names (root
/// first), exactly the reference canonical-name chain. `ws` resolves the
/// instances' model references (`software_system_id`/`container_id`) to
/// the names the reference canonicals are built from.
fn walk_node<'a>(
    ws: &'a Workspace,
    n: &'a DeploymentNode,
    chain: &mut Vec<&'a str>,
    out: &mut Vec<ElementRef<'a>>,
) {
    let env = if n.environment.is_empty() {
        DEFAULT_ENVIRONMENT
    } else {
        &n.environment
    };
    out.push(ElementRef {
        kind: "Deployment node",
        canonical: canonical::deployment_node(env, chain, &n.item.name),
        item: &n.item,
        technology: n.technology.as_deref(),
    });
    chain.push(&n.item.name);
    let node_chain = canonical::deployment_node_chain(env, chain);
    for c in &n.children {
        walk_node(ws, c, chain, out);
    }
    out.extend(n.infrastructure_nodes.iter().map(|i| ElementRef {
        kind: "Infrastructure node",
        canonical: canonical::infrastructure_node(&node_chain, &i.item.name),
        item: &i.item,
        technology: i.technology.as_deref(),
    }));
    out.extend(n.software_system_instances.iter().map(|si| {
        let system = ws
            .model
            .software_systems
            .iter()
            .find(|s| s.item.id == si.software_system_id)
            .map_or(si.item.name.as_str(), |s| s.item.name.as_str());
        ElementRef {
            kind: "Software system instance",
            canonical: canonical::software_system_instance(&node_chain, system, si.instance_id),
            item: &si.item,
            technology: None,
        }
    }));
    out.extend(n.container_instances.iter().map(|ci| {
        let (system, container) = ws
            .model
            .software_systems
            .iter()
            .find_map(|s| {
                s.containers
                    .iter()
                    .find(|c| c.item.id == ci.container_id)
                    .map(|c| (s.item.name.as_str(), c.item.name.as_str()))
            })
            .unwrap_or((ci.item.name.as_str(), ci.item.name.as_str()));
        ElementRef {
            kind: "Container instance",
            canonical: canonical::container_instance(
                &node_chain,
                system,
                container,
                ci.instance_id,
            ),
            item: &ci.item,
            technology: None,
        }
    }));
    chain.pop();
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::diagnostics::position;
    use oxidizr_api::{ApiOptions, Input};

    const SOURCE: &str = "workspace {\n  model {\n    p = person \"P\" {\n      tags \"Team\"\n    }\n    s = softwareSystem \"S\"\n    p -> s \"Uses\" {\n      technology \"HTTPS\"\n    }\n  }\n}\n";

    fn workspace_of(text: &str) -> Workspace {
        oxidizr_api::open(
            &Input::Bytes(text.as_bytes().to_vec()),
            &ApiOptions::default(),
        )
        .unwrap()
        .workspace
    }

    /// Hover the text `needle` (its first occurrence, middle offset).
    fn hover_at(text: &str, needle: &str, ws: Option<&Workspace>) -> Option<Hover> {
        let at = text.find(needle).unwrap() + needle.len() / 2;
        hover(text, position(text, at), ws)
    }

    fn value(h: Option<Hover>) -> String {
        match h.unwrap().contents {
            HoverContents::Markup(m) => m.value,
            other => panic!("markdown expected, got {other:?}"),
        }
    }

    #[test]
    fn keyword_leg_needs_no_parse() {
        let md = value(hover_at(SOURCE, "softwareSystem", None));
        assert!(md.contains("softwareSystem"), "{md}");
        assert!(md.contains("system in scope"), "{md}");
        // A word that is neither keyword nor model hovers nothing.
        assert!(hover_at(SOURCE, "Uses", None).is_none());
    }

    #[test]
    fn directive_leg_is_case_insensitive() {
        let text = "workspace {\n  !IDENTIFIERS hierarchical\n}\n";
        let md = value(hover_at(text, "!IDENTIFIERS", None));
        assert!(
            md.contains("!identifiers") || md.contains("!IDENTIFIERS"),
            "{md}"
        );
        assert!(md.contains("hierarchical"), "{md}");
    }

    #[test]
    fn quoted_name_hits_the_model() {
        let ws = workspace_of(SOURCE);
        let md = value(hover_at(SOURCE, "\"P\"", Some(&ws)));
        assert!(md.contains("**Person**"), "{md}");
        assert!(md.contains("`Person://P`"), "{md}");
        assert!(md.contains("Tags: Team"), "{md}");
        assert!(md.contains("1 out, 0 in"), "{md}");
    }

    #[test]
    fn identifier_wins_over_name_and_keyword() {
        let ws = workspace_of(SOURCE);
        // `p` is a registered identifier, not the word `person`: the
        // cursor sits right after `p` (the needle's middle lands there).
        let md = value(hover_at(SOURCE, "p =", Some(&ws)));
        assert!(md.contains("**Person** — `Person://P`"), "{md}");
    }

    #[test]
    fn nested_and_deployed_canonicals() {
        // DSL positional strings are name + DESCRIPTION; deployment trees
        // need a `deploymentEnvironment` wrapper (the model scope admits
        // `deploymentEnvironment`, never a bare `deploymentNode` — OXZ-2001
        // list); `infrastructureNode` is positional-only
        // `<name> [description] [technology]` (OXZ-2002 list — opens NO
        // block); and the qualified `s.c` identifier form needs
        // `!identifiers hierarchical` (corpus: deployment-groups-hierarchical.dsl).
        let source = "workspace {\n  !identifiers hierarchical\n  model {\n    s = softwareSystem \"S\" {\n      c = container \"C\" {\n        technology \"Java\"\n        k = component \"K\" {\n          technology \"Spring\"\n        }\n      }\n    }\n    deploymentEnvironment \"Default\" {\n      deploymentNode \"AWS\" {\n        deploymentNode \"EC2\" {\n          i = containerInstance s.c\n          infra = infrastructureNode \"ALB\" \"Balancer\" \"L7\"\n        }\n      }\n    }\n  }\n}\n";
        let ws = workspace_of(source);
        let md = value(hover_at(source, "\"K\"", Some(&ws)));
        assert!(md.contains("`Component://S.C.K`"), "{md}");
        assert!(md.contains("Technology: Spring"), "{md}");
        let md = value(hover_at(source, "\"EC2\"", Some(&ws)));
        assert!(md.contains("`DeploymentNode://Default/AWS/EC2`"), "{md}");
        let md = value(hover_at(source, "\"ALB\"", Some(&ws)));
        assert!(
            md.contains("`InfrastructureNode://Default/AWS/EC2/ALB`"),
            "{md}"
        );
    }

    #[test]
    fn broken_buffer_hovers_the_last_good_parse() {
        // The server hands the PREVIOUS good workspace in; the token may
        // even be new text (here `"P"` is retyped, the parse is broken).
        let ws = workspace_of(SOURCE);
        let broken = "workspace {\n  model {\n    p = person \"P\" ~~~\n  }\n}\n";
        let md = value(hover_at(broken, "\"P\"", Some(&ws)));
        assert!(md.contains("**Person**"), "{md}");
    }

    #[test]
    fn edge_offsets_never_panic() {
        let text = "workspace {\n  name \"Pří💔š\" \n}\n";
        for at in 0..=text.len() + 10 {
            let _ = hover(text, position(text, at.min(text.len())), None);
        }
        assert!(hover("", Position::new(0, 0), None).is_none());
    }
}
