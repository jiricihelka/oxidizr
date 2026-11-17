//! Default-views load parity (Phase 2 M1-e-B): `AbstractCommand.loadWorkspace`
//! creates default views when the model is non-empty but the view set is
//! empty — the tail sits on the SHARED load, so `dsl2json`, `json2json` and
//! the `export` loader all see the same views the reference exports. An
//! empty model or any explicitly defined view switches it off; the
//! `keep_unknown` superset leg (D3, documented non-parity) never applies it.

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::PathBuf;

use oxidizr_api::pipeline::{ApiOptions, Input, Request, execute};

/// Hermetic options (same contract as the M1-d export tests).
fn opts() -> ApiOptions {
    ApiOptions {
        strict: false,
        keep_unknown: false,
        env: Some(BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
    }
}

fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m1e-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `(key, kind)` of every view in a serialized workspace JSON, in writer
/// order — shape assertions (the retained-DSL identifier UUIDs are random
/// per process, bytes are never comparable).
fn view_shapes(bytes: &[u8]) -> Vec<(String, String)> {
    let ws = oxidizr_json::from_slice(bytes).unwrap();
    ws.views
        .all_views()
        .iter()
        .map(|v| (v.key().to_string(), v.kind_label().to_string()))
        .collect()
}

const NO_VIEWS_DSL: &str = concat!(
    "workspace {\n",
    "  model {\n",
    "    p = person \"P\"\n",
    "    s = softwareSystem \"S\"\n",
    "    p -> s \"Uses\"\n",
    "  }\n",
    "  views {\n",
    "  }\n",
    "}\n"
);

#[test]
fn dsl2json_no_views_gets_default_landscape_and_context() {
    let out = execute(
        Request::Dsl2Json {
            input: Input::Bytes(NO_VIEWS_DSL.as_bytes().to_vec()),
            output: None,
        },
        &opts(),
    );
    let bytes = out.bytes.unwrap();
    assert_eq!(
        view_shapes(&bytes),
        vec![
            (
                "SystemLandscape-001".to_string(),
                "system landscape view".into()
            ),
            (
                "SystemContext-001".to_string(),
                "system context view".into()
            ),
        ],
        "the reference `createDefaultViews` tail must materialize on dsl2json"
    );
    // generated keys are marked exactly like the reference (`generatedKey`)
    let ws = oxidizr_json::from_slice(&bytes).unwrap();
    assert!(
        ws.views.all_views().iter().all(|v| v.base().generated_key),
        "auto-generated keys carry generatedKey: true"
    );
}

#[test]
fn dsl2json_explicit_views_switch_the_tail_off() {
    let dsl = "workspace {\n  model {\n    s = softwareSystem \"S\"\n  }\n  views {\n    systemLandscape \"l1\" \"L\" {\n    }\n  }\n}\n";
    let out = execute(
        Request::Dsl2Json {
            input: Input::Bytes(dsl.as_bytes().to_vec()),
            output: None,
        },
        &opts(),
    );
    assert_eq!(
        view_shapes(&out.bytes.unwrap()),
        vec![("l1".to_string(), "system landscape view".into())],
        "one explicit view = non-empty view set = no default views"
    );
}

#[test]
fn dsl2json_empty_model_gets_no_views() {
    let dsl = "workspace {\n  model {\n  }\n  views {\n  }\n}\n";
    let out = execute(
        Request::Dsl2Json {
            input: Input::Bytes(dsl.as_bytes().to_vec()),
            output: None,
        },
        &opts(),
    );
    assert!(
        view_shapes(&out.bytes.unwrap()).is_empty(),
        "`model.isEmpty()` short-circuits the tail (people + systems + \
         deployment nodes only)"
    );
}

#[test]
fn json2json_parity_leg_gets_the_tail() {
    // A JSON workspace whose model holds a system but no views — the
    // reference JSON leg of `loadWorkspace` creates default views too.
    let json = r#"{"configuration":{},"model":{"softwareSystems":[{"id":"1","name":"S","description":"","tags":"Software System"}]},"views":{}}"#;
    let out = execute(
        Request::Json2Json {
            input: Input::Bytes(json.as_bytes().to_vec()),
            output: None,
        },
        &opts(),
    );
    assert_eq!(
        view_shapes(&out.bytes.unwrap()),
        vec![
            (
                "SystemLandscape-001".to_string(),
                "system landscape view".into()
            ),
            (
                "SystemContext-001".to_string(),
                "system context view".into()
            ),
        ]
    );
}

#[test]
fn json2json_keep_unknown_stays_verbatim() {
    let json = r#"{"configuration":{},"model":{"softwareSystems":[{"id":"1","name":"S","description":"","tags":"Software System"}]},"views":{}}"#;
    let out = execute(
        Request::Json2Json {
            input: Input::Bytes(json.as_bytes().to_vec()),
            output: None,
        },
        &ApiOptions {
            keep_unknown: true,
            ..opts()
        },
    );
    assert!(
        view_shapes(&out.bytes.unwrap()).is_empty(),
        "D3 superset mode is documented non-parity — no default views"
    );
}

#[test]
fn export_json_leg_materializes_default_views() {
    // The witness case (`m1d` probe): `export --format json` on a no-views
    // DSL workspace — the reference output carries the default views.
    let dir = temp_dir("export");
    let ws = dir.join("workspace.dsl");
    std::fs::write(&ws, NO_VIEWS_DSL).unwrap();
    let out = execute(
        Request::Export {
            workspace: ws.clone(),
            format: "json".to_string(),
            output: None,
        },
        &opts(),
    );
    assert_eq!(out.exit, oxidizr_api::pipeline::ExitClass::Ok);
    let produced = dir.join("workspace.json");
    let bytes = std::fs::read(&produced).unwrap();
    assert_eq!(
        view_shapes(&bytes),
        vec![
            (
                "SystemLandscape-001".to_string(),
                "system landscape view".into()
            ),
            (
                "SystemContext-001".to_string(),
                "system context view".into()
            ),
        ]
    );
}
