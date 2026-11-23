//! Semantic comparator for the three diagram dialects
//! (`plantuml/c4plantuml`, plain `plantuml`, `mermaid`; Category B, run
//! sparingly via `cargo xtask export-verify --format c4|plantuml|mermaid`).
//!
//! Byte parity pins the exact the official tooling formatting; this comparator proves
//! the weaker, intended-equal property: the two outputs state the SAME facts,
//! ignoring line ORDER — the diagram bodies are order-insensitive except
//! where the dialect itself binds position (those order-stable fixtures stay
//! pinned by the byte corpus). A tolerant line-grammar classifies every line
//! of a file into a fact string; each file becomes a multiset of
//! facts (`BTreeMap<fact, count>`) and the two multisets are diffed. The
//! whitelist is minimal and explicit: comment lines (puml `'`, mermaid
//! `%%`) and blank lines are dropped — everything else becomes a fact, so
//! nothing silently disappears.
//!
//! [`C4_MACROS`] doubles as the C4-PlantUML macro inventory that the
//! importer builds on (union of what the official tooling emits; an unknown
//! `Name(…)` line still parses — it just gets the `macro?` kind tag).

use std::collections::BTreeMap;

/// C4-PlantUML macros the official C4-PlantUML exporter emits
/// (incl. the `Container`/`Component` `Db`/`Queue` shape verbs).
pub const C4_MACROS: &[&str] = &[
    "AddBoundaryTag",
    "AddElementTag",
    "AddProperty",
    "AddRelTag",
    "Boundary",
    "Component",
    "ComponentDb",
    "ComponentQueue",
    "Container",
    "ContainerDb",
    "ContainerQueue",
    "Container_Boundary",
    "Deployment_Node",
    "Person",
    "Rel",
    "SHOW_LEGEND",
    "System",
    "System_Boundary",
    "WithoutPropertyHeader",
];

/// Multiset of classified lines, `fact string => occurrences`.
pub type FactSet = BTreeMap<String, u32>;

/// Classify one `.puml` file into its fact multiset.
pub fn facts(text: &str) -> FactSet {
    let mut out = FactSet::new();
    let mut style: Option<Vec<String>> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('\'') {
            // whitelist: blanks + header comment lines
            continue;
        }
        if let Some(buf) = style.as_mut() {
            if let Some(pos) = line.find("</style>") {
                let inner = &line[..pos];
                if !inner.trim().is_empty() {
                    buf.push(norm(inner));
                }
                let joined = std::mem::take(buf).join(";").replace(' ', "");
                insert(&mut out, format!("style<{joined}>"));
                style = None;
            } else {
                buf.push(norm(line));
            }
            continue;
        }
        if line.starts_with("<style>") {
            let mut buf = Vec::new();
            if let Some(pos) = line.find("</style>") {
                // single-line <style>…</style> (not emitted today; tolerant)
                buf.push(norm(&line[7..pos]));
                insert(
                    &mut out,
                    format!("style<{}>", buf.join(";").replace(' ', "")),
                );
            } else {
                style = Some(buf);
            }
            continue;
        }
        insert(&mut out, classify(line));
    }
    if let Some(mut buf) = style {
        // unterminated <style>: stay tolerant, keep what we saw as raw facts
        for l in buf.drain(..) {
            insert(&mut out, format!("raw {l}"));
        }
    }
    out
}

fn classify(line: &str) -> String {
    for kw in [
        "@startuml",
        "@enduml",
        "!include",
        "!includesub",
        "skinparam",
        "title ",
        "footer ",
        "hide ",
        "show ",
        "legend",
        "endlegend",
        "set ",
        "activate ",
        "deactivate ",
    ] {
        if line.starts_with(kw) {
            return format!("dir {}", norm(line));
        }
    }
    if line.starts_with("left to right direction") || line.starts_with("top to bottom direction") {
        return format!("dir {}", norm(line));
    }
    if let Some(rest) = line.strip_prefix("box ") {
        return format!("box {}", norm(rest));
    }
    // macro call (possibly with a trailing ` {` opening a boundary block)
    if let Some(open) = line.find('(') {
        let name = line[..open].trim_end();
        if !name.is_empty()
            && name.starts_with(char::is_alphabetic)
            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
        {
            let body = &line[open + 1..];
            let (args, brace) = match body.rfind(')') {
                Some(close) => (&body[..close], body[close + 1..].trim() == "{"),
                None => (body, false),
            };
            let kind = if C4_MACROS.contains(&name) {
                "macro"
            } else {
                "macro?"
            };
            let mut fact = format!("{kind} {name}({})", split_args(args).join(", "));
            if brace {
                fact.push_str(" {");
            }
            return fact;
        }
    }
    match line {
        "{" | "}" => format!("struct {line}"),
        _ if line.contains("-->") || line.contains("->") => {
            // bare arrow (sequence legs) — normalized relationship tuple
            format!("arrow {}", norm(line))
        }
        _ => format!("raw {}", norm(line)),
    }
}

