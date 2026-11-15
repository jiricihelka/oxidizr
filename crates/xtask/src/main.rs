//! `xtask` — dev-only tooling (P1-D10): vendor the reference fixture corpus into
//! `testdata/` with attribution, generate/verify goldens with the pinned official
//! `structurizr` application binary (the oracle), and run the CI gates locally
//! (`ci` / `bench` / `cov`, mirroring `.github/workflows/ci.yml`).
//!
//! Oracle discovery follows agents/plan/TESTING.md: `$OXIDZR_STRUCTURIZR_BIN` first, then
//! `structurizr` on `PATH`. The binary is slow — run this sparingly; goldens are
//! committed.
//!
//! Invocation contract (verified against 2026.10.26, see agents/plan/PROGRESS.md):
//! `structurizr export --workspace F --format json --output DIR` — GNU-style options
//! only; `DIR` is a directory and receives `<input-basename>.json`.

use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};

mod refs;

/// Pinned upstream monorepo commit (agents/plan/COMPAT.md).
const REFERENCE_COMMIT: &str = "ad34036";
/// Pinned unified application release used for goldens (bundles library 6.2.4);
/// mirrors `packages.structurizr` in flake.nix.
const REFERENCE_APP_VERSION: &str = "2026.10.26";

fn main() {
    if let Err(err) = real_main(env::args().skip(1).collect()) {
        eprintln!("xtask error: {err:#}");
        std::process::exit(1);
    }
}

fn real_main(args: Vec<String>) -> Result<()> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("vendor") => {
            let src = flag(&mut args, "--src")
                .or_else(|| env::var_os("OXIDZR_REFERENCE_SRC").map(PathBuf::from));
            let Some(src) = src else {
                bail!(
                    "usage: cargo xtask vendor --src <reference-repo-clone> \
                     (or set $OXIDZR_REFERENCE_SRC)"
                );
            };
            cmd_vendor(&src)
        }
        Some("ci") => cmd_ci(resolve_ci_steps(&args.collect::<Vec<_>>())?),
        Some("refs") => refs::cmd_refs(),
        Some("lint") => cmd_lint(),
        Some("bench") => {
            match args.next().as_deref() {
                None => cmd_bench(),
                Some("save") => cmd_bench_save(
                    &args
                        .next()
                        .context("bench save needs a label, e.g. `cargo xtask bench save M5`")?,
                ),
                Some("cmp") => cmd_bench_cmp(&args.next().context(
                    "bench cmp needs a baseline label, e.g. `cargo xtask bench cmp M4`",
                )?),
                Some(other) => {
                    bail!("bench subcommands: save <LABEL> | cmp <LABEL> | (none), got `{other}`")
                }
            }
        }
        Some("cov") => {
            ensure!(args.next().is_none(), "cov takes no arguments");
            cmd_cov()
        }
        Some("oracle") => {
            let oracle = find_oracle()?;
            match args.next().as_deref() {
                Some("gen") => {
                    let mut fixtures = Vec::new();
                    while let Some(a) = args.next() {
                        ensure!(a == "--fixture", "unexpected argument `{a}`");
                        let name = args.next().context("--fixture needs a name")?;
                        fixtures.push(name);
                    }
                    if fixtures.is_empty() {
                        fixtures.push("workspace-properties".into());
                    }
                    cmd_oracle_gen(&oracle, &fixtures)
                }
                Some("verify") => {
                    ensure!(args.next().is_none(), "unexpected argument");
                    cmd_oracle_verify(&oracle)
                }
                _ => bail!("usage: cargo xtask oracle gen [--fixture NAME]… | verify"),
            }
        }
        Some("-h" | "--help" | "help") => {
            print_help();
            Ok(())
        }
        _ => {
            print_help();
            bail!("no valid subcommand given");
        }
    }
}

