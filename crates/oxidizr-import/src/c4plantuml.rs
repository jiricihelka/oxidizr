//! The C4-PlantUML model importer — one of the agreed additional features;
//! the official tooling has NO model-parser counterpart. Scope: the
//! macro signatures verified
//! against the official plantuml-stdlib/C4-PlantUML includes.
//!
//! Import semantics oxidizr commits to (warning-and-skip, never
//! fail-all):
//! - ONE macro call per logical line (lines join while a `(` is open);
//!   `$key = value` keyword arguments override their positional slot;
//! - the element macros `Person(_Ext)?`, `System(_Ext|Db|Queue(
//!   (_Db|_Queue)(_Ext)?)?)?` (+ the legacy `_Db`/`_Queue` spellings),
//!   `Container(_Ext|Db|Queue|Db_Ext|Queue_Ext)?`, `Component(…)?` —
//!   `_Ext` adds the `External` tag, `Db`/`Queue` the `Database`/`Queue`
//!   tags (the C4 shapes expressed as Structurizr tags — semantic bar,
//!   not pixel bar);
//! - `System_Boundary` creates a `SoftwareSystem` and scopes the
//!   containers declared inside it, `Container_Boundary` a `Container`
//!   scoping components; `Enterprise_Boundary`/`Person_Boundary`/
//!   `Boundary`/`*_Box` are visual-only pass-through scopes (a failed
//!   boundary degrades to pass-through so its still-valid children
//!   import);
//! - `Rel` + every direction-suffixed variant is one directed
//!   relationship (suffixes are layout hints), `Rel_Back` reverses the
//!   pair, `BiRel` mints both directions;
//! - diagram chrome (`!include`/preprocessor, `skinparam`, `LAYOUT_*`,
//!   `Update*`/`Add*Tag`, `Lay_*`, legend, `title`) is tolerated; the
//!   first `title` line becomes the view name; raw `PlantUML` arrow lines
//!   and unknown macros warn and are skipped.
//!
//! The output is the MODEL ONLY — no view. Verified against reality:
//! the official element-add guard permits ONLY `CustomElement`s, and
//! every scoped view kind’s own
//! guard rejects a mixed C4 set (a container view forbids its scope
//! system, landscape/context forbid containers/components) — no view
//! kind may host the declared subset. Imported workspaces are opened
//! through `api::open`, whose `createDefaultViews` installs the native
//! views at load; the fidelity bar is the semantic model (element/
//! relationship sets), which is what import produces.
//! Standard Structurizr JSON the official tooling loads — the official tooling
//! read gate checks exactly that at milestone close.

use std::collections::HashMap;

use oxidizr_core::{Id, WorkspaceBuilder};

use crate::macrocall::{Call, Preprocessor, join_logical_lines, strip_comments};
use crate::{ImportError, Imported};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Person,
    System,
    Container,
    Component,
}

#[derive(Debug, Clone, Copy)]
struct ElementSpec {
    kind: Kind,
    /// Tags the macro spelling adds beyond the user's `$tags`
    /// (`_Ext` ⇒ External, `Db` ⇒ Database, `Queue` ⇒ Queue).
    extra: &'static [&'static str],
}

const EXT: &[&str] = &["External"];
const DB: &[&str] = &["Database"];
const QUEUE: &[&str] = &["Queue"];
const DB_EXT: &[&str] = &["Database", "External"];
const QUEUE_EXT: &[&str] = &["Queue", "External"];

