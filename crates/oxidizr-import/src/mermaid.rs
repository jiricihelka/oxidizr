//! The `Mermaid` C4 model importer (agreed additional feature; it is
//! verified the official tooling has no model-parser counterpart). The
//! machinery is shared with [`crate::c4plantuml`]; this module pins the
//! `Mermaid` dialect differences:
//!
//! - `C4Context`/`C4Container`/`C4Component` headers and `title`/
//!   `direction` lines are chrome, `%%` comments (incl. `%%{init}%%`
//!   config) vanish via [`strip_comments_mermaid`];
//! - positional `$tags` sits one slot earlier than in C4-`PlantUML`:
//!   `Person(a, "A", $descr, $tags)` and `Container(a, "A", $techn,
//!   $descr, $tags)` (`Component` likewise); `Rel(a, b, $label, $techn,
//!   $tags)` has NO positional `$descr` — it is keyword-only;
//! - `Container_Boundary_Ext` opens a container scope carrying the
//!   `External` tag;
//! - layout chrome (`Enter_Boundary`/`Leave_Boundary`/`Update*`/`Set*`/
//!   `Show*`/`Hide*`/`CLEAR_*`/`SHOW_LEGEND`) is tolerated silently;
//!   raw flowchart edges (`-->`) warn and are skipped — the macro
//!   calls are the promised surface;
//! - the output is the MODEL ONLY, no view: no view kind may host a
//!   mixed C4 element set, the default views install at open (see the
//!   `c4plantuml` module docs).

use crate::c4plantuml::{Dialect, import_dialect};
use crate::macrocall::strip_comments_mermaid;
use crate::{ImportError, Imported};