fn print_help() {
    println!(
        "cargo xtask ci [STEP…]               run CI gates locally (default: fmt clippy test refs)\n\
         \n\
         \x20   local CI mirror (see .github/workflows/ci.yml); STEPS, run in the given\n\
         \x20   order without fail-fast, each reported in a final summary:\n\
         \x20     fmt      cargo fmt --all --check\n\
         \x20     clippy   cargo clippy --workspace --all-targets -- -D warnings\n\
         \x20     test     cargo test --workspace            (host, Category A)\n\
         \x20     refs     doc-reference integrity          (local-only extra gate)\n\
         \x20     lint     actionlint on .github/           (skips when actionlint absent)\n\
         \x20     windows  cargo test --workspace --target x86_64-pc-windows-gnu\n\
         \x20              (Wine leg; needs the nix dev shell — it provides the runner,\n\
         \x20              linker and libpthread stub; the macOS leg stays CI-only)\n\
         cargo xtask refs                     check doc path & §section references only\n\
         cargo xtask lint                     actionlint only (skips if not installed)\n\
         cargo xtask bench                    cargo bench --workspace (reports: target/criterion)\n\
         cargo xtask bench save <LABEL>       snapshot criterion results into bench-baseline/<LABEL>/\n\
         cargo xtask bench cmp <LABEL>        critcmp bench-baseline/<LABEL> vs target/criterion\n\
         cargo xtask cov                      cargo llvm-cov --workspace --summary-only (report only)\n\
         \n\
         cargo xtask vendor --src <clone>     vendor reference fixtures into testdata/\n\
         cargo xtask oracle gen [--fixture N]…  generate goldens via the reference binary\n\
         cargo xtask oracle verify              re-run goldens and byte-compare (slow)\n\
         \n\
         pinned reference: commit {REFERENCE_COMMIT}, app release {REFERENCE_APP_VERSION}\n\
         oracle discovery: $OXIDZR_STRUCTURIZR_BIN, then `structurizr` on PATH"
    );
}

fn flag(args: &mut impl Iterator<Item = String>, name: &str) -> Option<PathBuf> {
    while let Some(a) = args.next() {
        if a == name {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(PathBuf::from)
        .context("xtask must live in <root>/crates/xtask")
}

// ---------------------------------------------------------------- local CI mirror

/// One CI gate. Cargo steps mirror a job's `run:` line in
/// `.github/workflows/ci.yml`; `Refs` is a local-only extra gate
/// (doc-reference integrity; user decision 2026-11-15: every commit gate,
/// not CI).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Step {
    Fmt,
    Clippy,
    Test,
    Refs,
    Lint,
    /// windows-latest matrix leg approximated via the windows-gnu + Wine dev leg.
    Windows,
}

impl Step {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "fmt" => Some(Self::Fmt),
            "clippy" => Some(Self::Clippy),
            "test" => Some(Self::Test),
            "refs" => Some(Self::Refs),
            "lint" => Some(Self::Lint),
            "windows" => Some(Self::Windows),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Fmt => "fmt",
            Self::Clippy => "clippy",
            Self::Test => "test",
            Self::Refs => "refs",
            Self::Lint => "lint",
            Self::Windows => "windows",
        }
    }

    /// Cargo argv for this gate (identical to the CI job's `run:` line).
    fn cargo_args(self) -> &'static [&'static str] {
        match self {
            Self::Fmt => &["fmt", "--all", "--check"],
            Self::Clippy => &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
            Self::Test => &["test", "--workspace"],
            Self::Refs => &["xtask", "refs"],
            Self::Lint => &["xtask", "lint"],
            Self::Windows => &["test", "--workspace", "--target", "x86_64-pc-windows-gnu"],
        }
    }
}

/// Default = the commit gate (fmt + clippy + test + refs); otherwise the given
/// names in the given order, de-duplicated. Pure, so the parsing rules are
/// unit-tested.
fn resolve_ci_steps(args: &[String]) -> Result<Vec<Step>> {
    if args.is_empty() {
        return Ok(vec![Step::Fmt, Step::Clippy, Step::Test, Step::Refs]);
    }
    let mut out = Vec::new();
    for arg in args {
        let step = Step::parse(arg).with_context(|| {
            format!("unknown CI step `{arg}` (fmt|clippy|test|refs|lint|windows)")
        })?;
        if !out.contains(&step) {
            out.push(step);
        }
    }
    Ok(out)
}

/// cargo honoring the `cargo` wrapper cargo sets for subcommands when available.
fn cargo_cmd() -> Command {
    let program = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(program);
    cmd.current_dir(repo_root().expect("repo root"));
    cmd
}

/// Stream a gate to the terminal; `Ok(false)` on nonzero exit (no fail-fast —
/// `ci` runs every selected step and reports them all at the end).
fn run_step(step: Step) -> Result<bool> {
    let args = step.cargo_args();
    println!("\n==> {} `$ cargo {}`", step.label(), args.join(" "));
    let status = cargo_cmd()
        .args(args)
        .status()
        .context("failed to run cargo")?;
    if !status.success() {
        println!("<== {} FAILED ({status})", step.label());
    }
    Ok(status.success())
}