/// The element macro table (arity verified from the official includes).
const ELEMENTS: &[(&str, ElementSpec)] = &[
    (
        "Person",
        ElementSpec {
            kind: Kind::Person,
            extra: &[],
        },
    ),
    (
        "Person_Ext",
        ElementSpec {
            kind: Kind::Person,
            extra: EXT,
        },
    ),
    (
        "System",
        ElementSpec {
            kind: Kind::System,
            extra: &[],
        },
    ),
    (
        "System_Ext",
        ElementSpec {
            kind: Kind::System,
            extra: EXT,
        },
    ),
    (
        "SystemDb",
        ElementSpec {
            kind: Kind::System,
            extra: DB,
        },
    ),
    (
        "System_Db",
        ElementSpec {
            kind: Kind::System,
            extra: DB,
        },
    ),
    (
        "SystemQueue",
        ElementSpec {
            kind: Kind::System,
            extra: QUEUE,
        },
    ),
    (
        "System_Queue",
        ElementSpec {
            kind: Kind::System,
            extra: QUEUE,
        },
    ),
    (
        "SystemDb_Ext",
        ElementSpec {
            kind: Kind::System,
            extra: DB_EXT,
        },
    ),
    (
        "System_Db_Ext",
        ElementSpec {
            kind: Kind::System,
            extra: DB_EXT,
        },
    ),
    (
        "SystemQueue_Ext",
        ElementSpec {
            kind: Kind::System,
            extra: QUEUE_EXT,
        },
    ),
    (
        "System_Queue_Ext",
        ElementSpec {
            kind: Kind::System,
            extra: QUEUE_EXT,
        },
    ),
    (
        "Container",
        ElementSpec {
            kind: Kind::Container,
            extra: &[],
        },
    ),
    (
        "Container_Ext",
        ElementSpec {
            kind: Kind::Container,
            extra: EXT,
        },
    ),
    (
        "ContainerDb",
        ElementSpec {
            kind: Kind::Container,
            extra: DB,
        },
    ),
    (
        "Container_Db",
        ElementSpec {
            kind: Kind::Container,
            extra: DB,
        },
    ),
    (
        "ContainerQueue",
        ElementSpec {
            kind: Kind::Container,
            extra: QUEUE,
        },
    ),
    (
        "Container_Queue",
        ElementSpec {
            kind: Kind::Container,
            extra: QUEUE,
        },
    ),
    (
        "ContainerDb_Ext",
        ElementSpec {
            kind: Kind::Container,
            extra: DB_EXT,
        },
    ),
    (
        "Container_Db_Ext",
        ElementSpec {
            kind: Kind::Container,
            extra: DB_EXT,
        },
    ),
    (
        "ContainerQueue_Ext",
        ElementSpec {
            kind: Kind::Container,
            extra: QUEUE_EXT,
        },
    ),
    (
        "Container_Queue_Ext",
        ElementSpec {
            kind: Kind::Container,
            extra: QUEUE_EXT,
        },
    ),
    (
        "Component",
        ElementSpec {
            kind: Kind::Component,
            extra: &[],
        },
    ),
    (
        "Component_Ext",
        ElementSpec {
            kind: Kind::Component,
            extra: EXT,
        },
    ),
    (
        "ComponentDb",
        ElementSpec {
            kind: Kind::Component,
            extra: DB,
        },
    ),
    (
        "Component_Db",
        ElementSpec {
            kind: Kind::Component,
            extra: DB,
        },
    ),
    (
        "ComponentQueue",
        ElementSpec {
            kind: Kind::Component,
            extra: QUEUE,
        },
    ),
    (
        "Component_Queue",
        ElementSpec {
            kind: Kind::Component,
            extra: QUEUE,
        },
    ),
    (
        "ComponentDb_Ext",
        ElementSpec {
            kind: Kind::Component,
            extra: DB_EXT,
        },
    ),
    (
        "Component_Db_Ext",
        ElementSpec {
            kind: Kind::Component,
            extra: DB_EXT,
        },
    ),
    (
        "ComponentQueue_Ext",
        ElementSpec {
            kind: Kind::Component,
            extra: QUEUE_EXT,
        },
    ),
    (
        "Component_Queue_Ext",
        ElementSpec {
            kind: Kind::Component,
            extra: QUEUE_EXT,
        },
    ),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    /// Creates the scope's model element (its containers/components
    /// declared inside become children).
    System,
    Container,
    /// Visual-only grouping: `Enterprise_Boundary`, `Person_Boundary`,
    /// generic `Boundary` and the `*_Box` macros.
    Pass,
}

