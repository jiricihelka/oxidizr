//! §P4-M3 O-4 `generate system-landscape` tests (Category A,
//! `agents/plan/build/PHASE_4.md`): the oracle-witnessed merge contract
//! (name-first person/system clone, description/url LAST-workspace-wins,
//! tags union, stored custom tags ride the clone), the `First`/`All`
//! relationship strategies, the `{workspace:<id>}/diagrams` hyperlink on
//! each source workspace's first container-bearing system, the
//! full-path filter legs (`--include`/`--exclude`), the exit family
//! (0 · zero workspaces 1 · unsupported pattern 1/OXZ-5010 · missing
//! path 3) and the load-failure skip note. Fixtures are the witness
//! alpha/beta DSLs verbatim; the goldens mask `structurizr.*` properties
//! exactly like the oracle A/B protocol (loader-regenerated, never
//! content). ONE workspace is legal — the reference only throws on ZERO
//! workspaces despite the "Two or more" wording.
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::path::PathBuf;

use expect_test::expect;
use oxidizr_api::RelationshipsStrategy;
use oxidizr_api::pipeline::{ApiOptions, ExitClass, LogItem, ParseFlags, Request, execute};

/// Hermetic options (same posture as the `init_probes` harness).
fn opts() -> ApiOptions {
    ApiOptions {
        parse: ParseFlags {
            strict: false,
            keep_unknown: false,
        },
        env: Some(std::collections::BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

fn tempdir(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("oxidizr-api-generate-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The witness fixtures (oracle session 2026-11-21, TZ=UTC legs): alpha
/// carries Alice (url + `Staff` tag + property), Billing (container,
/// `Uses`/HTTP relation with url + `CustomRel` tag), CRM; beta redefines
/// Alice (`External` tag), CRM (container), Warehouse and the edges.
const ALPHA: &str = r#"workspace "Alpha" {
    model {
        alice = person "Alice" "Alpha Alice" {
            url "https://alpha.example/alice"
            tag "Staff"
            properties {
                alphaProp "1"
            }
        }
        billing = softwareSystem "Billing" "Alpha billing description" {
            api = container "Billing API"
        }
        crm = softwareSystem "CRM" "Alpha CRM description"
        alice -> billing "Uses" "HTTP" {
            url "https://rel/uses"
            tag "CustomRel"
        }
        alice -> crm "Uses"
        billing -> crm "Synchronises with"
    }
}
"#;

const BETA: &str = r#"workspace "Beta" {
    model {
        alice = person "Alice" "Beta Alice" {
            tag "External"
        }
        crm = softwareSystem "CRM" "Beta CRM description" {
            ui = container "CRM UI"
        }
        warehouse = softwareSystem "Warehouse" "Beta warehouse"
        alice -> crm "Uses"
        alice -> crm "Reports on"
        crm -> warehouse "Pushes orders to"
    }
}
"#;

/// Lay the two witness workspaces at `<dir>/alpha|beta/workspace.dsl`
/// (the sorted scan order alpha→beta is the deterministic merge order).
fn fixtures(dir: &Path) {
    for (tag, text) in [("alpha", ALPHA), ("beta", BETA)] {
        let ws_dir = dir.join(tag);
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("workspace.dsl"), text).unwrap();
    }
}

/// Run `generate system-landscape`; returns
/// `(exit, stdout bytes, human log lines)`.
fn generate(
    inputs: Vec<PathBuf>,
    include: Option<&str>,
    exclude: Option<&str>,
    strategy: RelationshipsStrategy,
) -> (ExitClass, String, Vec<String>) {
    let out = execute(
        Request::Generate {
            inputs,
            include: include.map(str::to_string),
            exclude: exclude.map(str::to_string),
            strategy,
            output: None,
        },
        &opts(),
    );
    let log = out
        .log
        .iter()
        .map(|item| match item {
            LogItem::Issue(issue) => issue.message.clone(),
            LogItem::OkLine(line) | LogItem::NoteLine(line) => line.clone(),
        })
        .collect();
    (
        out.exit,
        String::from_utf8(out.bytes.unwrap_or_default()).unwrap(),
        log,
    )
}

/// The oracle-protocol mask: every `properties` object loses its
/// `structurizr.*` keys (loader-regenerated per parse — never content).
fn strip_structurizr_props(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(props)) = map.get_mut("properties") {
                props.retain(|k, _| !k.starts_with("structurizr."));
            }
            for child in map.values_mut() {
                strip_structurizr_props(child);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                strip_structurizr_props(child);
            }
        }
        _ => {}
    }
}

