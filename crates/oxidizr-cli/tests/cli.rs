//! P1-M6-a CLI end-to-end tests (Category A): the P1-D8 contract — commands,
//! `-` stdin/stdout, byte-exact outputs, exit codes (`0`/`1`/`2`/`3`),
//! miette-rendered `OXZ` diagnostics, D3 `--keep-unknown`, and the
//! production JSON-base loader injection.
#![allow(clippy::unwrap_used)]

use std::io::Write as _;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .canonicalize()
        .unwrap()
}

fn oxidizr() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_oxidizr"));
    cmd.env("NO_COLOR", "1"); // deterministic (ASCII) stderr
    cmd
}

fn run(args: &[&str], stdin: Option<&[u8]>) -> Output {
    let mut cmd = oxidizr();
    cmd.args(args);
    if let Some(bytes) = stdin {
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.as_mut().unwrap().write_all(bytes).unwrap();
        child.wait_with_output().unwrap()
    } else {
        cmd.stdin(Stdio::null()).output().unwrap()
    }
}

#[test]
fn dsl2json_file_matches_golden_bytes() {
    let dsl = testdata().join("dsl/extend/4.dsl");
    let out = run(&["dsl2json", dsl.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let golden = std::fs::read(testdata().join("golden/4.json")).unwrap();
    assert_eq!(out.stdout, golden, "CLI output must be golden byte-exact");
}

#[test]
fn dsl2json_stdin_matches_file_path() {
    // Self-contained source: with stdin there is no file context, so any
    // relative `workspace extends`/`!include` would (reference-faithfully)
    // skip and the byte comparison would not hold.
    let source = b"workspace \"Stdin\" {\n    model {\n        person \"P\" \"p desc\"\n    }\n}\n";
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-stdin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("ws.dsl");
    std::fs::write(&file, source).unwrap();
    let via_stdin = run(&["dsl2json", "-"], Some(source));
    let via_file = run(&["dsl2json", file.to_str().unwrap()], None);
    assert!(
        via_stdin.status.success(),
        "{}",
        String::from_utf8_lossy(&via_stdin.stderr)
    );
    assert!(
        via_file.status.success(),
        "{}",
        String::from_utf8_lossy(&via_file.stderr)
    );
    // `structurizr.dsl.identifier` carries a random UUID (fresh per parse) —
    // drop those lines, then the two pipeline paths must agree byte-wise.
    let strip = |bytes: &[u8]| -> String {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|l| !l.contains("structurizr.dsl.identifier"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(strip(&via_stdin.stdout), strip(&via_file.stdout));
}

#[test]
fn dsl2json_json_base_loader_is_injected() {
    // `workspace extends workspace.json` only resolves when the CLI wires
    // oxidizr-json as the engine's loader (topology P1-D1); stdin would hit
    // the `getFile() == null` skip, so this must be a file input.
    let dsl = testdata().join("dsl/extend/extend-workspace-from-json-file.dsl");
    let out = run(&["dsl2json", dsl.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("\"Software System 2\""), "{text}");
}

#[test]
fn dsl2json_output_file_round_trips() {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let json = dir.join("out.json");
    let dsl = testdata().join("dsl/extend/4.dsl");
    let out = run(
        &[
            "dsl2json",
            dsl.to_str().unwrap(),
            "-o",
            json.to_str().unwrap(),
        ],
        None,
    );
    assert!(out.status.success() && out.stdout.is_empty());
    let bytes = std::fs::read(&json).unwrap();
    let golden = std::fs::read(testdata().join("golden/4.json")).unwrap();
    assert_eq!(bytes, golden);
}

#[test]
fn json2json_parity_is_byte_identity_on_canonical_bytes() {
    let golden = std::fs::read(testdata().join("golden/m3-order.json")).unwrap();
    let out = run(&["json2json", "-"], Some(&golden));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, golden);
}

#[test]
fn json2json_keep_unknown_preserves_legacy_members() {
    // D3: the read-corpus v5-era workspace carries members the reference
    // reader drops (`revision`; `lastModified*` are KNOWN fields and
    // round-trip in both modes). Default mode drops `revision`,
    // `--keep-unknown` preserves it verbatim — and the output still reads.
    let file = testdata().join("read-corpus/structurizr-36141-workspace.json");
    let bytes = std::fs::read(&file).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(
        text.contains("\"revision\""),
        "fixture must carry legacy keys"
    );
    assert!(
        text.contains("\"lastModifiedDate\""),
        "fixture must carry known lastModified fields"
    );

    let plain = run(&["json2json", "-"], Some(&bytes));
    assert!(
        plain.status.success(),
        "{}",
        String::from_utf8_lossy(&plain.stderr)
    );
    let plain = String::from_utf8(plain.stdout).unwrap();
    assert!(
        !plain.contains("\"revision\""),
        "default mode must drop legacy keys"
    );
    assert!(
        plain.contains("\"lastModifiedDate\""),
        "known fields round-trip in parity mode"
    );

    let kept = run(&["--keep-unknown", "json2json", "-"], Some(&bytes));
    assert!(
        kept.status.success(),
        "{}",
        String::from_utf8_lossy(&kept.stderr)
    );
    let kept_text = String::from_utf8(kept.stdout).unwrap();
    assert!(
        kept_text.contains("\"revision\""),
        "keep mode must preserve legacy keys"
    );
    assert!(kept_text.contains("\"lastModifiedDate\""));
    // Superset output is still valid workspace JSON:
    let again = run(&["json2json", "-"], Some(kept_text.as_bytes()));
    assert!(
        again.status.success(),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
}

#[test]
fn validate_ok_and_quiet() {
    let dsl = testdata().join("dsl/source-parent.dsl");
    let p = dsl.to_str().unwrap();
    let out = run(&["validate", p], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("source-parent.dsl: ok"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let quiet = run(&["--quiet", "validate", p], None);
    assert!(quiet.status.success());
    assert!(quiet.stdout.is_empty(), "--quiet suppresses the ok line");
}

#[test]
fn validate_detects_format_by_extension() {
    // Unknown extension: a policy diagnostic (OXZ-5006), exit 1.
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-ext-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let odd = dir.join("workspace.txt");
    std::fs::write(&odd, "workspace {\n}\n").unwrap();
    let out = run(&["validate", odd.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXZ-5006"), "{stderr}");
    assert!(
        stderr.contains("cannot determine the workspace format"),
        "{stderr}"
    );
}

#[test]
fn parse_errors_exit_1_with_reference_text_and_code() {
    // D2 JVM-only directive: first recorded error is OXZ-5002 (M4 register).
    let dsl = testdata().join("dsl/plugin-with-parameters.dsl");
    let out = run(&["dsl2json", dsl.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXZ-5002"), "{stderr}");
    assert!(
        stderr.contains("!plugin is not supported by oxidizr"),
        "{stderr}"
    );
    // Reference exception text: `… at line N of <abs path>`.
    assert!(stderr.contains("at line "), "{stderr}");
    assert!(out.stdout.is_empty(), "no partial output on failed parse");
}

#[test]
fn json_syntax_error_exits_1_with_offset_snippet() {
    let bad = b"{ \"name\" : \"x\", oops }\n";
    let out = run(&["json2json", "-"], Some(bad.as_slice()));
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXZ-4001"), "{stderr}");
    assert!(stderr.contains("failed to parse JSON"), "{stderr}");
    // The snippet renders the input line:
    assert!(stderr.contains("oops"), "{stderr}");
}

#[test]
fn unreadable_input_exits_3() {
    let out = run(
        &["dsl2json", "/nonexistent/oxidizr-does-not-exist.dsl"],
        None,
    );
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXZ-5006"), "{stderr}");
}

#[test]
fn non_utf8_input_exits_3() {
    // V-8 resolved: UTF-8 only; the vendored latin-1 fixture fails loudly.
    let iso = testdata().join("dsl/iso-8859.dsl");
    let out = run(&["dsl2json", iso.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(3));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("not valid UTF-8"), "{stderr}");
}

#[test]
fn usage_errors_exit_2() {
    let out = run(&["merge"], None);
    assert_eq!(out.status.code(), Some(2));
    let out = run(&["validate"], None); // needs at least one file
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn unreached_milestones_bail_with_plan_refs() {
    let out = run(&["fmt", "x.dsl"], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("M6-b"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["reconcile", "--dsl", "e.dsl", "--layout", "l.json"], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("M6-c"));
}

#[test]
fn version_and_help_exit_0() {
    let out = run(&["--version"], None);
    assert!(out.status.success());
    let out = run(&["dsl2json", "--help"], None);
    assert!(out.status.success());
}