fn boundary_of(name: &str) -> Option<Boundary> {
    match name {
        "System_Boundary" => Some(Boundary::System),
        // `Container_Boundary_Ext` is the Mermaid spelling of an
        // external container scope (the `_Ext` name adds the tag).
        "Container_Boundary" | "Container_Boundary_Ext" => Some(Boundary::Container),
        "Enterprise_Boundary"
        | "Person_Boundary"
        | "Boundary"
        | "Enterprise_Box"
        | "System_Box"
        | "Container_Box"
        | "Person_Box" => Some(Boundary::Pass),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelKind {
    Forward,
    /// `Rel_Back`: the pair is drawn backwards ⇒ the model relationship
    /// runs destination → source.
    Backward,
    /// `BiRel`: two directed relationships, one per direction.
    Bidirectional,
}

fn rel_kind(name: &str) -> Option<RelKind> {
    if name == "BiRel" || name.strip_prefix("BiRel_").is_some_and(|s| !s.is_empty()) {
        return Some(RelKind::Bidirectional);
    }
    if name == "Rel_Back"
        || name
            .strip_prefix("Rel_Back_")
            .is_some_and(|s| !s.is_empty())
    {
        return Some(RelKind::Backward);
    }
    if name == "Rel" || name.strip_prefix("Rel_").is_some_and(|s| !s.is_empty()) {
        return Some(RelKind::Forward);
    }
    // Legacy scoped spellings: SystemRel/SystemBiRel/ContainerRel/…
    // (`System_Rel` included via the trailing-underscore trim).
    for (suffix, kind) in [("BiRel", RelKind::Bidirectional), ("Rel", RelKind::Forward)] {
        if name.strip_suffix(suffix).is_some_and(|p| {
            matches!(
                p.trim_end_matches('_'),
                "System" | "Container" | "Component"
            )
        }) {
            return Some(kind);
        }
    }
    None
}

/// Macro-name prefixes that style/lay out rather than model
/// (`LAYOUT_WITH_LEGEND`, `UpdateRelStyle`, `AddElementTag`, `Lay_Out`,
/// `SET_*`, `SHOW_*`/`HIDE_*`, `Remove*`, the Mermaid `Set*`/`Show*`/
/// `Hide*`/`CLEAR_*`/`Enter_Boundary`/`Leave_Boundary` chrome) —
/// tolerated silently.
const TOLERATED: &[&str] = &[
    "LAYOUT",
    "SHOW_",
    "HIDE_",
    "Lay_",
    "SET_",
    "SHOW",
    "SET",
    "Update",
    "Add",
    "Remove",
    "Set",
    "Show",
    "Hide",
    "CLEAR",
    "Enter_Boundary",
    "Leave_Boundary",
];

/// The two macro languages sharing this machinery: identical
/// macro names and scope semantics, different positional slots for the
/// optional `$tags`/`$descr` — Mermaid spells `Person` as
/// `(alias, label, descr, ?tags)`, `Container`/`Component` as
/// `(alias, label, techn, descr, ?tags)` and `Rel` as
/// `(from, to, label, techn, ?tags)`: its slot 4 is `$tags` where
/// C4-`PlantUML` has `$descr`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    PlantUml,
    Mermaid,
}

impl Dialect {
    #[must_use]
    const fn name(self) -> &'static str {
        match self {
            Self::PlantUml => "PlantUML",
            Self::Mermaid => "Mermaid",
        }
    }

    const fn tags_person(self) -> usize {
        match self {
            Self::PlantUml => 4,
            Self::Mermaid => 3,
        }
    }

    const fn tags_container(self) -> usize {
        match self {
            Self::PlantUml => 5,
            Self::Mermaid => 4,
        }
    }

    /// `usize::MAX` reads keyword-only: Mermaid has no positional
    /// `$descr` (its `Rel` slot 4 is `$tags`).
    const fn rel_descr(self) -> usize {
        match self {
            Self::PlantUml => 4,
            Self::Mermaid => usize::MAX,
        }
    }

    const fn rel_tags(self) -> usize {
        match self {
            Self::PlantUml => 6,
            Self::Mermaid => 4,
        }
    }
}

