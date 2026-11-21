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
