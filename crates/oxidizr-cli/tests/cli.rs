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
        // A child that exits BEFORE consuming stdin (usage errors) closes the
        // pipe: EPIPE here is expected, the exit status is the assertion.
        let wr = child.stdin.as_mut().unwrap().write_all(bytes);
        assert!(wr.is_ok() || wr.unwrap_err().kind() == std::io::ErrorKind::BrokenPipe);
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
fn validate_renders_did_you_mean_suggestions() {
    // D1 polish (P1-D2): OXZ-2001 suggests from the expected-token list,
    // the identifier families from the parsed workspace's known names.
    let dsl = recon_temp("suggest.dsl");
    std::fs::write(
        &dsl,
        "workspace \"W\" {\n    model {\n        api = softwareSystem \"API\"\n        softwareSyste \"X\"\n        api -> ap \"Uses\"\n    }\n}\n",
    )
    .unwrap();
    let out = run(&["validate", dsl.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("did you mean \"softwareSystem\"?"),
        "{stderr}"
    );
    assert!(stderr.contains("did you mean \"api\"?"), "{stderr}");
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

// §P3-M8 / F10 — the `view` subcommand is stable `--help` surface in
// EVERY build; only its runtime behavior is feature-gated.
#[test]
fn view_help_is_stable_surface() {
    let out = run(&["view", "--help"], None);
    assert!(out.status.success(), "`view` is advertised without `gui`");
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--theme"), "`--theme` documented: {help}");
}

#[cfg(not(feature = "gui"))]
#[test]
fn view_without_gui_feature_exits_1_with_rebuild_hint() {
    // Default (no `gui`) build: valid argv, but no shell is linked — the
    // request is unserveable (P1-D8 exit 1) with a rebuild hint.
    let out = run(&["view"], None);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("gui"), "hint names the feature: {err}");
    assert!(
        err.contains("cargo build --features gui"),
        "hint names the flag: {err}"
    );
}

// §P4-M0 / F3 — `oxidizr diff` (D-3): exit codes 0/1/3, the human ± report,
// the schema-1 JSON document, `--quiet` (exit code only) and `--ignore-layout`.
// Self-contained fixtures in a temp dir (the `cli-stdin` precedent); NO_COLOR
// is set by `oxidizr()`, so the report bytes are the PLAIN rendering.

fn diff_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-diff-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

const DIFF_A: &str = "workspace \"D\" {\n    model {\n        person \"Alice\" \"CEO\"\n    }\n}\n";
const DIFF_B: &str = "workspace \"D\" {\n    model {\n        person \"Alice\" \"CEO\"\n        person \"Bob\"\n    }\n}\n";

fn diff_pair() -> (PathBuf, PathBuf) {
    let (a, b) = (diff_temp("a.dsl"), diff_temp("b.dsl"));
    std::fs::write(&a, DIFF_A).unwrap();
    std::fs::write(&b, DIFF_B).unwrap();
    (a, b)
}

#[test]
fn diff_identical_exits_zero_silent() {
    let a = diff_temp("same.dsl");
    std::fs::write(&a, DIFF_A).unwrap();
    let out = run(&["diff", a.to_str().unwrap(), a.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(0), "equal sides exit 0");
    assert!(out.stdout.is_empty(), "human format is silent when equal");
}

#[test]
fn diff_human_report_exit_one_golden() {
    let (a, b) = diff_pair();
    let out = run(&["diff", a.to_str().unwrap(), b.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1), "differences found — P4-D1");
    // The default `SystemLandscape-001` view is generated on both sides, so
    // membership and the new element's position follow the model leg.
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "added (1):\n\
         + model.people[Bob] (person)\n\
         changed (2):\n\
         ~ views[SystemLandscape-001] elements: [\"Person://Alice\"] -> [\"Person://Alice\",\"Person://Bob\"]\n\
         ~ views[SystemLandscape-001] position[Bob]: null -> {\"x\":0,\"y\":0}\n"
    );
}

#[test]
fn diff_json_schema_document_exit_one() {
    let (a, b) = diff_pair();
    let out = run(
        &[
            "diff",
            "--format",
            "json",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    // The frozen schema-1 shape (docs/diff-schema.md): stable key order,
    // all categories present, the added entry first. Provenance stamps
    // (`structurizr.dsl*`) never appear.
    assert!(
        text.starts_with("{\n  \"schema\": 1,\n  \"added\": [\n"),
        "{text}"
    );
    assert!(text.contains("\"path\": \"model.people[Bob]\""), "{text}");
    assert!(text.contains("\"idChanged\": []\n}\n"), "{text}");
    assert!(!text.contains("structurizr.dsl"), "{text}");
}

#[test]
fn diff_quiet_is_exit_code_only() {
    let (a, b) = diff_pair();
    let out = run(
        &["--quiet", "diff", a.to_str().unwrap(), b.to_str().unwrap()],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"");
    assert_eq!(out.stderr, b"");
}

#[test]
fn diff_ignore_layout_skips_geometry() {
    let (a, b) = diff_pair();
    let out = run(
        &[
            "diff",
            "--ignore-layout",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("+ model.people[Bob]"), "{text}");
    // Membership is not layout (§P4-M0): only the geometry field disappears.
    assert!(text.contains("elements"), "{text}");
    assert!(!text.contains("position[Bob]"), "{text}");
}

#[test]
fn diff_missing_side_exit_three() {
    let a = diff_temp("present.dsl");
    std::fs::write(&a, DIFF_A).unwrap();
    let missing = diff_temp("absent.dsl");
    let out = run(
        &["diff", a.to_str().unwrap(), missing.to_str().unwrap()],
        None,
    );
    assert_eq!(
        out.status.code(),
        Some(3),
        "unreadable side is I/O, not equality"
    );
}

// §P4-M1 / F-inspect-parity — `oxidizr inspect` (I-4): exit 0/1/2, the
// human `type | SEVERITY | message` report (NO_COLOR ⇒ PLAIN bytes), the
// schema-1 JSON document (docs/inspect-schema.md), the severity
// threshold, `--only`/`--except` and `--quiet`, and the `open` default-
// views leg (generated keys are REPORTED — the reference quirk).
// Self-contained fixtures in a temp dir (the `diff` precedent).

fn inspect_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-inspect-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn inspect_fixture(name: &str, source: &str) -> PathBuf {
    let path = inspect_temp(name);
    std::fs::write(&path, source).unwrap();
    path
}

/// Scope set (silences `workspace.scope`), one explicit-key landscape
/// view (prevents the default-views leg and silences noview) with a
/// single disconnected person.
const INSPECT_DIRTY: &str = "workspace \"I\" {\n    configuration {\n        scope softwaresystem\n    }\n    model {\n        person \"Alice\" \"CEO\"\n    }\n    views {\n        systemLandscape main {\n            include *\n        }\n    }\n}\n";

/// The same, with the disconnected severity downgraded on the element.
const INSPECT_WARN: &str = "workspace \"I\" {\n    configuration {\n        scope softwaresystem\n    }\n    model {\n        person \"Alice\" \"CEO\" {\n            properties {\n                \"structurizr.inspection.model.element.disconnected\" \"warning\"\n            }\n        }\n    }\n    views {\n        systemLandscape main {\n            include *\n        }\n    }\n}\n";

/// A fully clean workspace (documented, related, on a view).
const INSPECT_CLEAN: &str = "workspace \"I\" {\n    configuration {\n        scope softwaresystem\n    }\n    model {\n        alice = person \"Alice\" \"CEO\"\n        bank = softwareSystem \"Bank\" \"Banking system\"\n        alice -> bank \"Uses\" \"HTTPS\"\n    }\n    views {\n        systemLandscape main {\n            include *\n        }\n    }\n}\n";

/// No views at all: `open` creates the default views, whose keys ARE
/// automatically generated — the reference reports key + layout per
/// model view (landscape, then context).
const INSPECT_DEFAULT_VIEWS: &str = "workspace \"I\" {\n    configuration {\n        scope softwaresystem\n    }\n    model {\n        alice = person \"Alice\" \"CEO\"\n        bank = softwareSystem \"Bank\" \"Banking system\"\n        alice -> bank \"Uses\" \"HTTPS\"\n    }\n}\n";

#[test]
fn inspect_clean_workspace_exit_zero_silent() {
    let ws = inspect_fixture("clean.dsl", INSPECT_CLEAN);
    let out = run(&["inspect", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(0), "clean workspace");
    assert!(out.stdout.is_empty(), "human format is silent when clean");
}

#[test]
fn inspect_human_report_exit_one_golden() {
    let ws = inspect_fixture("dirty.dsl", INSPECT_DIRTY);
    let out = run(&["inspect", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1), "violations reported — P4-D1");
    // The message is the reference `toString` verbatim (terminology
    // lowercased, canonical name minus the `Type://` prefix).
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "model.element.disconnected | ERROR | The person \"Alice\" is disconnected - add a relationship to/from it, or consider removing it from the model.\n"
    );
}

#[test]
fn inspect_json_schema_document_exit_one() {
    let ws = inspect_fixture("dirty.dsl", INSPECT_DIRTY);
    let out = run(&["inspect", "--format", "json", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    // The frozen schema-1 shape (docs/inspect-schema.md): schema first,
    // run order, lowercase severity, message verbatim.
    assert!(
        text.starts_with("{\n  \"schema\": 1,\n  \"violations\": [\n"),
        "{text}"
    );
    assert!(
        text.contains("\"type\": \"model.element.disconnected\""),
        "{text}"
    );
    assert!(text.contains("\"severity\": \"error\""), "{text}");
    assert!(text.ends_with("    }\n  ]\n}\n"), "{text}");
}

#[test]
fn inspect_severity_threshold_filters() {
    let ws = inspect_fixture("warn.dsl", INSPECT_WARN);
    // The default threshold (`info`) keeps the WARNING…
    let out = run(&["inspect", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("| WARNING |"),
        "element property downgrades the severity"
    );
    // …and `error` filters it out (exit flips to 0 — exit counts only
    // REPORTED violations).
    let out = run(
        &[
            "inspect",
            "--severity-threshold",
            "error",
            ws.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"");
}

#[test]
fn inspect_only_except_filters() {
    let ws = inspect_fixture("dirty.dsl", INSPECT_DIRTY);
    // `--except` exact type match ⇒ clean.
    let out = run(
        &[
            "inspect",
            "--except",
            "model.element.disconnected",
            ws.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    // `--only` with a type that never fired ⇒ nothing reported.
    let out = run(
        &["inspect", "--only", "model.empty", ws.to_str().unwrap()],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    // `--only` matching keeps it.
    let out = run(
        &[
            "inspect",
            "--only",
            "model.element.disconnected",
            ws.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    // `--only` BEFORE `--except`: the same type in both ⇒ filtered.
    let out = run(
        &[
            "inspect",
            "--only",
            "model.element.disconnected",
            "--except",
            "model.element.disconnected",
            ws.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn inspect_quiet_is_exit_code_only() {
    let ws = inspect_fixture("dirty.dsl", INSPECT_DIRTY);
    let out = run(&["--quiet", "inspect", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"");
    assert_eq!(out.stderr, b"");
}

#[test]
fn inspect_default_views_report_generated_keys() {
    let ws = inspect_fixture("default-views.dsl", INSPECT_DEFAULT_VIEWS);
    let out = run(&["inspect", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(1));
    // `open` installs the default views; their keys ARE auto-generated
    // (the pinned `createDefaultViews`), and neither the landscape nor
    // the context view gets automatic layout — key + layout per view,
    // in `all_views` order.
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "views.view.key | ERROR | The view with key \"SystemLandscape-001\" has an automatically generated view key and this is not guaranteed to be stable over time.\n\
         views.view.layout | ERROR | The view with key \"SystemLandscape-001\" has an automatically generated view key and this may cause manual layout information to be lost in the future.\n\
         views.view.key | ERROR | The view with key \"SystemContext-001\" has an automatically generated view key and this is not guaranteed to be stable over time.\n\
         views.view.layout | ERROR | The view with key \"SystemContext-001\" has an automatically generated view key and this may cause manual layout information to be lost in the future.\n"
    );
}

#[test]
fn inspect_unknown_workspace_exit_three() {
    let missing = inspect_temp("absent.dsl");
    let out = run(&["inspect", missing.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(3), "unreadable input is I/O");
}

// §P4-M2 / F12 — `oxidizr import` (N-4): one diagram → a fresh
// workspace. The macro legs (C4-`PlantUML`, `Mermaid`) produce the
// MODEL only (the default views install at open — the read-check), the
// image leg is the reference `ImageImporter` port (D14: raw bytes
// embedded). Format by `--format` else extension sniff; skip notes ride
// stderr (warning-and-skip); unguessable formats exit 2. Self-contained
// fixtures in a temp dir (the `inspect` precedent).

fn import_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

fn import_fixture(name: &str, source: &str) -> PathBuf {
    let path = import_temp(name);
    std::fs::write(&path, source).unwrap();
    path
}

/// A real 1×1 PNG — the reference embeds the RAW bytes (D14) and the
/// `.png` extension picks `image/png` (`ImageUtils`).
fn import_png_fixture() -> PathBuf {
    let path = import_temp("dot.png");
    std::fs::write(
        &path,
        [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 207,
            192, 80, 15, 0, 4, 133, 1, 128, 132, 169, 140, 33, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66,
            96, 130,
        ],
    )
    .unwrap();
    path
}

const IMPORT_CONTEXT_PUML: &str = r#"Person(customer, "Personal Banking Customer", "A customer of the bank.")
System(banking, "Internet Banking System", "Allows customers to view information about their bank accounts.")
Rel(customer, banking, "Uses", "HTTPS")
"#;

const IMPORT_CONTEXT_PUML_GOLDEN: &str = r#"{
  "configuration" : { },
  "documentation" : { },
  "id" : 0,
  "model" : {
    "people" : [ {
      "description" : "A customer of the bank.",
      "id" : "1",
      "name" : "Personal Banking Customer",
      "relationships" : [ {
        "description" : "Uses",
        "destinationId" : "2",
        "id" : "3",
        "sourceId" : "1",
        "tags" : "Relationship",
        "technology" : "HTTPS"
      } ],
      "tags" : "Element,Person"
    } ],
    "softwareSystems" : [ {
      "description" : "Allows customers to view information about their bank accounts.",
      "documentation" : { },
      "id" : "2",
      "name" : "Internet Banking System",
      "tags" : "Element,Software System"
    } ]
  },
  "views" : {
    "configuration" : {
      "styles" : { },
      "terminology" : { }
    }
  }
}"#;

const IMPORT_DEMO_MMD: &str = r#"C4Context
title Demo
Person(customer, "Customer", "A person.")
System(banking, "Banking")
Rel(customer, banking, "Uses", "HTTPS")
bob --> banking
"#;

const IMPORT_DEMO_MMD_GOLDEN: &str = r#"{
  "configuration" : { },
  "documentation" : { },
  "id" : 0,
  "model" : {
    "people" : [ {
      "description" : "A person.",
      "id" : "1",
      "name" : "Customer",
      "relationships" : [ {
        "description" : "Uses",
        "destinationId" : "2",
        "id" : "3",
        "sourceId" : "1",
        "tags" : "Relationship",
        "technology" : "HTTPS"
      } ],
      "tags" : "Element,Person"
    } ],
    "softwareSystems" : [ {
      "documentation" : { },
      "id" : "2",
      "name" : "Banking",
      "tags" : "Element,Software System"
    } ]
  },
  "views" : {
    "configuration" : {
      "styles" : { },
      "terminology" : { }
    }
  }
}"#;

const PNG_DATA_URI: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

/// The generated-key form: `Image-001` + `generatedKey` (reference
/// `generateViewKey`), title and name both the file name.
fn image_golden(key: &str, generated: bool, title: &str) -> String {
    format!(
        r#"{{
  "configuration" : {{ }},
  "documentation" : {{ }},
  "id" : 0,
  "model" : {{ }},
  "views" : {{
    "configuration" : {{
      "styles" : {{ }},
      "terminology" : {{ }}
    }},
    "imageViews" : [ {{
      "content" : "{PNG_DATA_URI}",
      "contentType" : "image/png",{}
      "key" : "{key}",
      "name" : "{title}",
      "order" : 1,
      "title" : "{title}"
    }} ]
  }}
}}"#,
        if generated {
            "\n      \"generatedKey\" : true,"
        } else {
            ""
        }
    )
}

#[test]
fn import_c4plantuml_sniffed_extension_stdout() {
    let puml = import_fixture("context.puml", IMPORT_CONTEXT_PUML);
    let out = run(&["import", puml.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        IMPORT_CONTEXT_PUML_GOLDEN
    );
    assert_eq!(out.stderr, b"", "a clean diagram stays silent");
}

#[test]
fn import_mermaid_sniffed_extension_and_skip_note() {
    let mmd = import_fixture("demo.mmd", IMPORT_DEMO_MMD);
    let out = run(&["import", mmd.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        IMPORT_DEMO_MMD_GOLDEN
    );
    // warning-and-skip (P4-D8 R-1): the raw flowchart edge is a NOTE on
    // stderr, the model still imports, the exit stays 0.
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "skipped unsupported Mermaid relationship: bob --> banking\n"
    );
}

#[test]
fn import_unknown_macro_warns_and_skips() {
    let puml = import_fixture("unknown.puml", "Person(a, \"A\")\nFoo(bar)\n");
    let out = run(&["import", puml.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stderr).unwrap(),
        "unknown macro \"Foo\" — skipped\n"
    );
}

#[test]
fn import_image_reference_port_generated_key() {
    let png = import_png_fixture();
    let out = run(&["import", png.to_str().unwrap()], None);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        image_golden("Image-001", true, "dot.png")
    );
}

#[test]
fn import_image_view_key_and_name_options() {
    let png = import_png_fixture();
    let out = run(
        &[
            "import",
            png.to_str().unwrap(),
            "--format",
            "image",
            "--view-key",
            "k1",
            "--view-name",
            "My View",
        ],
        None,
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // An explicit key is no longer `generatedKey`; the title follows the
    // name override (the reference `view.setTitle` twist).
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        image_golden("k1", false, "My View")
    );
}

#[test]
fn import_output_file_matches_stdout_bytes() {
    let puml = import_fixture("context.puml", IMPORT_CONTEXT_PUML);
    let out_file = import_temp("context.json");
    let _ = std::fs::remove_file(&out_file);
    let out = run(
        &[
            "import",
            puml.to_str().unwrap(),
            "-o",
            out_file.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(out.stdout, b"", "-o moves the payload to the file");
    let written = std::fs::read(&out_file).unwrap();
    assert_eq!(
        String::from_utf8(written).unwrap(),
        IMPORT_CONTEXT_PUML_GOLDEN,
        "the file carries exactly the bytes stdout would"
    );
}

#[test]
fn import_usage_errors() {
    // Unguessable extension: the name decides BEFORE any I/O (the file
    // never needs to exist).
    let txt = import_temp("diagram.txt");
    let out = run(&["import", txt.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("OXZ-5006") && err.contains("cannot guess the import format of"),
        "{err}"
    );

    // A nameless input (stdin) has nothing to sniff.
    let out = run(&["import", "-"], Some(b"System(s, \"S\")\n"));
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("cannot guess the import format of stdin")
    );

    // The image leg reads the bytes off disk — no stdin form.
    let out = run(
        &["import", "-", "--format", "image"],
        Some(b"not really a png"),
    );
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("image import needs a file"));
}

#[test]
fn import_stdin_with_explicit_format() {
    let out = run(
        &["import", "-", "--format", "c4plantuml"],
        Some(b"System(s, \"S\")\n"),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains(r#""name" : "S""#));
}

/// Hermetic options for the read-check (the `oxidizr-api` `open` tests'
/// posture: empty env map, no stdin, no network).
fn import_opts() -> oxidizr_api::ApiOptions {
    oxidizr_api::ApiOptions {
        strict: false,
        keep_unknown: false,
        env: Some(std::collections::BTreeMap::new()),
        stdin: None,
        allow_net: false,
        themes_dir: None,
        color: false,
    }
}

#[test]
fn import_read_check_reopens_clean_with_default_views() {
    // The N-4 read-check: the imported JSON re-opens through
    // `oxidizr_api::open` with NOTHING to report, and the default views
    // install THERE (the model-only contract, N-2).
    let puml = import_fixture("context.puml", IMPORT_CONTEXT_PUML);
    let out_file = import_temp("read-check.json");
    let _ = std::fs::remove_file(&out_file);
    let out = run(
        &[
            "import",
            puml.to_str().unwrap(),
            "-o",
            out_file.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    let opened = oxidizr_api::open(&oxidizr_api::Input::File(out_file), &import_opts()).unwrap();
    assert!(
        opened.log.is_empty(),
        "the imported workspace re-opens clean"
    );
    assert_eq!(opened.workspace.model.software_systems.len(), 1);
    assert_eq!(opened.workspace.model.people.len(), 1);
    assert!(
        !opened.workspace.views.all_views().is_empty(),
        "createDefaultViews fires at open for the model-only import"
    );
}

// §P4-M3 / F6 — `oxidizr query` (O-1): the DSL filter grammar as a query
// language (`docs/query-schema.md`). Exit codes 0/1/2 only (the command
// is informational — zero matches stays 0, P4-D1 family); the `table`
// rows, `ids` lines and the schema-1 JSON document are pinned below on
// the vendored `testdata/probes/diff/base.dsl` fixture.

fn probe_ws(name: &str) -> PathBuf {
    testdata().join("probes/diff").join(name)
}

#[test]
fn query_table_rows_golden() {
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "element.tag==Element",
            "--kind",
            "element",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "1  Person  Person://Alice  Element,Person\n\
         2  Person  Person://Bob  Element,Person\n\
         3  SoftwareSystem  SoftwareSystem://Bank  Element,Software System\n\
         4  Container  Container://Bank.Web  Element,Container\n\
         5  Container  Container://Bank.DB  Element,Container\n\
         6  SoftwareSystem  SoftwareSystem://Mail  Element,Software System\n",
        "table = `id  kind  canonicalName  [tags]` (EFFECTIVE tags)"
    );
}

#[test]
fn query_ids_one_per_line_and_zero_matches_stays_zero() {
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "element.tag==Element",
            "--kind",
            "element",
            "--format",
            "ids",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "1\n2\n3\n4\n5\n6\n");
    // Zero matches is success with NO bytes (query is informational).
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "element.tag==NoSuchTag",
            "--format",
            "ids",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "zero matches: no bytes: {:?}",
        out.stdout
    );
}

#[test]
fn query_json_schema_document_golden() {
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "alice->bank",
            "--format",
            "json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "{\n  \"schema\": 1,\n  \"filter\": \"alice->bank\",\n  \"kind\": \"all\",\n  \
         \"elements\": [],\n  \"relationships\": [\n    {\n      \"id\": \"7\",\n      \
         \"kind\": \"Relationship\",\n      \
         \"canonicalName\": \"Relationship://Person://Alice -> SoftwareSystem://Bank \
         (Uses)\",\n      \"tags\": [\n        \"Relationship\"\n      ]\n    }\n  \
         ],\n  \"views\": []\n}\n",
        "schema-1 document (docs/query-schema.md), key order frozen"
    );
}

#[test]
fn query_view_subset_and_throwing_arm() {
    // The documented view subset (`key==…`) — the fixture's explicit-key
    // landscape view.
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "key==landscape",
            "--kind",
            "view",
            "--format",
            "ids",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), "landscape\n");
    // A THROWING reference arm (`element.parent==<unknown>`) exits 1 with
    // the reference message — no new diagnostic code appears.
    let out = run(
        &[
            "query",
            probe_ws("base.dsl").to_str().unwrap(),
            "--filter",
            "element.parent==nobody",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("The parent element \"nobody\" does not exist"),
        "{err}"
    );
    // Usage error (missing the required `--filter`) stays clap's 2.
    let out = run(&["query", probe_ws("base.dsl").to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(2));
}

// §P4-M3 / F7 — `oxidizr init` (O-2, P4-D4): the scaffold contract —
// created/skipped report on the five template files (relative paths in
// template ORDER, the next-commands footer), exit 0 for created AND
// skipped runs, `--force` overwrite, clap's 2 for a bad `--template`.
// The workspace-JSON goldens of both templates ride the API tier
// (`oxidizr-api/tests/init_probes.rs`).

fn init_temp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("oxidizr-cli-init-{}", std::process::id()))
        .join(tag);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

const INIT_CREATED: &str = "created workspace.dsl\n\
    created README.md\n\
    created .gitignore\n\
    created .editorconfig\n\
    created .vscode/settings.json\n\
    next steps:\n  \
    oxidizr validate workspace.dsl\n  \
    oxidizr export workspace.dsl -f plantuml\n";

const INIT_SKIPPED: &str = "skipped workspace.dsl (exists, use --force to overwrite)\n\
    skipped README.md (exists, use --force to overwrite)\n\
    skipped .gitignore (exists, use --force to overwrite)\n\
    skipped .editorconfig (exists, use --force to overwrite)\n\
    skipped .vscode/settings.json (exists, use --force to overwrite)\n\
    next steps:\n  \
    oxidizr validate workspace.dsl\n  \
    oxidizr export workspace.dsl -f plantuml\n";

#[test]
fn init_creates_then_skips_then_forces() {
    let dir = init_temp("cycle");
    let d = dir.to_str().unwrap();
    let out = run(&["init", d], None);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        INIT_CREATED,
        "c4 default"
    );
    // Idempotent re-run: everything skipped, exit stays 0.
    let out = run(&["init", d], None);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), INIT_SKIPPED);
    // --force re-creates the whole set.
    let out = run(&["init", d, "--force"], None);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8(out.stdout).unwrap(), INIT_CREATED);
}

#[test]
fn init_minimal_template_and_scaffold_validates() {
    let dir = init_temp("minimal");
    let d = dir.to_str().unwrap();
    let out = run(&["init", d, "--template", "minimal"], None);
    assert_eq!(out.status.code(), Some(0));
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("created workspace.dsl\n"),
        "{:?}",
        out.stdout
    );
    // P4-D4 teeth through the binary: the scaffold validates.
    let ws = dir.join("workspace.dsl");
    let out = run(&["validate", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(0));
    // …and a bad template value stays clap's 2.
    let out = run(&["init", d, "--template", "bogus"], None);
    assert_eq!(out.status.code(), Some(2));
}

// §P4-M3 / O-3 — `oxidizr completions` (P4-D10): the script IS the
// output contract per shell (clap_complete renders the argv table). The
// goldens pin the shell-specific entry lines + a real subcommand so a
// surface regression cannot silently emit an empty/stale script; the
// syntax smokes (`bash -n`, `zsh -n`) run only where the shell exists.
// An unknown shell name stays clap's usage error (exit 2).

/// Generate the script for one shell (exit must be 0); returns stdout.
fn completions_run(shell: &str) -> String {
    let out = run(&["completions", shell], None);
    assert_eq!(out.status.code(), Some(0), "completions {shell}");
    let script = String::from_utf8(out.stdout).unwrap();
    assert!(script.contains("dsl2json"), "{shell}: surface present");
    script
}

/// `sh -n`-style syntax check where the shell binary exists.
fn shell_syntax_ok(shell: &str, script: &str) {
    if which(shell).is_none() {
        return; // smoke is opportunistic (the plan: "where the shell exists")
    }
    let path = init_temp("syntax").with_extension(shell);
    let _ = std::fs::create_dir_all(path.parent().unwrap());
    std::fs::write(&path, script).unwrap();
    let out = std::process::Command::new(shell)
        .arg("-n")
        .arg(&path)
        .output()
        .unwrap();
    assert!(out.status.success(), "{shell} -n: {:?}", out.stderr);
}

fn which(shell: &str) -> Option<PathBuf> {
    std::process::Command::new(shell)
        .arg("--version")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|_| PathBuf::from(shell))
}

#[test]
fn completions_bash_and_zsh_scripts_are_valid() {
    let bash = completions_run("bash");
    assert!(bash.starts_with("_oxidizr() {"), "bash entry line");
    assert!(
        bash.contains("complete -F _oxidizr -o bashdefault -o default oxidizr"),
        "bash registration"
    );
    shell_syntax_ok("bash", &bash);
    let zsh = completions_run("zsh");
    assert!(zsh.starts_with("#compdef oxidizr"), "zsh entry line");
    shell_syntax_ok("zsh", &zsh);
}

#[test]
fn completions_other_shells_render() {
    let fish = completions_run("fish");
    assert!(fish.contains("complete -c oxidizr"), "fish registration");
    let pwsh = completions_run("powershell");
    assert!(
        pwsh.contains("Register-ArgumentCompleter -Native -CommandName 'oxidizr'"),
        "powershell registration"
    );
    let elvish = completions_run("elvish");
    assert!(
        elvish.contains("set edit:completion:arg-completer[oxidizr]"),
        "elvish registration"
    );
    // Unknown shells are clap usage errors (exit 2), never a stack.
    assert_eq!(run(&["completions", "csh"], None).status.code(), Some(2));
    assert_eq!(run(&["completions"], None).status.code(), Some(2));
}

/// §P4-M3 O-4: the `generate system-landscape` argv surface through the
/// binary — the exit family (0 · zero-workspaces 1 · OXZ-5010 pattern 1
/// · missing path 3 · usage 2) and the `-o` file (ALWAYS JSON).
#[test]
fn generate_system_landscape_merges_into_the_output_file() {
    let dir = init_temp("generate");
    for (tag, text) in [
        (
            "alpha",
            "workspace \"Alpha\" {\n    model {\n        a = person \"Alice\" \"A\"\n        s = softwareSystem \"Billing\" \"B\"\n        a -> s \"Uses\"\n    }\n}\n",
        ),
        (
            "beta",
            "workspace \"Beta\" {\n    model {\n        s = softwareSystem \"CRM\" \"C\"\n        s2 = softwareSystem \"Warehouse\" \"W\"\n        s -> s2 \"Pushes orders to\"\n    }\n}\n",
        ),
    ] {
        let ws_dir = dir.join(tag);
        std::fs::create_dir_all(&ws_dir).unwrap();
        std::fs::write(ws_dir.join("workspace.dsl"), text).unwrap();
    }
    let out_file = dir.join("landscape.json");
    let out = run(
        &[
            "generate",
            "system-landscape",
            dir.to_str().unwrap(),
            "-o",
            out_file.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let json = std::fs::read_to_string(&out_file).unwrap();
    assert!(json.contains("\"System Landscape\""), "{json}");
    assert!(json.contains("\"scope\" : \"Landscape\""), "{json}");
    assert!(json.contains("\"Warehouse\""), "beta merged: {json}");

    // Zero matches: the ported reference message, exit 1.
    let out = run(
        &[
            "generate",
            "system-landscape",
            dir.to_str().unwrap(),
            "--include",
            r".*gamma.*",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("Two or more workspaces must be provided"),
        "{:?}",
        out.stderr
    );

    // Unsupported construct: OXZ-5010, exit 1.
    let out = run(
        &[
            "generate",
            "system-landscape",
            dir.to_str().unwrap(),
            "--exclude",
            r"a{2}",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("OXZ-5010"), "{stderr}");
    assert!(stderr.contains("unsupported construct"), "{stderr}");

    // Missing path: exit 3; zero inputs: clap usage 2.
    let out = run(
        &[
            "generate",
            "system-landscape",
            dir.join("nowhere").to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(3));
    assert_eq!(
        run(&["generate", "system-landscape"], None).status.code(),
        Some(2)
    );
}

fn export_temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oxidizr-cli-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

const EXPORT_WS: &str = "workspace \"Bank\" {\n    model {\n        p = person \"Alice\"\n        \
                         s = softwareSystem \"Big Bank\"\n        p -> s \"Uses\"\n    }\n    \
                         views {\n        systemLandscape \"sys-landscape\" {\n            \
                         include *\n            autoLayout\n        }\n    }\n}\n";

/// §P4T T-6 (gap 6): the `export` success leg through the binary — the
/// reference `ExportCommand` write loop: id-keyed per-view file PLUS the
/// `-key` legend file for a diagram export, and one `Writing <path>` note
/// per file on STDERR (the paths are environment-dependent, so the notes
/// are asserted by SHAPE; the exact note goldens live in the next test).
#[test]
fn export_plantuml_writes_view_and_legend_files() {
    let ws = export_temp("ws.dsl");
    std::fs::write(&ws, EXPORT_WS).unwrap();
    let out_dir = export_temp("out");
    let out = run(
        &[
            "export",
            "-w",
            ws.to_str().unwrap(),
            "-f",
            "plantuml",
            "-o",
            out_dir.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // `structurizr` prefix (workspace id 0) + view key; legend second.
    let mut names = std::fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(
        names,
        [
            "structurizr-sys-landscape-key.puml",
            "structurizr-sys-landscape.puml"
        ]
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    let notes: Vec<&str> = stderr
        .lines()
        .filter(|l| l.starts_with("Writing "))
        .collect();
    assert_eq!(notes.len(), 2, "{stderr}");
    assert!(
        notes[0].ends_with("structurizr-sys-landscape.puml"),
        "{notes:?}"
    );
    assert!(
        notes[1].ends_with("structurizr-sys-landscape-key.puml"),
        "{notes:?}"
    );
}

/// §P4T T-6: the two reference INFO-log lines the export path can emit are
/// byte goldens (reference `ExportCommand` 233/245) and BOTH still exit 0 —
/// the viewless leg writes nothing, the unknown-format leg is a reference
/// no-op too (witnessed against the pinned binary 2026-11-22).
#[test]
fn export_notes_are_golden_and_exit_zero() {
    let empty = export_temp("empty.dsl");
    std::fs::write(&empty, "workspace {\n  model {\n  }\n  views {\n  }\n}\n").unwrap();
    let out = run(
        &[
            "export",
            "-w",
            empty.to_str().unwrap(),
            "-f",
            "plantuml",
            "-o",
            export_temp("notes-1").to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stderr, b"The workspace contains no views\n");

    let ws = export_temp("ws.dsl"); // same bytes as the test above — fine
    std::fs::write(&ws, EXPORT_WS).unwrap();
    let out = run(
        &[
            "export",
            "-w",
            ws.to_str().unwrap(),
            "-f",
            "frobnicate",
            "-o",
            export_temp("notes-2").to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.stderr, b"Unknown export format: frobnicate\n");
}

/// §P4T T-6: the `export` exit family — `static`/`png`/`svg` are
/// known-but-unlanded exporters (OXZ-5007, exit 1; the reference agrees on
/// the exit code for PNG/SVG and would for `static` once it fails);
/// missing workspace file is class 3; a missing `-f` is clap usage 2.
#[test]
fn export_exit_family_static_png_missing_usage() {
    let ws = export_temp("ws.dsl");
    std::fs::write(&ws, EXPORT_WS).unwrap();
    for fmt in ["static", "png"] {
        let out = run(
            &[
                "export",
                "-w",
                ws.to_str().unwrap(),
                "-f",
                fmt,
                "-o",
                export_temp(&format!("fail-{fmt}")).to_str().unwrap(),
            ],
            None,
        );
        assert_eq!(out.status.code(), Some(1), "format `{fmt}`");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("OXZ-5007"), "{stderr}");
        assert!(
            stderr.contains(&format!("`{fmt}` is not implemented")),
            "{stderr}"
        );
    }
    let out = run(
        &[
            "export",
            "-w",
            export_temp("nope.dsl").to_str().unwrap(),
            "-f",
            "json",
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(3));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run(&["export", "-w", ws.to_str().unwrap()], None);
    assert_eq!(out.status.code(), Some(2));
}

/// §P4T T-6 (WSD matrix row): `export -f websequencediagrams` through the
/// binary over the corpus dynamic-view fixture — the sequence-form leg the
/// dialect claims (`wsd.rs`, the M2 N-5 shape): one `.wsd` file, one
/// `Writing` note, exit 0 (bytes are goldened in Category A, so the CLI
/// asserts the argv/file shape only).
#[test]
fn export_websequencediagrams_writes_the_dynamic_view_file() {
    let dsl = testdata().join("dsl/dynamic-view-with-explicit-relationships.dsl");
    let out_dir = export_temp("wsd-out");
    let out = run(
        &[
            "export",
            "-w",
            dsl.to_str().unwrap(),
            "-f",
            "websequencediagrams",
            "-o",
            out_dir.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let names = std::fs::read_dir(&out_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["structurizr-Dynamic-001.wsd"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let notes: Vec<&str> = stderr
        .lines()
        .filter(|l| l.starts_with("Writing "))
        .collect();
    assert_eq!(notes.len(), 1, "{stderr}");
}