#[derive(Clone)]
enum Scope {
    System(Id),
    Container(Id),
    Pass,
}

/// Import one C4-PlantUML diagram into a fresh workspace (module docs
/// for the full contract). File I/O stays upstream (the CLI arc).
///
/// # Errors
///
/// [`ImportError::Core`] is never produced today (model construction
/// failures are warnings by contract, never fail-all); the signature
/// mirrors the `image` leg for the shared CLI path.
pub fn import(text: &str) -> Result<Imported, ImportError> {
    import_dialect(&strip_comments(text), Dialect::PlantUml)
}

/// The shared macro-diagram pipeline (comment-stripped text in, model
/// out) — [`crate::mermaid`] reuses it with [`Dialect::Mermaid`].
///
/// # Errors
///
/// Same never-fail-all contract as [`import`].
#[allow(clippy::unnecessary_wraps)] // the Result is the shared CLI contract, see import()
pub(crate) fn import_dialect(cleaned: &str, dialect: Dialect) -> Result<Imported, ImportError> {
    let mut pp = Preprocessor::default();
    let fed: Vec<String> = cleaned.lines().filter_map(|l| pp.feed(l)).collect();
    let mut im = Importer::new(dialect);
    for line in join_logical_lines(fed.iter().map(String::as_str)) {
        im.line(&line);
    }
    Ok(Imported {
        workspace: im.builder.build(),
        warnings: im.warnings,
    })
}

struct Importer {
    dialect: Dialect,
    builder: WorkspaceBuilder,
    warnings: Vec<String>,
    aliases: HashMap<String, Id>,
    /// Open `{ }` scopes, innermost last (the boundary stack).
    scopes: Vec<Scope>,
    /// A `*_Boundary` call whose `{` has not appeared yet (single-line
    /// and next-line brace styles both work).
    pending: Vec<Scope>,
}

