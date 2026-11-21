//! §P4-M3 O-2 `init` tests (Category A, `agents/plan/build/PHASE_4.md`):
//! the full scaffold tree, the idempotence contract (re-run SKIPS with
//! notes, `--force` restores the template bytes), nested-directory
//! creation, and the P4-D4 CI teeth — every scaffolded `workspace.dsl`
//! passes `validate` AND its `dsl2json` bytes match the inline golden
//! (template DRIFT fails the snapshot, which is the point). The golden
//! masks `structurizr.dsl*` properties exactly like the oracle A/B
//! protocol: they are loader-regenerated per parse (fresh
//! UUIDs / base64 snapshots), never content.
//!
//! These tests drive `oxidizr_api::execute` (the pipeline wiring) with
//! the hermetic options; the scaffold writes are REAL files under the
//! process temp dir (the command's documented side effect).
#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::path::PathBuf;

use expect_test::expect;
use oxidizr_api::InitTemplate;
use oxidizr_api::pipeline::{ApiOptions, ExitClass, Input, Request, execute};

/// Hermetic options (same posture as the `diff_probes` harness).
fn opts() -> ApiOptions {
    ApiOptions {
        strict: false,
        keep_unknown: false,
        env: Some(std::collections::BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

fn tempdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-api-init-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Run `init`; returns `(exit, stdout report)`.
fn init(dir: &Path, template: InitTemplate, force: bool) -> (ExitClass, String) {
    let out = execute(
        Request::Init {
            dir: dir.to_path_buf(),
            template,
            force,
        },
        &opts(),
    );
    (
        out.exit,
        String::from_utf8(out.bytes.unwrap_or_default()).unwrap(),
    )
}

const TREE: [&str; 5] = [
    "workspace.dsl",
    "README.md",
    ".gitignore",
    ".editorconfig",
    ".vscode/settings.json",
];

const CREATED: &str = "created workspace.dsl\n\
    created README.md\n\
    created .gitignore\n\
    created .editorconfig\n\
    created .vscode/settings.json\n\
    next steps:\n  \
    oxidizr validate workspace.dsl\n  \
    oxidizr export workspace.dsl -f plantuml\n";

const SKIPPED: &str = "skipped workspace.dsl (exists, use --force to overwrite)\n\
    skipped README.md (exists, use --force to overwrite)\n\
    skipped .gitignore (exists, use --force to overwrite)\n\
    skipped .editorconfig (exists, use --force to overwrite)\n\
    skipped .vscode/settings.json (exists, use --force to overwrite)\n\
    next steps:\n  \
    oxidizr validate workspace.dsl\n  \
    oxidizr export workspace.dsl -f plantuml\n";

#[test]
fn init_creates_the_full_tree_and_reruns_skip_everything() {
    for (template, tag) in [(InitTemplate::Minimal, "min"), (InitTemplate::C4, "c4")] {
        let dir = tempdir(tag);
        let (exit, report) = init(&dir, template, false);
        assert_eq!(exit, ExitClass::Ok, "{tag}: fresh init");
        assert_eq!(report, CREATED, "{tag}: created report");
        for file in TREE {
            assert!(dir.join(file).is_file(), "{tag}: {file} missing");
        }

        // Idempotent: the re-run SKIPS every file and changes nothing.
        let before: Vec<Vec<u8>> = TREE
            .iter()
            .map(|f| std::fs::read(dir.join(f)).unwrap())
            .collect();
        let (exit, report) = init(&dir, template, false);
        assert_eq!(exit, ExitClass::Ok, "{tag}: idempotent re-run stays 0");
        assert_eq!(report, SKIPPED, "{tag}: skipped report");
        let after: Vec<Vec<u8>> = TREE
            .iter()
            .map(|f| std::fs::read(dir.join(f)).unwrap())
            .collect();
        assert_eq!(before, after, "{tag}: re-run touched files");
    }
}

#[test]
fn force_restores_tampered_files() {
    let dir = tempdir("force");
    let (exit, _) = init(&dir, InitTemplate::C4, false);
    assert_eq!(exit, ExitClass::Ok);
    let ws = dir.join("workspace.dsl");
    std::fs::write(&ws, b"tampered").unwrap();
    let (exit, report) = init(&dir, InitTemplate::C4, true);
    assert_eq!(exit, ExitClass::Ok);
    assert_eq!(report, CREATED, "--force reports created");
    let bytes = std::fs::read(&ws).unwrap();
    assert!(!bytes.starts_with(b"tampered"), "template restored");
    assert!(
        bytes.starts_with(b"workspace \"Web Shop\""),
        "{:?}",
        &bytes[..30]
    );
}

#[test]
fn nested_dirs_are_created_on_the_way() {
    let dir = tempdir("nested").join("deep/deeper");
    let (exit, report) = init(&dir, InitTemplate::Minimal, false);
    assert_eq!(exit, ExitClass::Ok);
    assert_eq!(report, CREATED);
    assert!(dir.join(TREE[0]).is_file());
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

/// P4-D4 CI teeth: scaffold → `validate` exits 0 → `dsl2json` matches
/// the golden (masked). Template drift fails the snapshot.
#[allow(clippy::too_many_lines)] // the inline goldens ARE the content
#[test]
fn scaffolded_workspaces_validate_and_match_the_json_golden() {
    let mut snapshots = Vec::new();
    for (template, tag) in [(InitTemplate::Minimal, "min"), (InitTemplate::C4, "c4")] {
        let dir = tempdir(&format!("golden-{tag}"));
        let (exit, _) = init(&dir, template, false);
        assert_eq!(exit, ExitClass::Ok);
        let ws = dir.join("workspace.dsl");

        let out = execute(
            Request::Validate {
                files: vec![ws.clone()],
            },
            &opts(),
        );
        assert_eq!(out.exit, ExitClass::Ok, "{tag}: scaffold must validate");

        let out = execute(
            Request::Dsl2Json {
                input: Input::File(ws),
                output: None,
            },
            &opts(),
        );
        assert_eq!(out.exit, ExitClass::Ok);
        let mut doc: serde_json::Value = serde_json::from_slice(&out.bytes.unwrap()).unwrap();
        strip_structurizr_props(&mut doc);
        snapshots.push(format!(
            "### {tag}\n{}\n",
            serde_json::to_string_pretty(&doc).unwrap()
        ));
    }
    let joined = snapshots.join("\n");
    // bless with UPDATE_EXPECT=1 after an intentional template change.
    expect![[r##"
        ### min
        {
          "configuration": {},
          "description": "A starter workspace created by `oxidizr init`.",
          "documentation": {},
          "id": 0,
          "model": {
            "people": [
              {
                "description": "A user of my software system.",
                "id": "1",
                "name": "User",
                "properties": {},
                "relationships": [
                  {
                    "description": "Uses",
                    "destinationId": "2",
                    "id": "3",
                    "sourceId": "1",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Person"
              }
            ],
            "softwareSystems": [
              {
                "description": "The software system I want to describe.",
                "documentation": {},
                "id": "2",
                "name": "My Software System",
                "properties": {},
                "tags": "Element,Software System"
              }
            ]
          },
          "name": "My Workspace",
          "properties": {},
          "views": {
            "configuration": {
              "styles": {
                "elements": [
                  {
                    "background": "#08427b",
                    "color": "#ffffff",
                    "shape": "Person",
                    "tag": "Person"
                  },
                  {
                    "background": "#1168bd",
                    "color": "#ffffff",
                    "tag": "Software System"
                  }
                ]
              },
              "terminology": {}
            },
            "systemLandscapeViews": [
              {
                "automaticLayout": {
                  "edgeSeparation": 50,
                  "nodeSeparation": 50,
                  "rankDirection": "LeftRight",
                  "rankSeparation": 100,
                  "vertices": true
                },
                "description": "The big picture of my software system.",
                "elements": [
                  {
                    "id": "1",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "2",
                    "x": 0,
                    "y": 0
                  }
                ],
                "enterpriseBoundaryVisible": true,
                "key": "landscape",
                "name": "System Landscape View",
                "order": 1,
                "relationships": [
                  {
                    "id": "3"
                  }
                ],
                "title": "System Landscape"
              }
            ]
          }
        }

        ### c4
        {
          "configuration": {},
          "description": "A starter C4 system landscape created by `oxidizr init --template c4`.",
          "documentation": {},
          "id": 0,
          "model": {
            "people": [
              {
                "description": "A customer of the web shop.",
                "id": "1",
                "name": "Customer",
                "properties": {},
                "relationships": [
                  {
                    "description": "Browses and buys products via",
                    "destinationId": "2",
                    "id": "8",
                    "sourceId": "1",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Person"
              }
            ],
            "softwareSystems": [
              {
                "containers": [
                  {
                    "description": "Provides the shop to customers via their web browser.",
                    "documentation": {},
                    "id": "3",
                    "name": "Web Application",
                    "properties": {},
                    "relationships": [
                      {
                        "description": "Reads from and writes to",
                        "destinationId": "4",
                        "id": "5",
                        "sourceId": "3",
                        "tags": "Relationship"
                      }
                    ],
                    "tags": "Element,Container",
                    "technology": "JavaScript"
                  },
                  {
                    "description": "Stores products, customers and orders.",
                    "documentation": {},
                    "id": "4",
                    "name": "Database",
                    "properties": {},
                    "tags": "Element,Container",
                    "technology": "PostgreSQL"
                  }
                ],
                "description": "Allows customers to browse and buy products.",
                "documentation": {},
                "id": "2",
                "name": "Web Shop",
                "properties": {},
                "relationships": [
                  {
                    "description": "Makes payments through",
                    "destinationId": "6",
                    "id": "9",
                    "sourceId": "2",
                    "tags": "Relationship"
                  },
                  {
                    "description": "Sends order confirmations through",
                    "destinationId": "7",
                    "id": "10",
                    "sourceId": "2",
                    "tags": "Relationship"
                  }
                ],
                "tags": "Element,Software System"
              },
              {
                "description": "Third-party provider that processes card payments.",
                "documentation": {},
                "id": "6",
                "name": "Payment Provider",
                "properties": {},
                "tags": "Element,Software System,External System"
              },
              {
                "description": "Sends order confirmations to customers.",
                "documentation": {},
                "id": "7",
                "name": "E-mail System",
                "properties": {},
                "tags": "Element,Software System,External System"
              }
            ]
          },
          "name": "Web Shop",
          "properties": {},
          "views": {
            "configuration": {
              "styles": {
                "elements": [
                  {
                    "background": "#23a2d7",
                    "color": "#ffffff",
                    "tag": "Container"
                  },
                  {
                    "background": "#999999",
                    "color": "#ffffff",
                    "tag": "External System"
                  },
                  {
                    "background": "#08427b",
                    "color": "#ffffff",
                    "shape": "Person",
                    "tag": "Person"
                  },
                  {
                    "background": "#1168bd",
                    "color": "#ffffff",
                    "tag": "Software System"
                  }
                ]
              },
              "terminology": {}
            },
            "containerViews": [
              {
                "automaticLayout": {
                  "edgeSeparation": 50,
                  "nodeSeparation": 50,
                  "rankDirection": "LeftRight",
                  "rankSeparation": 100,
                  "vertices": true
                },
                "description": "The containers inside the web shop.",
                "elements": [
                  {
                    "id": "3",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "4",
                    "x": 0,
                    "y": 0
                  }
                ],
                "externalSoftwareSystemBoundariesVisible": false,
                "key": "containers",
                "name": "Container View: Web Shop",
                "order": 3,
                "relationships": [
                  {
                    "id": "5"
                  }
                ],
                "softwareSystemId": "2",
                "title": "Containers"
              }
            ],
            "systemContextViews": [
              {
                "automaticLayout": {
                  "edgeSeparation": 50,
                  "nodeSeparation": 50,
                  "rankDirection": "LeftRight",
                  "rankSeparation": 100,
                  "vertices": true
                },
                "description": "The web shop and the users and systems it interacts with.",
                "elements": [
                  {
                    "id": "1",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "2",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "6",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "7",
                    "x": 0,
                    "y": 0
                  }
                ],
                "enterpriseBoundaryVisible": true,
                "key": "context",
                "name": "System Context View: Web Shop",
                "order": 2,
                "relationships": [
                  {
                    "id": "10"
                  },
                  {
                    "id": "8"
                  },
                  {
                    "id": "9"
                  }
                ],
                "softwareSystemId": "2",
                "title": "System Context"
              }
            ],
            "systemLandscapeViews": [
              {
                "automaticLayout": {
                  "edgeSeparation": 50,
                  "nodeSeparation": 50,
                  "rankDirection": "LeftRight",
                  "rankSeparation": 100,
                  "vertices": true
                },
                "description": "The big picture: the web shop and the systems it depends on.",
                "elements": [
                  {
                    "id": "1",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "2",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "6",
                    "x": 0,
                    "y": 0
                  },
                  {
                    "id": "7",
                    "x": 0,
                    "y": 0
                  }
                ],
                "enterpriseBoundaryVisible": true,
                "key": "landscape",
                "name": "System Landscape View",
                "order": 1,
                "relationships": [
                  {
                    "id": "10"
                  },
                  {
                    "id": "8"
                  },
                  {
                    "id": "9"
                  }
                ],
                "title": "System Landscape"
              }
            ]
          }
        }
    "##]]
    .assert_eq(&joined);
}