/// The witnessed merge under BOTH strategies (bless with `UPDATE_EXPECT=1`
/// after an intentional contract change — drift is the point).
#[allow(clippy::too_many_lines)] // the inline goldens ARE the contract
#[test]
fn landscape_matches_the_witnessed_merge() {
    let dir = tempdir("merge");
    fixtures(&dir);
    let mut snapshots = Vec::new();
    for strategy in [RelationshipsStrategy::First, RelationshipsStrategy::All] {
        let (exit, json, log) = generate(vec![dir.clone()], None, None, strategy);
        assert_eq!(exit, ExitClass::Ok, "{strategy:?}: clean generate");
        assert!(log.is_empty(), "{strategy:?}: clean log: {log:?}");
        let mut doc: serde_json::Value = serde_json::from_slice(json.as_bytes()).unwrap();
        strip_structurizr_props(&mut doc);
        snapshots.push(format!(
            "### {strategy:?}\n{}\n",
            serde_json::to_string_pretty(&doc).unwrap()
        ));
    }
    let joined = snapshots.join("\n");
    expect![[r#"
        ### First
        {
          "configuration": {
            "scope": "Landscape"
          },
          "documentation": {},
          "id": 0,
          "model": {
            "people": [
              {
                "description": "Beta Alice",
                "id": "1",
                "name": "Alice",
                "properties": {
                  "alphaProp": "1"
                },
                "relationships": [
                  {
                    "description": "Uses",
                    "destinationId": "2",
                    "id": "5",
                    "sourceId": "1",
                    "tags": "Relationship,CustomRel",
                    "technology": "HTTP",
                    "url": "https://rel/uses"
                  },
                  {
                    "description": "Uses",
                    "destinationId": "3",
                    "id": "6",
                    "sourceId": "1",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Person,Staff,External"
              }
            ],
            "softwareSystems": [
              {
                "description": "Alpha billing description",
                "documentation": {},
                "id": "2",
                "name": "Billing",
                "properties": {},
                "relationships": [
                  {
                    "description": "Synchronises with",
                    "destinationId": "3",
                    "id": "7",
                    "sourceId": "2",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Software System",
                "url": "{workspace:0}/diagrams"
              },
              {
                "description": "Beta CRM description",
                "documentation": {},
                "id": "3",
                "name": "CRM",
                "properties": {},
                "relationships": [
                  {
                    "description": "Pushes orders to",
                    "destinationId": "4",
                    "id": "8",
                    "sourceId": "3",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Software System",
                "url": "{workspace:0}/diagrams"
              },
              {
                "description": "Beta warehouse",
                "documentation": {},
                "id": "4",
                "name": "Warehouse",
                "properties": {},
                "tags": "Element,Software System"
              }
            ]
          },
          "name": "System Landscape",
          "views": {
            "configuration": {
              "styles": {},
              "terminology": {}
            }
          }
        }

        ### All
        {
          "configuration": {
            "scope": "Landscape"
          },
          "documentation": {},
          "id": 0,
          "model": {
            "people": [
              {
                "description": "Beta Alice",
                "id": "1",
                "name": "Alice",
                "properties": {
                  "alphaProp": "1"
                },
                "relationships": [
                  {
                    "description": "Uses",
                    "destinationId": "2",
                    "id": "5",
                    "sourceId": "1",
                    "tags": "Relationship,CustomRel",
                    "technology": "HTTP",
                    "url": "https://rel/uses"
                  },
                  {
                    "description": "Uses",
                    "destinationId": "3",
                    "id": "6",
                    "sourceId": "1",
                    "tags": "Relationship"
                  },
                  {
                    "description": "Reports on",
                    "destinationId": "3",
                    "id": "8",
                    "sourceId": "1",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Person,Staff,External"
              }
            ],
            "softwareSystems": [
              {
                "description": "Alpha billing description",
                "documentation": {},
                "id": "2",
                "name": "Billing",
                "properties": {},
                "relationships": [
                  {
                    "description": "Synchronises with",
                    "destinationId": "3",
                    "id": "7",
                    "sourceId": "2",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Software System",
                "url": "{workspace:0}/diagrams"
              },
              {
                "description": "Beta CRM description",
                "documentation": {},
                "id": "3",
                "name": "CRM",
                "properties": {},
                "relationships": [
                  {
                    "description": "Pushes orders to",
                    "destinationId": "4",
                    "id": "9",
                    "sourceId": "3",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Software System",
                "url": "{workspace:0}/diagrams"
              },
              {
                "description": "Beta warehouse",
                "documentation": {},
                "id": "4",
                "name": "Warehouse",
                "properties": {},
                "tags": "Element,Software System"
              }
            ]
          },
          "name": "System Landscape",
          "views": {
            "configuration": {
              "styles": {},
              "terminology": {}
            }
          }
        }
    "#]]
    .assert_eq(&joined);
}

/// The filter legs over the FULL scan path (full match, like the
/// reference's `Pattern.matches`): an exclude kills beta, an unmatched
/// include yields ZERO workspaces — the ported reference message.
#[test]
fn filters_decide_the_scan() {
    let dir = tempdir("filters");
    fixtures(&dir);

    // `--exclude .*beta.*` — only alpha contributes (the directory
    // component match, as witnessed on the reference).
    let (exit, json, log) = generate(
        vec![dir.clone()],
        None,
        Some(r".*beta.*"),
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Ok, "alpha-only exclude");
    assert!(log.is_empty(), "exclude is silent: {log:?}");
    let doc: serde_json::Value = serde_json::from_slice(json.as_bytes()).unwrap();
    let systems: Vec<&str> = doc["model"]["softwareSystems"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(systems, ["Billing", "CRM"], "beta's Warehouse absent");

    // Nothing matches → exit 1 with the reference text verbatim.
    let (exit, json, log) = generate(
        vec![dir],
        Some(r".*gamma.*"),
        None,
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Diagnostics, "zero workspaces exits 1");
    assert!(json.is_empty(), "no bytes on the failure");
    assert_eq!(
        log,
        ["Two or more workspaces must be provided"],
        "ported reference message"
    );
}

/// ONE workspace is legal despite the reference's "Two or more" wording
/// (witnessed: the exception fires on ZERO workspaces only).
#[test]
fn one_workspace_is_legal() {
    let dir = tempdir("single");
    fixtures(&dir);
    let (exit, json, log) = generate(
        vec![dir.join("alpha/workspace.dsl")],
        None,
        None,
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Ok, "single explicit file");
    assert!(log.is_empty(), "clean log: {log:?}");
    let doc: serde_json::Value = serde_json::from_slice(json.as_bytes()).unwrap();
    assert_eq!(doc["name"], "System Landscape");
    assert_eq!(doc["configuration"]["scope"], "Landscape");
    // An explicit file positional loads even though it would miss a
    // custom include (documented deviation).
    let (exit, _, _) = generate(
        vec![dir.join("alpha/workspace.dsl")],
        Some(r".*nomatch.*"),
        None,
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Ok, "explicit file beats the filter");
}

/// An `--include`/`--exclude` construct outside the std-only subset is
/// OXZ-5010 (exit 1) — never a silent widening.
#[test]
fn unsupported_pattern_is_diagnostic() {
    let dir = tempdir("badpattern");
    fixtures(&dir);
    let (exit, json, log) = generate(
        vec![dir],
        Some(r"workspace\.(json|dsl){2}"),
        None,
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Diagnostics, "unsupported construct");
    assert!(json.is_empty(), "no bytes on the failure");
    assert_eq!(log.len(), 1, "one issue: {log:?}");
    assert!(
        log[0].starts_with("invalid `--include` pattern")
            && log[0].contains("unsupported construct"),
        "OXZ-5010 text: {:?}",
        log[0]
    );
}

/// A path positional that is neither file nor directory: exit 3.
#[test]
fn missing_path_is_io() {
    let dir = tempdir("missing");
    let (exit, _, log) = generate(
        vec![dir.join("nowhere")],
        None,
        None,
        RelationshipsStrategy::default(),
    );
    assert_eq!(exit, ExitClass::Io, "missing path exits 3");
    assert_eq!(log.len(), 1, "one issue: {log:?}");
    assert!(
        log[0].starts_with("cannot read `") && log[0].ends_with("(no such file or directory)"),
        "CLI_IO text: {:?}",
        log[0]
    );
}

/// A workspace that fails to LOAD is the reference per-file `log.warn`
/// skip (a note, the run carries on); if NOTHING loads, the exit-1
/// zero-workspaces message wins.
#[test]
fn broken_workspace_is_skipped_with_note() {
    let dir = tempdir("broken");
    fixtures(&dir);
    let gamma = dir.join("gamma");
    std::fs::create_dir_all(&gamma).unwrap();
    // Statement error: `person` without a name fails the reference
    // parser too (a full throw → file skip, never a partial merge).
    std::fs::write(
        gamma.join("workspace.dsl"),
        "workspace \"Gamma\" {\n    model {\n        person\n    }\n}\n",
    )
    .unwrap();
    let (exit, json, log) = generate(vec![dir], None, None, RelationshipsStrategy::default());
    assert_eq!(exit, ExitClass::Ok, "the good workspaces still merge");
    assert_eq!(log.len(), 1, "one skip note: {log:?}");
    // The note must name the file AS THE WALK RENDERED IT (on Windows the
    // same join displays backslash-separated) — a hardcoded "/" here is a
    // separator assumption, not a contract.
    let named = gamma.join("workspace.dsl");
    let named = named.display().to_string();
    assert!(
        log[0].starts_with("Could not parse ") && log[0].contains(&named),
        "skip note names the file: {:?}",
        log[0]
    );
    let doc: serde_json::Value = serde_json::from_slice(json.as_bytes()).unwrap();
    assert!(
        doc["model"]
            .get("people")
            .is_some_and(|p| p.as_array().unwrap().len() == 1),
        "gamma contributed nothing: {json}"
    );

    // ONLY the broken file → zero workspaces → the ported message.
    let lone = tempdir("broken-lone");
    std::fs::create_dir_all(lone.join("gamma")).unwrap();
    std::fs::write(
        lone.join("gamma/workspace.dsl"),
        "workspace \"Gamma\" {\n    model {\n        person\n    }\n}\n",
    )
    .unwrap();
    let (exit, _, log) = generate(vec![lone], None, None, RelationshipsStrategy::default());
    assert_eq!(exit, ExitClass::Diagnostics, "nothing loads exits 1");
    assert_eq!(log[1], "Two or more workspaces must be provided", "{log:?}");
}