impl Importer {
    fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            builder: WorkspaceBuilder::default(),
            warnings: Vec::new(),
            aliases: HashMap::new(),
            scopes: Vec::new(),
            pending: Vec::new(),
        }
    }

    fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    fn top_system(&self) -> Option<Id> {
        self.scopes.iter().rev().find_map(|s| match s {
            Scope::System(i) => Some(i.clone()),
            _ => None,
        })
    }

    fn top_container(&self) -> Option<Id> {
        self.scopes.iter().rev().find_map(|s| match s {
            Scope::Container(i) => Some(i.clone()),
            _ => None,
        })
    }

    /// One logical line: leading `}` close scopes, then at most one
    /// macro call, then the trailing `{`/`}` keep the scope stack in
    /// sync; non-call lines are chrome (`title`/`Title:` tolerated, raw
    /// arrows warned).
    fn line(&mut self, line: &str) {
        let mut rest = line.trim();
        while let Some(r) = rest.strip_prefix('}') {
            self.scopes.pop();
            rest = r.trim_start();
        }
        if rest.is_empty() {
            return;
        }
        if let Some((call, end)) = Call::parse(rest) {
            let tail = &rest[end..];
            let opens = tail.contains('{');
            let pushed = self.call(&call, opens);
            if !pushed && opens {
                self.take_pending_or_pass();
            }
            for _ in 0..tail.matches('}').count() {
                self.scopes.pop();
            }
            if tail
                .chars()
                .filter(|c| *c != '{' && *c != '}')
                .collect::<String>()
                .contains('(')
            {
                self.warn(format!(
                    "skipped content after \"{}\" (only one macro call per line is imported)",
                    call.name
                ));
            }
            return;
        }
        let lower = rest.to_ascii_lowercase();
        let is_title = lower == "title" || lower.starts_with("title ");
        if !is_title && contains_arrow(rest) {
            self.warn(format!(
                "skipped unsupported {} relationship: {rest}",
                self.dialect.name()
            ));
        }
        for _ in 0..rest.matches('{').count() {
            self.take_pending_or_pass();
        }
        for _ in 0..rest.matches('}').count() {
            self.scopes.pop();
        }
    }

    fn take_pending_or_pass(&mut self) {
        if self.pending.is_empty() {
            self.scopes.push(Scope::Pass);
        } else {
            self.scopes.push(self.pending.remove(0));
        }
    }

    /// Dispatches one call; returns whether a scope was pushed directly
    /// (a boundary consuming a same-line `{`).
    fn call(&mut self, call: &Call, opens: bool) -> bool {
        if let Some((_, spec)) = ELEMENTS.iter().find(|(n, _)| *n == call.name) {
            self.element(*spec, call);
            return false;
        }
        if let Some(b) = boundary_of(&call.name) {
            return self.boundary(b, call, opens);
        }
        if let Some(kind) = rel_kind(&call.name) {
            self.rel(kind, call);
            return false;
        }
        if TOLERATED.iter().any(|p| call.name.starts_with(p)) {
            return false;
        }
        self.warn(format!("unknown macro \"{}\" — skipped", call.name));
        false
    }

    /// `Person/System` arity `(alias, label, descr, sprite, tags, link,
    /// type)`; `Container/Component` swaps `descr`→`(techn, descr)` and
    /// shifts the tail by one (verified from the official includes).
    fn element(&mut self, spec: ElementSpec, call: &Call) {
        let macro_name = &call.name;
        let Some(alias) = call.pos(0) else {
            self.warn(format!("{macro_name}: no alias — skipped"));
            return;
        };
        let label = call.pos(1).unwrap_or("");
        let (descr, technology, tags) = if matches!(spec.kind, Kind::Container | Kind::Component) {
            (
                call.arg(3, "descr").map(str::to_owned),
                call.arg(2, "techn").map(str::to_owned),
                call.arg(self.dialect.tags_container(), "tags"),
            )
        } else {
            (
                call.arg(2, "descr").map(str::to_owned),
                // `System`'s optional `$type` renders where technology
                // goes — keep it as technology.
                call.arg(6, "type").map(str::to_owned),
                call.arg(self.dialect.tags_person(), "tags"),
            )
        };
        let csv = tag_csv(tags, spec.extra);
        let id = match spec.kind {
            Kind::Person => self.builder.add_person(label, descr, csv.as_deref()),
            Kind::System => self
                .builder
                .add_software_system(label, descr, csv.as_deref()),
            Kind::Container => {
                let Some(system) = self.top_system() else {
                    self.warn(format!(
                        "skipped {macro_name} \"{label}\": declared outside a System_Boundary"
                    ));
                    return;
                };
                self.builder
                    .add_container(&system, label, descr, technology, csv.as_deref())
            }
            Kind::Component => {
                let Some(container) = self.top_container() else {
                    self.warn(format!(
                        "skipped {macro_name} \"{label}\": declared outside a Container_Boundary"
                    ));
                    return;
                };
                self.builder
                    .add_component(&container, label, descr, technology, csv.as_deref())
            }
        };
        match id {
            Ok(id) => {
                self.aliases.insert(alias.to_owned(), id);
            }
            Err(err) => self.warn(format!("skipped {macro_name} \"{label}\": {err}")),
        }
    }

    /// `*_Boundary(alias, label, tags, link, descr)` (verified arity).
    fn boundary(&mut self, b: Boundary, call: &Call, opens: bool) -> bool {
        let Some(alias) = call.pos(0) else {
            self.warn(format!("{}: no alias — skipped", call.name));
            return false;
        };
        if b == Boundary::Pass {
            return false;
        }
        let label = call.pos(1).unwrap_or("");
        let descr = call.arg(4, "descr").map(str::to_owned);
        let extra: &[&str] = if call.name.ends_with("_Ext") {
            EXT
        } else {
            &[]
        };
        let csv = tag_csv(call.arg(2, "tags"), extra);
        let created = if b == Boundary::System {
            self.builder
                .add_software_system(label, descr, csv.as_deref())
        } else {
            let Some(system) = self.top_system() else {
                self.warn(format!(
                    "skipped {} \"{label}\": no enclosing System_Boundary",
                    call.name
                ));
                return false;
            };
            self.builder
                .add_container(&system, label, descr, None, csv.as_deref())
        };
        match created {
            Ok(id) => {
                self.aliases.insert(alias.to_owned(), id.clone());
                let scope = if b == Boundary::System {
                    Scope::System(id)
                } else {
                    Scope::Container(id)
                };
                if opens {
                    self.scopes.push(scope);
                    return true;
                }
                self.pending.push(scope);
                false
            }
            Err(err) => {
                // Failed boundary: pass-through (its children still
                // import, top-level where their own scope is missing).
                self.warn(format!("skipped {} \"{label}\": {err}", call.name));
                false
            }
        }
    }

    /// `Rel*(from, to, label, techn, descr, sprite, tags, link)`. The
    /// label is the model DESCRIPTION (round-trip symmetry with the
    /// the official C4-PlantUML exporter, which writes the description into
    /// the label slot); the optional `$descr` renders below the label in
    /// `PlantUML` but joins it here — Structurizr has one description
    /// field.
    fn rel(&mut self, kind: RelKind, call: &Call) {
        let Some(from) = call.pos(0) else { return };
        let Some(to) = call.pos(1) else { return };
        let label = call.pos(2).unwrap_or("");
        let (source, destination) = match (self.aliases.get(from), self.aliases.get(to)) {
            (Some(s), Some(d)) => (s.clone(), d.clone()),
            (None, _) => {
                self.warn(format!(
                    "skipped {} \"{label}\": unknown element alias \"{from}\"",
                    call.name
                ));
                return;
            }
            (_, None) => {
                self.warn(format!(
                    "skipped {} \"{label}\": unknown element alias \"{to}\"",
                    call.name
                ));
                return;
            }
        };
        let description = match (
            label.is_empty(),
            call.arg(self.dialect.rel_descr(), "descr"),
        ) {
            (true, d) => d.map(str::to_owned),
            (false, None) => Some(label.to_owned()),
            (false, Some(d)) => Some(format!("{label}, {d}")),
        };
        let technology = call.arg(3, "techn").map(str::to_owned);
        let csv = tag_csv(call.arg(self.dialect.rel_tags(), "tags"), &[]);
        let pairs: Vec<(Id, Id)> = match kind {
            RelKind::Forward => vec![(source, destination)],
            RelKind::Backward => vec![(destination, source)],
            RelKind::Bidirectional => {
                vec![(source.clone(), destination.clone()), (destination, source)]
            }
        };
        for (s, d) in pairs {
            if let Err(err) = self.builder.add_relationship(
                &s,
                &d,
                description.clone(),
                technology.clone(),
                csv.as_deref(),
            ) {
                self.warn(format!("skipped relationship \"{label}\": {err}"));
            }
        }
    }
}