fn cmd_ci(steps: Vec<Step>) -> Result<()> {
    let had_windows = steps.contains(&Step::Windows);
    if had_windows && env::var_os("CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER").is_none() {
        bail!(
            "windows step needs the Wine dev shell (nix develop): it provides \
             CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUNNER/-RUSTFLAGS and the mingw linker"
        );
    }
    let mut failed = Vec::new();
    for step in steps {
        if !run_step(step)? {
            failed.push(step.label());
        }
    }
    if had_windows {
        reap_wine();
    }
    if failed.is_empty() {
        println!("\nci: all steps green");
        Ok(())
    } else {
        bail!("ci: FAILED step(s): {}", failed.join(", "));
    }
}

/// Bench job mirror: no trailing args (they would hit the libtest harnesses —
/// see ci.yml note), criterion defaults to `target/criterion` anyway.
fn cmd_bench() -> Result<()> {
    let status = cargo_cmd()
        .args(["bench", "--workspace"])
        .status()
        .context("failed to run cargo bench")?;
    ensure!(status.success(), "cargo bench failed ({status})");
    println!(
        "\nreports: target/criterion/ — snapshot with `cargo xtask bench save <LABEL>`, \
         compare with `cargo xtask bench cmp <LABEL>`"
    );
    Ok(())
}

/// TESTING.md milestone ritual: copy criterion's per-benchmark result JSONs from
/// `target/criterion/` into `bench-baseline/<LABEL>/` (same layout as M0: one dir
/// per benchmark, `estimates/sample/tukey/benchmark.json`; the html `report/`
/// dirs are not committed).
fn cmd_bench_save(label: &str) -> Result<()> {
    ensure!(
        !label.is_empty()
            && label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "label must be [A-Za-z0-9_-]+ (e.g. M5)"
    );
    let root = repo_root()?;
    let src = root.join("target/criterion");
    ensure!(
        src.is_dir(),
        "no {} — run `cargo xtask bench` first",
        src.display()
    );
    let dst = root.join("bench-baseline").join(label);
    let mut n = 0usize;
    for bench in fs::read_dir(&src)? {
        let group = bench?.path();
        // criterion layout: target/criterion/<group>/new/{estimates,sample,tukey,
        // benchmark}.json — the committed baseline is the same, minus `new/`
        // (M0 layout, bench-baseline/README.md).
        let new_dir = group.join("new");
        if !new_dir.is_dir() {
            continue; // stray files at criterion root (top-level report/)
        }
        let name = group.file_name().unwrap_or_default().to_os_string();
        for entry in fs::read_dir(&new_dir)? {
            let p = entry?.path();
            if p.extension().is_some_and(|e| e == "json") {
                fs::create_dir_all(dst.join(&name))?;
                fs::copy(&p, dst.join(&name).join(p.file_name().unwrap()))?;
                n += 1;
            }
        }
        println!("saved {}", dst.join(&name).display());
    }
    ensure!(n > 0, "no criterion result JSONs under {}", src.display());
    println!("baseline `{label}`: {n} file(s) — commit them (TESTING.md §Benchmarks)");
    Ok(())
}

/// Compare the committed baseline with the latest run. critcmp's canonical
/// input is two baseline dirs under ONE criterion root (`critcmp OLD NEW` run
/// from the repo root — verified against critcmp 0.1.8; bare directory args
/// and `<dir>/<group>/new` staging match nothing). So the flat committed
/// baseline (bench-baseline/README.md: `<LABEL>/<group>/*.json`) is staged
/// into `target/criterion/<group>/<LABEL>/`, compared against criterion's
/// automatic `new`, then un-staged.
fn cmd_bench_cmp(label: &str) -> Result<()> {
    let root = repo_root()?;
    let base = root.join("bench-baseline").join(label);
    ensure!(base.is_dir(), "no baseline `{label}` ({})", base.display());
    let criterion = root.join("target/criterion");
    ensure!(
        criterion.is_dir(),
        "no {} — run `cargo xtask bench` first",
        criterion.display()
    );
    std::env::set_current_dir(&root)?;
    let staged = stage_baseline(&criterion, &base, label);
    let staged = match staged {
        Ok(n) => n,
        Err(e) => {
            let _ = unstage_baseline(&criterion, label);
            return Err(e);
        }
    };
    ensure!(
        staged > 0,
        "baseline `{label}` has no criterion JSONs ({})",
        base.display()
    );
    let status = Command::new("critcmp").args([label, "new"]).status();
    unstage_baseline(&criterion, label)?;
    match status {
        Ok(s) => ensure!(s.success(), "critcmp failed ({s})"),
        Err(_) => {
            bail!("critcmp not found — the nix dev shell provides it (agents/plan/WORKFLOW.md)")
        }
    }
    Ok(())
}

