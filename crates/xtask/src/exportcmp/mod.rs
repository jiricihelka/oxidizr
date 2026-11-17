//! P2-D9 semantic comparator for `plantuml/c4plantuml` output (Category B,
//! run sparingly via `cargo xtask export-verify`).
//!
//! Byte parity pins the exact reference formatting; this comparator proves
//! the weaker, intended-equal property: the two outputs state the SAME facts,
//! ignoring line ORDER — the C4 diagram body is order-insensitive except
//! where `PlantUML` itself binds position (those order-stable fixtures stay
//! pinned by the byte corpus). A tolerant line-grammar classifies every line
//! of a `.puml` file into a fact string; each file becomes a multiset of
//! facts (`BTreeMap<fact, count>`) and the two multisets are diffed. The
//! whitelist is minimal and explicit: comment lines (leading `'`) and blank
//! lines are dropped — everything else becomes a fact, so nothing silently
//! disappears.
//!
//! [`C4_MACROS`] doubles as the C4-PlantUML macro inventory the P4 importer
//! builds on (union of what the pinned reference emits; an unknown
//! `Name(…)` line still parses — it just gets the `macro?` kind tag).

use std::collections::BTreeMap;

/// C4-PlantUML macros the pinned reference `C4PlantUMLExporter` emits
/// (`ad34036`, incl. the `Container`/`Component` `Db`/`Queue` shape verbs).
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
    use super::{classify, diff_facts, facts};

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
}