/// `$tags` are `+`-joined (official C4-PlantUML convention); `,` also
/// splits defensively. Stored order: user tags, then the macro's extra.
fn tag_csv(tags: Option<&str>, extra: &[&str]) -> Option<String> {
    let mut parts: Vec<&str> = tags
        .map(|t| {
            t.split(['+', ','])
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();
    parts.extend(extra);
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(","))
    }
}

/// Relationship syntax outside quotes: `->`, `<-`, `..>`, `<..` (covers
/// `-->`, `<<-->>`, dashed forms). Raw `PlantUML` arrows are NOT promised
/// (macro calls are the promised surface) — the caller warns and skips.
fn contains_arrow(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut quoted = false;
    for i in 0..chars.len() {
        let c = chars[i];
        if quoted {
            if c == '"' {
                quoted = false;
            }
            continue;
        }
        if c == '"' {
            quoted = true;
            continue;
        }
        if matches!(c, '-' | '.') && chars.get(i + 1) == Some(&'>') {
            return true;
        }
        if matches!(c, '<' | '.') && chars.get(i + 1) == Some(&'-') {
            return true;
        }
        if c == '.' && chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'>') {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::testutil::digest;
    use expect_test::expect;
    use oxidizr_core::tags::TagKind;

    fn import_ok(src: &str) -> Imported {
        import(src).unwrap()
    }

    #[test]
    fn context_diagram_end_to_end() {
        let src = "\
title Example Bank - System Context
!include https://github.com/plantuml-stdlib/C4-PlantUML/blob/master/C4_Container.puml
skinparam defaultFontSize 14
Person(customer, \"Personal Banking Customer\", \"A customer of the bank.\")
Enterprise_Boundary(bank, \"Example Bank\") {
    System(banking, \"Internet Banking System\", $descr=\"Allows customers to view accounts.\", $tags=\"Critical\")
    System_Ext(mail, \"E-mail System\", \"The internal Microsoft Exchange system.\")
}
LAYOUT_WITH_LEGEND()
Rel(customer, banking, \"Views accounts\", \"HTTPS\")
BiRel(banking, mail, \"Sends e-mails\", \"SMTP\")
Rel_Back(mail, customer, \"Notifies\", \"SMS\")
left to right direction
@enduml
";
        let im = import_ok(src);
        expect![[r#"
            person Personal Banking Customer [Element,Person] Some("A customer of the bank.")
            system Internet Banking System [Element,Software System,Critical] Some("Allows customers to view accounts.")
            system E-mail System [Element,Software System,External] Some("The internal Microsoft Exchange system.")
            rel Personal Banking Customer -> Internet Banking System "Views accounts" tech=Some("HTTPS") []
            rel Internet Banking System -> E-mail System "Sends e-mails" tech=Some("SMTP") []
            rel E-mail System -> Internet Banking System "Sends e-mails" tech=Some("SMTP") []
            rel Personal Banking Customer -> E-mail System "Notifies" tech=Some("SMS") []
        "#]].assert_eq(&digest(&im));
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn boundaries_scope_children_and_nest() {
        let src = "\
System_Boundary(c1, \"Internet Banking System\") {
    Container(web, \"Web Application\", \"Java, Spring Boot\")
    ContainerDb(db, \"Database\", \"Relational Database Schema\", $descr=\"Stores user data\")
    Container_Boundary(m, \"Mobile App\") {
        Component(sync, \"Sync Service\", \"Kotlin\")
    }
    Component(orphan, \"Ghost\")
}
Container(floating, \"Floating\")
Rel(web, sync, \"Reads\", \"JDBC\")
";
        let im = import_ok(src);
        let ws = &im.workspace;
        assert_eq!(ws.model.software_systems.len(), 1);
        let sys = &ws.model.software_systems[0];
        assert_eq!(sys.item.name, "Internet Banking System");
        let containers: Vec<&str> = sys
            .containers
            .iter()
            .map(|c| c.item.name.as_str())
            .collect();
        assert_eq!(containers, ["Web Application", "Database", "Mobile App"]);
        let mobile = sys
            .containers
            .iter()
            .find(|c| c.item.name == "Mobile App")
            .unwrap();
        assert_eq!(mobile.components.len(), 1);
        assert_eq!(mobile.components[0].item.name, "Sync Service");
        assert_eq!(ws.model.relationships.len(), 1);
        // The out-of-scope Component and Container warn, never abort.
        assert_eq!(im.warnings.len(), 2, "{:?}", im.warnings);
        assert!(im.warnings[0].starts_with("skipped Component \"Ghost\""));
        assert!(im.warnings[1].starts_with("skipped Container \"Floating\""));
    }

    #[test]
    fn boundary_brace_may_start_the_next_line() {
        let src = "\
System_Boundary(c1, \"Boxed\")
{
    Container(x, \"X\")
}
";
        let im = import_ok(src);
        let sys = &im.workspace.model.software_systems[0];
        assert_eq!(sys.containers.len(), 1);
        assert_eq!(sys.containers[0].item.name, "X");
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn variables_kwargs_and_multiline_calls() {
        let src = "\
!$name = \"Big Bank plc\"
!$who = \"Analyst\"
Person($who, \"System Analyst\", \"Maintains the architecture\")
System(banking, $name, , $tags=\"Finance+Core\")
Rel(
    banking,
    $who,
    \"Uses, really\"
    , \"HTTPS\"
)
";
        let im = import_ok(src);
        let ws = &im.workspace;
        let analyst = &ws.model.people[0];
        // The FIRST argument is the alias (`Analyst`), the model name is
        // the label.
        assert_eq!(analyst.item.name, "System Analyst");
        let banking = &ws.model.software_systems[0];
        assert_eq!(banking.item.name, "Big Bank plc");
        assert_eq!(banking.item.description, None); // empty positional slot
        assert_eq!(
            banking.item.tags.serialized_for(TagKind::SoftwareSystem),
            "Element,Software System,Finance,Core"
        );
        let r = &ws.model.relationships[0];
        assert_eq!(r.description.as_deref(), Some("Uses, really"));
        assert_eq!(r.technology.as_deref(), Some("HTTPS"));
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn chrome_is_tolerated_and_unsupported_forms_warn() {
        let src = "\
Person(customer, \"Customer\")
System(banking, \"Banking\")
legend
  colors and such
endlegend
UpdateElementStyle($bgColor=blue)
AddElementTag(\"t\", $bgColor=\"#fff\")
Lay_Out()
UnknownThing(a, \"b\")
customer -> banking : Uses
";
        let im = import_ok(src);
        assert_eq!(im.warnings.len(), 2, "{:?}", im.warnings);
        assert_eq!(im.warnings[0], "unknown macro \"UnknownThing\" — skipped");
        assert_eq!(
            im.warnings[1],
            "skipped unsupported PlantUML relationship: customer -> banking : Uses"
        );
    }

    #[test]
    fn rel_variants_map_to_model_directions() {
        let src = "\
Person(a, \"A\")
System(b, \"B\")
SystemRel(a, b, \"Scoped legacy\", \"HTTP\")
Rel_U(a, b, \"Upwards\")
Rel_D(b, a, \"Downwards\", \"SMTP\")
";
        let im = import_ok(src);
        assert_eq!(
            digest(&im),
            "person A [Element,Person] None\n\
             system B [Element,Software System] None\n\
             rel A -> B \"Scoped legacy\" tech=Some(\"HTTP\") []\n\
             rel A -> B \"Upwards\" tech=None []\n\
             rel B -> A \"Downwards\" tech=Some(\"SMTP\") []\n"
        );
        // A duplicate (same ends + description) warns through the
        // builder; the reverse direction of the BiRel still mints.
        let dup =
            import_ok("Person(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"x\")\nBiRel(a, b, \"x\")\n");
        assert_eq!(dup.workspace.model.relationships.len(), 2);
        assert_eq!(dup.warnings.len(), 1);
    }

    #[test]
    fn import_never_produces_views() {
        // Pinned guard: no view
        // kind may host a mixed C4 set, so imports ship the model only —
        // `api::open` installs the default views on load.
        let im = import_ok("Person(a, \"A\")\nSystem(b, \"B\")\nRel(a, b, \"uses\")\n");
        let views = &im.workspace.views;
        assert_eq!(
            views.custom_views,
            [] as [oxidizr_core::views::CustomView; 0]
        );
        assert_eq!(views.image_views, [] as [oxidizr_core::views::ImageView; 0]);
        assert_eq!(
            views.system_landscape_views,
            [] as [oxidizr_core::views::SystemLandscapeView; 0]
        );
        let empty = import_ok("@startuml\n@enduml\n");
        assert_eq!(empty.warnings, [] as [String; 0]);
        assert_eq!(empty.workspace.model.people.len(), 0);
    }
}
