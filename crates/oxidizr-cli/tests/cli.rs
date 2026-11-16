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

/// Temp-dir helper for the `reconcile` end-to-end fixtures.
fn recon_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-recon-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn reconcile_remaps_geometry_by_identifier() {
    // Layout (pre-edit): person Bob is old id "2". Edited DSL inserts two
    // systems first — Bob re-parses as id "3" — but his `structurizr.dsl.
    // identifier` is stable, so the view geometry must follow him there.
    let dsl = recon_temp("edited.dsl");
    std::fs::write(
        &dsl,
        "workspace \"W\" {\n    model {\n        z1 = softwareSystem \"Z1\"\n        z2 = softwareSystem \"Z2\"\n        bob = person \"Bob\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include bob\n        }\n    }\n}\n",
    )
    .unwrap();
    let layout = recon_temp("layout.json");
    std::fs::write(
        &layout,
        r#"{
  "configuration": { },
  "id": 0,
  "model": {
    "people": [ {
      "id": "2",
      "name": "Bob",
      "properties": { "structurizr.dsl.identifier": "bob" },
      "tags": "Element,Person"
    } ]
  },
  "name": "W",
  "views": {
    "systemLandscapeViews": [ {
      "elements": [ { "id": "2", "x": 777, "y": 778 } ],
      "enterpriseBoundaryVisible": true,
      "key": "L",
      "relationships": [ ]
    } ]
  }
}"#,
    )
    .unwrap();
    let out_json = recon_temp("out-clean.json");
    let out = run(
        &[
            "reconcile",
            "--dsl",
            dsl.to_str().unwrap(),
            "--layout",
            layout.to_str().unwrap(),
            "--output",
            out_json.to_str().unwrap(),
        ],
        None,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("reconciled 1 view(s)"), "{stderr}");
    let json = std::fs::read_to_string(&out_json).unwrap();
    assert!(json.contains("\"x\" : 777"), "{json}");
    // The grafted entry carries the NEW id, not the layout's old one.
    assert!(json.contains("\"id\" : \"3\""), "{json}");
}

#[test]
fn reconcile_ambiguous_contest_emits_json_and_exits_1() {
    // The edited system took the FIRST old element's identifier and the
    // SECOND's name — identifier and canonical-name lookups both land on the
    // single new id. Contest: OXZ-3002, never guessed — JSON is still
    // emitted, geometry is not, exit is 1.
    let dsl = recon_temp("contested.dsl");
    std::fs::write(
        &dsl,
        "workspace \"W\" {\n    model {\n        x = softwareSystem \"Bar\"\n    }\n    views {\n        systemLandscape \"L\" {\n            include x\n        }\n    }\n}\n",
    )
    .unwrap();
    let layout = recon_temp("contested.json");
    std::fs::write(
        &layout,
        r#"{
  "configuration": { },
  "id": 0,
  "model": {
    "softwareSystems": [
      {
        "id": "1",
        "name": "Foo",
        "properties": { "structurizr.dsl.identifier": "x" },
        "tags": "Element,Software System"
      },
      {
        "id": "2",
        "name": "Bar",
        "properties": { "structurizr.dsl.identifier": "y" },
        "tags": "Element,Software System"
      }
    ]
  },
  "name": "W",
  "views": {
    "systemLandscapeViews": [ {
      "elements": [
        { "id": "1", "x": 10, "y": 10 },
        { "id": "2", "x": 20, "y": 20 }
      ],
      "enterpriseBoundaryVisible": true,
      "key": "L",
      "relationships": [ ]
    } ]
  }
}"#,
    )
    .unwrap();
    let out_json = recon_temp("out-contested.json");
    let out = run(
        &[
            "reconcile",
            "--dsl",
            dsl.to_str().unwrap(),
            "--layout",
            layout.to_str().unwrap(),
            "--output",
            out_json.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("ambiguous reconcile"), "{stderr}");
    assert!(stderr.contains("OXZ-3002"), "{stderr}");
    // The contract: the JSON is emitted even when ambiguous.
    let json = std::fs::read_to_string(&out_json).unwrap();
    assert!(json.contains("\"id\" : \"1\""), "{json}");
    assert!(
        !json.contains("\"x\" : 10") && !json.contains("\"x\" : 20"),
        "contested geometry must not graft: {json}"
    );
}

/// Temp-dir helper for the `fmt` end-to-end tests.
fn fmt_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-fmt-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn fmt_stdin_writes_canonical_output_to_stdout() {
    let source = b"workspace \"W\" {\n      model {\n  person p \"P\"   \n    }\n}\n";
    let want = b"workspace W {\n    model {\n        person p P\n    }\n}\n";
    let out = run(&["fmt", "-"], Some(source));
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, want);
    // Already canonical input is byte-stable (`fmt` output feeds `fmt`).
    let again = run(&["fmt", "-"], Some(want));
    assert_eq!(again.stdout, want);
}

#[test]
fn fmt_check_flags_unformatted_then_write_fixes_in_place() {
    let file = fmt_temp("check.dsl");
    let source = "a {\n      b\n}\n";
    std::fs::write(&file, source).unwrap();

    let out = run(&["fmt", "--check", file.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1), "unformatted must exit 1");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim_end(),
        file.display().to_string()
    );
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        source,
        "--check writes nothing"
    );

    let out = run(&["fmt", "--write", file.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "a {\n    b\n}\n");

    let out = run(&["fmt", "--check", file.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stdout.is_empty(), "quiet check: {:?}", out.stdout);

    // `--write` on a formatted file leaves it alone (mtime-stable rewrite).
    let before = std::fs::metadata(&file).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));
    let out = run(&["fmt", "--write", file.to_str().unwrap()], None);
    assert!(out.status.success());
    assert_eq!(
        std::fs::metadata(&file).unwrap().modified().unwrap(),
        before
    );
}

#[test]
fn fmt_default_prints_and_leaves_file_untouched() {
    let file = fmt_temp("stdout.dsl");
    std::fs::write(&file, "a {\n        b\n}\n").unwrap();
    let out = run(&["fmt", file.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stdout, b"a {\n    b\n}\n");
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        "a {\n        b\n}\n"
    );
}

#[test]
fn fmt_write_needs_a_file_not_stdin() {
    // Usage error (P1-D8 code 2): stdin has no in-place target.
    let out = run(&["fmt", "--write", "-"], Some(b"a\n"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("in-place"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn fmt_never_validates_formats_any_input() {
    // Invalid statements re-print token-identically; formatting still exits 0
    // (the same text fails to PARSE — but that is not fmt's business).
    let out = run(
        &["fmt", "-"],
        Some(b"this is not a workspace {{{{  &&&\n\t\n"),
    );
    assert_eq!(out.status.code(), Some(0));
    let again = run(&["fmt", "-"], Some(&out.stdout));
    assert_eq!(again.stdout, out.stdout, "fmt is idempotent");
}

#[test]
fn version_and_help_exit_0() {
    let out = run(&["--version"], None);
    assert!(out.status.success());
    let out = run(&["dsl2json", "--help"], None);
    assert!(out.status.success());
}