/// Import one `Mermaid` C4 diagram into a fresh workspace (module docs
/// for the full contract). File I/O stays upstream (N-4's CLI arc).
///
/// # Errors
///
/// Same never-fail-all contract as [`crate::c4plantuml::import`].
pub fn import(text: &str) -> Result<Imported, ImportError> {
    import_dialect(&strip_comments_mermaid(text), Dialect::Mermaid)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::import;
    use crate::testutil::digest;
    use expect_test::expect;
    use oxidizr_core::tags::TagKind;

    fn import_ok(src: &str) -> crate::Imported {
        import(src).unwrap()
    }

    #[test]
    fn context_diagram_end_to_end() {
        let src = "\
C4Context
%% Example Bank - System Context, Mermaid flavour. Comments vanish. %%
title Example Bank - System Context
Person(customer, \"Personal Banking Customer\", \"A customer of the bank.\")
Enterprise_Boundary(bank, \"Example Bank\") {
    System(banking, \"Internet Banking System\", $descr=\"Allows customers to view accounts.\", $tags=\"Critical\")
    System_Ext(mail, \"E-mail System\", \"The internal Microsoft Exchange system.\")
}
Person(analyst, \"System Analyst\", \"\", \"Internal\")
SystemQueue(claims, \"Claims Queue\", \"Kafka\")
System_Boundary(mbank, \"Mobile Banking System\") {
    Container(web, \"Web Application\", \"Java 21, Spring Boot\", \"Serves the single page app.\", \"Critical\")
    ContainerDb(db, \"Database\", \"PostgreSQL\", \"Stores customer data.\")
}
Rel(customer, banking, \"Uses\", \"HTTPS\", \"Primary\")
Rel(banking, mail, \"Sends email\", $descr=\"via SMTP\")
Rel_Back(banking, analyst, \"Reports to\")
BiRel(web, db, \"Reads and writes\")
";
        let im = import_ok(src);
        expect![[r#"
            person Personal Banking Customer [Element,Person] Some("A customer of the bank.")
            person System Analyst [Element,Person,Internal] None
            system Internet Banking System [Element,Software System,Critical] Some("Allows customers to view accounts.")
            system E-mail System [Element,Software System,External] Some("The internal Microsoft Exchange system.")
            system Claims Queue [Element,Software System,Queue] Some("Kafka")
            system Mobile Banking System [Element,Software System] None
              container Web Application [Element,Container,Critical] tech=Some("Java 21, Spring Boot") Some("Serves the single page app.")
              container Database [Element,Container,Database] tech=Some("PostgreSQL") Some("Stores customer data.")
            rel Personal Banking Customer -> Internet Banking System "Uses" tech=Some("HTTPS") [Primary]
            rel Internet Banking System -> E-mail System "Sends email, via SMTP" tech=None []
            rel System Analyst -> Internet Banking System "Reports to" tech=None []
            rel Mobile Banking System / Web Application -> Mobile Banking System / Database "Reads and writes" tech=None []
            rel Mobile Banking System / Database -> Mobile Banking System / Web Application "Reads and writes" tech=None []
        "#]].assert_eq(&digest(&im));
    }

    #[test]
    fn positional_tag_slots_are_one_earlier() {
        let im = import_ok(
            "\
Person(a, \"A\", \"Desc\", \"Alpha\")
System(s, \"S\", \"SD\", \"Beta\")
System_Boundary(sb, \"SB\") {
    Container(c, \"C\", \"tech\", \"cd\", \"Gamma\")
}
Rel(a, s, \"uses\", \"HTTPS\", \"Delta\")
",
        );
        let ws = &im.workspace;
        assert_eq!(
            ws.model.people[0].item.tags.serialized_for(TagKind::Person),
            "Element,Person,Alpha"
        );
        assert_eq!(
            ws.model.software_systems[0]
                .item
                .tags
                .serialized_for(TagKind::SoftwareSystem),
            "Element,Software System,Beta"
        );
        let container = &ws.model.software_systems[1].containers[0];
        assert_eq!(
            container.item.tags.serialized_for(TagKind::Container),
            "Element,Container,Gamma"
        );
        let r = &ws.model.relationships[0];
        assert_eq!(r.description.as_deref(), Some("uses"));
        assert_eq!(r.technology.as_deref(), Some("HTTPS"));
        assert_eq!(r.tags.stored().join("+"), "Delta");
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn rel_descr_is_keyword_only() {
        let im = import_ok(
            "\
System(a, \"A\")
System(b, \"B\")
Rel(a, b, \"Uses\", \"HTTPS\")
System(c, \"C\")
Rel(a, c, \"X\", \"Y\", $descr=\"d\")
",
        );
        let ws = &im.workspace;
        assert_eq!(
            ws.model.relationships[0].description.as_deref(),
            Some("Uses")
        );
        assert_eq!(
            ws.model.relationships[1].description.as_deref(),
            Some("X, d")
        );
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn container_boundary_ext_is_external() {
        let im = import_ok(
            "\
System_Boundary(host, \"Host\") {
    Container_Boundary_Ext(legacy, \"Legacy Suite\") {
        Component(ui, \"UI\", \"JS\", \"The legacy front end.\")
    }
}
",
        );
        let ws = &im.workspace;
        let suite = &ws.model.software_systems[0].containers[0];
        assert_eq!(suite.item.name, "Legacy Suite");
        assert!(
            suite
                .item
                .tags
                .serialized_for(TagKind::Container)
                .contains("External")
        );
        assert_eq!(suite.components[0].item.name, "UI");
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn chrome_is_tolerated_silently() {
        let im = import_ok(
            "\
C4Context
title Layout demo
direction LR
System(s, \"S\")
UpdateElementStyle(s, $bgColor=red)
Person(p, \"P\")
UpdateRelStyle(p, s, $offsetY=-20)
SetConfigOption(\"showPersona\", \"true\")
HidePerson()
ShowPerson()
CLEAR_SKIPPED_HISTORY()
SHOW_LEGEND()
Enter_Boundary(b1, \"Inner\") {
    System(x, \"X\")
}
Leave_Boundary()
",
        );
        assert_eq!(im.workspace.model.software_systems.len(), 2);
        assert_eq!(im.warnings, [] as [String; 0]);
    }

    #[test]
    fn raw_flowchart_edge_warns() {
        let im = import_ok("System(s1, \"S1\")\nbob --> s1\n");
        assert_eq!(
            im.warnings,
            ["skipped unsupported Mermaid relationship: bob --> s1".to_owned()]
        );
    }

    #[test]
    fn semicolons_and_next_line_braces() {
        let im = import_ok(
            "\
Person(a, \"A\");
System_Boundary(s, \"S\")
{
    Container(c, \"C\");
}
",
        );
        let ws = &im.workspace;
        assert_eq!(ws.model.software_systems[0].containers[0].item.name, "C");
        assert_eq!(im.warnings, [] as [String; 0]);
    }
}