/// Classify one plain-plantuml (the official Structurizr dialect) file.
///
/// Verified legs (official tooling vs ours over the
/// `testdata/golden/plantuml` corpus) show this dialect is byte-identical
/// except for relationship-line ORDER (the `bulk-operations` case), so the
/// same tolerant line grammar applies: directives, `<style>` blocks (whose
/// `// tag` comments ride inside the folded fact), element lines
/// (`person "==…" <<Element-…>> as X`), boundary opens
/// (`rectangle "…" <<Group-…>> as groupN {`) and `X --> Y <<…>> : "…"`
/// edges all become facts; the `macro` branch simply never fires because
/// the dialect uses no `Name(…)` call syntax.
pub fn facts_plantuml(text: &str) -> FactSet {
    facts(text)
}

/// Classify one Mermaid (the official Mermaid dialect) file.
/// Verified grammar (same corpus, `.mmd`): `graph <dir>`,
/// `linkStyle default …`, nested `subgraph <id> ["title"]` / `end`, node
/// declarations `N["<div…>"]`, per-id `style N …` lines and edges
/// `N-. "<div>label</div><div …>[tech]</div>" .->M` (every edge labeled;
/// ids are positional and identical on both sides, so they stay in the
/// fact). Nothing else is emitted; an unknown line still becomes a `raw`
/// fact, so nothing disappears.
pub fn facts_mermaid(text: &str) -> FactSet {
    let mut out = FactSet::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with("%%") {
            // whitelist: blanks + mermaid comment lines
            continue;
        }
        insert(&mut out, classify_mermaid(line));
    }
    out
}

fn classify_mermaid(line: &str) -> String {
    if line == "end" {
        return "struct end".to_owned();
    }
    let (kind, payload) = if let Some(rest) = line
        .strip_prefix("graph ")
        .or_else(|| line.strip_prefix("flowchart "))
    {
        ("dir", rest)
    } else if let Some(rest) = line.strip_prefix("subgraph ") {
        ("subgraph", rest)
    } else if let Some(rest) = line.strip_prefix("style ") {
        ("style", rest)
    } else if let Some(rest) = line.strip_prefix("linkStyle ") {
        ("linkstyle", rest)
    } else if line.contains(".->") || line.contains("-->") || line.contains("==>") {
        ("edge", line)
    } else if line.contains("[\"") {
        ("node", line)
    } else {
        return format!("raw {}", norm(line));
    };
    format!("{kind} {}", norm(payload))
}

/// Split a macro argument list on top-level commas (quotes + nested parens
/// respected).
fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0_i32;
    let mut quoted = false;
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                out.push(norm(&cur));
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(norm(&cur));
    }
    out
}

/// Collapse whitespace runs to single spaces (both writers use identical
/// spelling otherwise; the byte corpus pins any deeper formatting).
fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn insert(set: &mut FactSet, fact: String) {
    *set.entry(fact).or_insert(0) += 1;
}

/// Diff two per-file fact sets; empty result = semantically equal.
pub fn diff_facts(reference: &FactSet, ours: &FactSet) -> Vec<String> {
    let mut diffs = Vec::new();
    for (fact, n) in reference {
        match ours.get(fact) {
            None => diffs.push(format!("missing in ours: {fact} ×{n}")),
            Some(m) if m < n => diffs.push(format!("missing in ours: {fact} ×{}", n - m)),
            Some(_) => {}
        }
    }
    for (fact, n) in ours {
        match reference.get(fact) {
            None => diffs.push(format!("extra in ours: {fact} ×{n}")),
            Some(m) if m < n => diffs.push(format!("extra in ours: {fact} ×{}", n - m)),
            Some(_) => {}
        }
    }
    diffs
}

#[cfg(test)]
mod tests {
    use super::{classify, diff_facts, facts, facts_mermaid, facts_plantuml};

    #[test]
    fn reordering_is_semantically_equal() {
        let a = "@startuml\nPerson(a, \"A\", $descr=\"\", $tags=\"\", $link=\"\")\nPerson(b, \"B\", $descr=\"\", $tags=\"\", $link=\"\")\nRel(a, b, \"uses\", $techn=\"\", $tags=\"\", $link=\"\")\nSHOW_LEGEND()\n@enduml\n";
        let b = "@startuml\nPerson(b, \"B\", $descr=\"\", $tags=\"\", $link=\"\")\nRel(a, b, \"uses\", $techn=\"\", $tags=\"\", $link=\"\")\nPerson(a, \"A\", $descr=\"\", $tags=\"\", $link=\"\")\nSHOW_LEGEND()\n@enduml\n";
        assert_eq!(diff_facts(&facts(a), &facts(b)), Vec::<String>::new());
    }

