//! `export` theme-wiring integration tests (Phase 2 M1-d, P2-D3): the
//! install chain reaches the parser (installed names) and the `loadThemes`
//! leg reaches the diagram branch of `export_cmd` — reference
//! `ExportCommand` order (theme failures precede the format's OXZ-5007).
//! The process environment never leaks: `env = Some(empty)` keeps the
//! hermetic contract, the `<ws-parent>/themes` probe is exercised on
//! purpose (it is the reference's workspace-local source).

#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_api::pipeline::{ApiOptions, LogItem, ParseFlags, Request, execute};
use oxidizr_diag::codes;

/// Hermetic options: empty env map (no `OXIDZR_THEMES_DIR`/`STRUCTURIZR_THEMES`
/// leak), no stdin, no network, no explicit themes dir.
fn opts() -> ApiOptions {
    ApiOptions {
        parse: ParseFlags {
            strict: false,
            keep_unknown: false,
        },
        env: Some(BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-m1d-{topic}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn valid_theme() -> &'static str {
    "{\"name\":\"tst\",\"elements\":[{\"tag\":\"Person\",\"background\":\"#08427b\"}]}"
}

fn workspace_dsl(theme_token: &str) -> String {
    format!(
        "workspace {{\n  model {{\n    s = softwareSystem \"S\"\n  }}\n  views {{\n    theme {theme_token}\n  }}\n}}\n"
    )
}

/// Write `<dir>/workspace.dsl`; return its path.
fn write_ws(dir: &Path, theme_token: &str) -> PathBuf {
    let ws = dir.join("workspace.dsl");
    std::fs::write(&ws, workspace_dsl(theme_token)).unwrap();
    ws
}

fn install_theme(dir: &Path, name: &str, content: &str) {
    let tdir = dir.join("themes").join(name);
    std::fs::create_dir_all(&tdir).unwrap();
    std::fs::write(tdir.join("theme.json"), content).unwrap();
}

fn issues(out: &oxidizr_api::pipeline::Outcome) -> Vec<(oxidizr_diag::Code, String)> {
    out.log
        .iter()
        .filter_map(|l| match l {
            LogItem::Issue(i) => Some((i.code, i.message.clone())),
            _ => None,
        })
        .collect()
}

fn exported(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == ext))
        .collect();
    files.sort();
    files
}

#[test]
fn installed_name_reaches_the_parser_and_json_export() {
    // `<ws-parent>/themes/probe/theme.json` → `theme probe` takes the
    // installed branch (stored verbatim, P2-D3); `-f json` is unaffected by
    // loadThemes (json is not a DiagramExporter format, reference order) and
    // the styles injection never serializes (P2-D2): exactly one "themes".
    let dir = temp_dir("installed-json");
    install_theme(&dir, "probe", valid_theme());
    let ws = write_ws(&dir, "probe");
    let out = execute(
        Request::Export {
            workspace: ws,
            format: "json".into(),
            output: None,
        },
        &opts(),
    );
    assert_eq!(out.exit, oxidizr_api::pipeline::ExitClass::Ok);
    let json = exported(&dir, "json").into_iter().next().unwrap();
    let text = std::fs::read_to_string(&json).unwrap();
    assert!(text.contains("\"themes\" : [ \"probe\" ]"), "{text}");
    assert_eq!(text.matches("\"themes\"").count(), 1, "{text}");
}

#[test]
fn url_theme_needs_allow_net_before_the_format_leg() {
    // `-f plantuml` (DiagramExporter branch): loadThemes runs BEFORE the
    // export, so the URL without `--allow-net` reports OXZ-5001, not the
    // OXZ-5007 the unlanded format would raise.
    let dir = temp_dir("url-5001");
    let ws = write_ws(&dir, "https://example.invalid/themes/mine/theme.json");
    let out = execute(
        Request::Export {
            workspace: ws,
            format: "plantuml".into(),
            output: None,
        },
        &opts(),
    );
    let first = issues(&out).into_iter().next().expect("one issue");
    assert_eq!(first.0, codes::NET_DISABLED);
    assert_eq!(out.exit, oxidizr_api::pipeline::ExitClass::Diagnostics);
}

#[test]
fn broken_installed_theme_is_5008() {
    // Installed name resolves in the registry (parse keeps it) but the file
    // is unparsable → the loadThemes leg reports OXZ-5008 ahead of 5007.
    let dir = temp_dir("broken-5008");
    install_theme(&dir, "probe", "not json {{{");
    let ws = write_ws(&dir, "probe");
    let out = execute(
        Request::Export {
            workspace: ws,
            format: "plantuml".into(),
            output: None,
        },
        &opts(),
    );
    let first = issues(&out).into_iter().next().expect("one issue");
    assert_eq!(first.0, codes::EXPORT_THEME);
    assert_eq!(out.exit, oxidizr_api::pipeline::ExitClass::Diagnostics);
}

#[test]
fn json_export_never_loads_themes() {
    // The reference only runs loadThemes for DiagramExporter formats: a URL
    // theme exports `-f json` cleanly (exit 0), URLs stay stored verbatim.
    let dir = temp_dir("json-noload");
    let ws = write_ws(&dir, "https://example.invalid/themes/mine/theme.json");
    let out = execute(
        Request::Export {
            workspace: ws,
            format: "json".into(),
            output: None,
        },
        &opts(),
    );
    assert_eq!(out.exit, oxidizr_api::pipeline::ExitClass::Ok);
    assert_eq!(issues(&out), Vec::new());
    let json = exported(&dir, "json").into_iter().next().unwrap();
    let text = std::fs::read_to_string(&json).unwrap();
    assert!(
        text.contains("\"themes\" : [ \"https://example.invalid/themes/mine/theme.json\" ]"),
        "{text}"
    );
}