/// Copy flat baseline groups into `target/criterion/<group>/<label>/`; return
/// the number of JSONs copied (0 ⇒ no overlapping groups with the current run).
fn stage_baseline(criterion: &Path, base: &Path, label: &str) -> Result<usize> {
    let mut n = 0usize;
    for group in fs::read_dir(base)? {
        let group = group?.path();
        if !group.is_dir() {
            continue; // bench-baseline/README.md sits next to the labels
        }
        let dest = criterion
            .join(group.file_name().unwrap_or_default())
            .join(label);
        for entry in fs::read_dir(&group)? {
            let p = entry?.path();
            if p.extension().is_some_and(|e| e == "json") {
                fs::create_dir_all(&dest)?;
                fs::copy(&p, dest.join(p.file_name().unwrap()))?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Remove the staged `<label>` dirs again (target/ stays as if nothing happened).
fn unstage_baseline(criterion: &Path, label: &str) -> Result<()> {
    if criterion.is_dir() {
        for group in fs::read_dir(criterion)? {
            let staged = group?.path().join(label);
            if staged.is_dir() {
                fs::remove_dir_all(&staged)?;
            }
        }
    }
    Ok(())
}

/// Workflow lint via actionlint (dev shell tool); CI's `lint` job is the authority.
fn cmd_lint() -> Result<()> {
    let probe = Command::new("actionlint")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !probe.is_ok_and(|s| s.success()) {
        println!(
            "lint: actionlint not on PATH — skipping locally (the nix dev shell provides it; \
             CI's lint job is authoritative)"
        );
        return Ok(());
    }
    let status = Command::new("actionlint")
        .current_dir(repo_root()?)
        .status()
        .context("failed to run actionlint")?;
    ensure!(status.success(), "actionlint failed ({status})");
    Ok(())
}

/// Best-effort Wine teardown after the windows leg — without this each
/// `ci windows` run leaves ~100 wine server processes (agents/notes/env/wine-windows-leg.md).
fn reap_wine() {
    let quiet = |cmd: &mut Command| {
        let _ = cmd
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    };
    let mut probe = Command::new("wineserver");
    probe.arg("-V");
    if probe
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
    {
        quiet(Command::new("wineserver").args(["-k", "-q"]));
        println!("reaped wine server (wineserver -k)");
    }
}

/// Coverage job mirror: report only, never a gate (agents/plan/TESTING.md).
fn cmd_cov() -> Result<()> {
    let probe = cargo_cmd()
        .args(["llvm-cov", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !probe.is_ok_and(|s| s.success()) {
        bail!("cargo-llvm-cov not found — the nix dev shell provides it (agents/plan/WORKFLOW.md)");
    }
    let status = cargo_cmd()
        .args(["llvm-cov", "--workspace", "--summary-only"])
        .status()
        .context("failed to run cargo llvm-cov")?;
    ensure!(status.success(), "cargo llvm-cov failed ({status})");
    Ok(())
}

// ---------------------------------------------------------------- oracle plumbing

fn find_oracle() -> Result<PathBuf> {
    if let Some(explicit) = env::var_os("OXIDZR_STRUCTURIZR_BIN") {
        let path = PathBuf::from(&explicit);
        ensure!(
            path.is_file(),
            "$OXIDZR_STRUCTURIZR_BIN={}",
            explicit.to_string_lossy()
        );
        return Ok(path);
    }
    for dir in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        // Windows ships .bat/.cmd wrappers; plain name covers unix.
        for name in ["structurizr", "structurizr.bat", "structurizr.cmd"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    bail!(
        "structurizr reference binary not found ($OXIDZR_STRUCTURIZR_BIN → PATH); \
         the nix dev flake provides it (agents/plan/TESTING.md)"
    );
}

/// Run `structurizr export --format json` and return the produced file path.
fn reference_export_json(oracle: &Path, workspace: &Path, outdir: &Path) -> Result<PathBuf> {
    fs::create_dir_all(outdir)?;
    let output = Command::new(oracle)
        .arg("export")
        .arg("--workspace")
        .arg(workspace)
        .arg("--format")
        .arg("json")
        .arg("--output")
        .arg(outdir)
        .output()
        .with_context(|| format!("failed to run {}", oracle.display()))?;
    if !output.status.success() {
        bail!(
            "structurizr export failed ({}):\n{}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let stem = workspace
        .file_stem()
        .and_then(|s| s.to_str())
        .context("fixture without usable file stem")?;
    let produced = outdir.join(format!("{stem}.json"));
    ensure!(
        produced.is_file(),
        "expected output missing: {}",
        produced.display()
    );
    Ok(produced)
}

/// Golden normalization: drop the four violation-counter properties the reference
/// **application harness** injects into `workspace.properties` on every load
/// (`AbstractCommand.loadWorkspace` → `new DefaultInspector(workspace)`;
/// `structurizr-inspection/DefaultInspector.java`). They are harness artifacts, not
/// workspace content — oxidizr models them as ordinary (round-tripped) properties but
/// never computes counts (that would mean reimplementing `structurizr-inspection`).
/// Verified against 2026.10.26 (2026-11-14, `agents/plan/PROGRESS.md`).
///
/// Line deletion must fix up the preceding comma when a removed entry was the object's
/// LAST member (the reference pretty-prints one entry per line, so a comma-less removed
/// line means the previous line's trailing comma must go too). Note the export output
/// has NO volatile members (`lastModifiedDate*` only round-trips from JSON inputs,
/// deterministically) — nothing else is stripped.
fn normalize_reference_output(raw: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(raw);
    // Byte parity includes EOF: the reference pretty-printer emits NO trailing
    // newline; `lines()` would invent one (M0 goldens carried that artifact).
    let raw_trailing_newline = text.ends_with('\n');
    let mut out = String::new();
    for line in text.lines() {
        let t = line.trim_start();
        let removed = (t.starts_with("\"structurizr.inspection.error\"")
            || t.starts_with("\"structurizr.inspection.warning\"")
            || t.starts_with("\"structurizr.inspection.info\"")
            || t.starts_with("\"structurizr.inspection.ignore\""))
            && t.contains("\" :");
        if removed {
            if !t.ends_with(',') {
                // removed entry was the last in its object: drop the previous comma
                if let Some(pos) = out.trim_end_matches('\n').rfind(',')
                    && out.trim_end_matches('\n')[pos + 1..]
                        .chars()
                        .all(char::is_whitespace)
                {
                    out.remove(pos);
                }
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !raw_trailing_newline {
        out.pop();
    }
    out.into_bytes()
}

// ---------------------------------------------------------------- vendor

fn cmd_vendor(src: &Path) -> Result<()> {
    let dsl_root = src
        .join("structurizr-dsl")
        .join("src")
        .join("test")
        .join("resources")
        .join("dsl");
    let client_root = src
        .join("structurizr-client")
        .join("src")
        .join("test")
        .join("resources");
    ensure!(
        dsl_root.is_dir(),
        "not a reference repo clone: {}",
        src.display()
    );

    let testdata = repo_root()?.join("testdata");
    fs::create_dir_all(&testdata)?;

    copy_dir_all(&dsl_root, &testdata.join("dsl"))?;
    copy_dir_all(
        &client_root.join("backwardsCompatibility"),
        &testdata.join("read-corpus").join("backwardsCompatibility"),
    )?;
    fs::create_dir_all(testdata.join("read-corpus"))?;
    fs::copy(
        client_root.join("structurizr-36141-workspace.json"),
        testdata
            .join("read-corpus")
            .join("structurizr-36141-workspace.json"),
    )?;
    copy_dir_all(
        &client_root.join("workspaceValidation"),
        &testdata.join("validation"),
    )?;
    fs::create_dir_all(testdata.join("golden"))?;

    // Deferred corpus (D7): fixtures using `!docs`/`!adrs` are vendored but excluded
    // from the P1 parse/golden gates; P4-D13 retires the exclusion.
    let deferred = scan_deferred(&testdata.join("dsl"))?;
    let dsl_count = walk_files(&testdata.join("dsl"), ".dsl")?.len();
    let json_count = walk_files(&testdata.join("read-corpus"), ".json")?.len()
        + walk_files(&testdata.join("validation"), ".json")?.len();

    fs::write(
        testdata.join("README.md"),
        format!(
            "# testdata — vendored reference corpus\n\n\
             Generated by `cargo xtask vendor` (crates/xtask). Do not edit by hand.\n\n\
             * Source: https://github.com/structurizr/structurizr (monorepo), commit `{REFERENCE_COMMIT}`,\n\
               licensed Apache-2.0 — attribution for all vendored files stays with the Structurizr project.\n\
             * Goldens: generated with the unified application binary **structurizr {REFERENCE_APP_VERSION}**\n\
               (library 6.2.4) via `cargo xtask oracle gen`; the four `structurizr.inspection.*` violation\n\
               counters the reference app injects on load are stripped at generation time (see\n\
               `xtask::normalize_reference_output` and `agents/plan/PROGRESS.md`). Requires Java 21.\n\
             * `dsl/` — {dsl_count} DSL fixtures (reference test resources; P1 parse corpus).\n\
             * `probes/` — hand-written oracle-probe fixtures (WRITE-CATALOG witnesses; see\n\
               `agents/plan/build/WRITE-CATALOG.md`); goldens for them are generated the same way.\n\
             * `read-corpus/` + `validation/` — {json_count} JSON workspaces (back-compat read + validation rules).\n\
             * `golden/` — committed reference goldens (byte-parity gate; regenerate sparingly).\n\n\
             ## Deferred fixtures (D7 — `!docs`/`!adrs`, excluded from P1–P3 parse/golden gates)\n\n\
             These fixtures parse-statement-form only until the importers land in Phase 4 (P4-D13);\n\
             loading them yields OXZ-5003. P4 retires this list.\n\n\
             {}\n",
            deferred
                .iter()
                .map(|p| format!("- `{p}`"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )?;

    println!(
        "vendored {} .dsl fixtures + {} JSON files into testdata/ ({} deferred for D7)",
        dsl_count,
        json_count,
        deferred.len()
    );
    Ok(())
}

/// Files under `root` whose text contains a `!docs`/`!adrs` statement.
fn scan_deferred(root: &Path) -> Result<BTreeSet<String>> {
    let mut hits = BTreeSet::new();
    for path in walk_files(root, ".dsl")? {
        // Lossy on purpose: non-UTF-8 fixtures exist (iso-8859.dsl, V-8); the
        // `!docs`/`!adrs` markers are ASCII and survive any lossy decode.
        let bytes = fs::read(&path)?;
        let text = String::from_utf8_lossy(&bytes);
        if text.lines().any(|line| {
            let t = line.trim_start();
            t.starts_with("!docs") || t.starts_with("!adrs")
        }) {
            let rel = path.strip_prefix(root.parent().context("unexpected layout")?)?;
            hits.insert(rel.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(hits)
}

fn copy_dir_all(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn walk_files(root: &Path, extension: &str) -> Result<Vec<PathBuf>> {
    // `extension` may be given as ".dsl" or "dsl"; Path::extension() has no dot.
    let extension = extension.strip_prefix('.').unwrap_or(extension);
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == extension) {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

// ---------------------------------------------------------------- oracle gen/verify

fn find_fixture(root: &Path, name: &str) -> Result<PathBuf> {
    let mut candidates = walk_files(&root.join("testdata").join("dsl"), ".dsl")?;
    // Probe fixtures (testdata/probes) are first-class golden sources too
    // (WRITE-CATALOG witnesses).
    candidates.extend(walk_files(&root.join("testdata").join("probes"), ".dsl")?);
    candidates.retain(|p| p.file_stem().is_some_and(|s| s == name));
    match candidates.as_slice() {
        [] => bail!("fixture `{name}` not found under testdata/dsl"),
        [one] => Ok(one.clone()),
        [first, rest @ ..] => {
            eprintln!(
                "xtask note: fixture `{name}` matches {} files, using {}",
                rest.len() + 1,
                first.display()
            );
            Ok(first.clone())
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    env::temp_dir().join(format!("oxz-oracle-{}-{}", std::process::id(), name))
}

fn cmd_oracle_gen(oracle: &Path, fixtures: &[String]) -> Result<()> {
    let root = repo_root()?;
    let golden_dir = root.join("testdata").join("golden");
    fs::create_dir_all(&golden_dir)?;
    for name in fixtures {
        let dsl = find_fixture(&root, name)?;
        let tmp = scratch(name);
        let produced = reference_export_json(oracle, &dsl, &tmp)
            .with_context(|| format!("oracle gen `{name}`"))?;
        let golden = normalize_reference_output(&fs::read(&produced)?);
        let out = golden_dir.join(format!("{name}.json"));
        fs::write(&out, &golden)?;
        let _ = fs::remove_dir_all(&tmp);
        println!("golden: {} ({} bytes)", out.display(), golden.len());
    }
    Ok(())
}

fn cmd_oracle_verify(oracle: &Path) -> Result<()> {
    let root = repo_root()?;
    let goldens = walk_files(&root.join("testdata").join("golden"), ".json")?;
    ensure!(!goldens.is_empty(), "no committed goldens to verify");
    let mut failures = 0_usize;
    for golden_path in &goldens {
        let name = golden_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("bad golden name")?;
        let dsl = find_fixture(&root, name)?;
        let tmp = scratch(name);
        let produced = reference_export_json(oracle, &dsl, &tmp)
            .with_context(|| format!("oracle verify `{name}`"))?;
        let fresh = normalize_reference_output(&fs::read(&produced)?);
        let _ = fs::remove_dir_all(&tmp);
        let committed = fs::read(golden_path)?;
        if fresh == committed {
            println!("ok: {name}");
        } else {
            failures += 1;
            println!("MISMATCH: {name} (reference output drifted from committed golden)");
        }
    }
    ensure!(failures == 0, "{failures} golden(s) failed verification");
    println!("oracle verify: {} golden(s) match", goldens.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Step, normalize_reference_output, resolve_ci_steps};

    #[test]
    fn ci_steps_default_to_the_commit_gate_quartet() {
        assert_eq!(
            resolve_ci_steps(&[]).unwrap(),
            vec![Step::Fmt, Step::Clippy, Step::Test, Step::Refs]
        );
    }

    #[test]
    fn ci_steps_keep_order_and_dedup() {
        let args: Vec<String> = ["test", "windows", "test"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            resolve_ci_steps(&args).unwrap(),
            vec![Step::Test, Step::Windows]
        );
    }

    #[test]
    fn ci_steps_reject_unknown_names() {
        assert!(resolve_ci_steps(&["macos".into()]).is_err());
    }

    fn norm(s: &str) -> String {
        String::from_utf8(normalize_reference_output(s.as_bytes())).unwrap()
    }

    #[test]
    fn removes_middle_entries_untouched() {
        let src = "{\n  \"properties\" : {\n    \"a\" : \"1\",\n    \"structurizr.inspection.error\" : \"3\",\n    \"structurizr.inspection.info\" : \"0\",\n    \"z\" : \"9\"\n  }\n}\n";
        assert_eq!(
            norm(src),
            "{\n  \"properties\" : {\n    \"a\" : \"1\",\n    \"z\" : \"9\"\n  }\n}\n"
        );
    }

    #[test]
    fn preserves_missing_trailing_newline() {
        // The reference pretty-printer emits no trailing newline; goldens must
        // not invent one (byte parity includes EOF).
        let src = "{\n  \"a\" : \"1\"\n}";
        assert_eq!(norm(src), "{\n  \"a\" : \"1\"\n}");
    }

    #[test]
    fn removes_last_entry_and_fixes_comma() {
        // inspection entries sorted last (as in workspace-properties.dsl, 2026-11-14):
        // the surviving `structurizr.dslEditor` line must lose its trailing comma.
        let src = "{\n  \"properties\" : {\n    \"structurizr.dslEditor\" : \"false\",\n    \"structurizr.inspection.warning\" : \"0\"\n  }\n}\n";
        assert_eq!(
            norm(src),
            "{\n  \"properties\" : {\n    \"structurizr.dslEditor\" : \"false\"\n  }\n}\n"
        );
    }

    #[test]
    fn leaves_other_keys_alone() {
        let src = "{\n  \"structurizr.inspectionUrl\" : \"x\",\n  \"structurizr.dsl\" : \"y\"\n}\n";
        assert_eq!(norm(src), src);
    }
}