    #[test]
    fn content_changes_are_caught() {
        let a = "Person(a, \"A\", $descr=\"\", $tags=\"\", $link=\"\")\n";
        let b = "Person(a, \"A2\", $descr=\"\", $tags=\"\", $link=\"\")\n";
        assert_eq!(diff_facts(&facts(a), &facts(b)).len(), 2);
    }

    #[test]
    fn args_split_respects_quotes_and_parens() {
        let fact = classify("Rel(a, b, \"x, y (z)\", $tags=\"A,B\")");
        assert_eq!(fact, "macro Rel(a, b, \"x, y (z)\", $tags=\"A,B\")");
    }

    #[test]
    fn boundary_macro_with_brace_and_style_block() {
        let f = facts(
            "Enterprise_Boundary(\"e\", \"E\", $tags=\"\") {\n<style>\n  root {\n    BackgroundColor: #ffffff\n  }\n</style>\n}\n",
        );
        let names: Vec<&String> = f.keys().collect();
        assert!(
            names.contains(&&"macro? Enterprise_Boundary(\"e\", \"E\", $tags=\"\") {".to_string())
        );
        assert!(
            names
                .iter()
                .any(|k| k.starts_with("style<") && k.contains("#ffffff"))
        );
        assert!(names.contains(&&"struct }".to_string()));
    }

    // The two newer dialects, verified-shape samples.

    const PLAIN_A: &str = "@startuml\nset separator none\n<style>\n  // Element,Person\n  .Element-abc= {\n    BackgroundColor: #08427b;\n  }\n</style>\nperson \"==A\" <<Element-abc=>> as A\nrectangle \"G\" <<Group-Z>> as group1 {\n  person \"==B\" <<Element-abc=>> as B\n}\nA --> B <<Relationship-x>> : \"Uses\"\nB --> A <<Relationship-x>> : \"Back\"\n@enduml\n";

    #[test]
    fn plain_plantuml_reordering_is_equal_labels_are_pinned() {
        let e1 = "A --> B <<Relationship-x>> : \"Uses\"";
        let e2 = "B --> A <<Relationship-x>> : \"Back\"";
        let swapped = PLAIN_A.replace(e1, "@@").replace(e2, e1).replace("@@", e2);
        assert_eq!(
            diff_facts(&facts_plantuml(PLAIN_A), &facts_plantuml(&swapped)),
            Vec::<String>::new()
        );
        let relabeled = PLAIN_A.replace("\"Uses\"", "\"Use\"");
        assert_eq!(
            diff_facts(&facts_plantuml(PLAIN_A), &facts_plantuml(&relabeled)).len(),
            2
        );
    }

    #[test]
    fn plain_plantuml_style_tag_comments_are_facts() {
        // The `// tag` comment lives INSIDE the folded style fact: retagging
        // changes the fact (it is never silently dropped like body `'`).
        let retagged = PLAIN_A.replace("// Element,Person", "// Element,Bot");
        assert_eq!(
            diff_facts(&facts_plantuml(PLAIN_A), &facts_plantuml(&retagged)).len(),
            2
        );
    }

    const MERMAID_A: &str = "graph LR\n  linkStyle default fill:#ffffff\n\n  subgraph diagram [\"V\"]\n    style diagram fill:#ffffff\n\n    subgraph group1 [\"G\"]\n      2[\"<div>B</div>\"]\n      style 2 fill:#999999,stroke:#6b6b6b\n      3[\"<div>C</div>\"]\n      style 3 fill:#1168bd\n    end\n\n    1[\"<div>A</div>\"]\n    style 1 fill:#08427b\n    1-. \"<div>Uses</div><div style='font-size: 70%'></div>\" .->2\n    2-. \"<div>Uses</div><div style='font-size: 70%'>[T]</div>\" .->3\n\n  end\n";

    #[test]
    fn mermaid_reordering_is_equal() {
        // swap the two edge lines (the verified `bulk-operations` drift)
        let e1 = "1-. \"<div>Uses</div><div style='font-size: 70%'></div>\" .->2";
        let e2 = "2-. \"<div>Uses</div><div style='font-size: 70%'>[T]</div>\" .->3";
        let swapped = MERMAID_A
            .replace(e1, "@@")
            .replace(e2, e1)
            .replace("@@", e2);
        assert_eq!(
            diff_facts(&facts_mermaid(MERMAID_A), &facts_mermaid(&swapped)),
            Vec::<String>::new()
        );
    }

    #[test]
    fn mermaid_content_changes_are_caught() {
        let restyled = MERMAID_A.replace("style 1 fill:#08427b", "style 1 fill:#08427c");
        assert_eq!(
            diff_facts(&facts_mermaid(MERMAID_A), &facts_mermaid(&restyled)).len(),
            2
        );
        let retitled = MERMAID_A.replace("subgraph group1 [\"G\"]", "subgraph group1 [\"G2\"]");
        assert_eq!(
            diff_facts(&facts_mermaid(MERMAID_A), &facts_mermaid(&retitled)).len(),
            2
        );
    }
}
