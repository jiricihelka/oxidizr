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
//! `--format plantuml` writes `structurizr-<id>-<key>.puml` (+ `-key` legend) files
//! into `DIR` instead (M2-d); those goldens land under
//! `testdata/golden/plantuml/<fixture>/` and need NO normalization.
//! `--format plantuml/c4plantuml` (M3-d) emits the same file naming, no legend
//! files, and lands goldens under `testdata/golden/plantuml-c4/<fixture>/`.
//! `cargo xtask export-verify` (M3-d, P2-D9; §P4-M5 E-2: `--format`)
//! semantically compares C4 / plain-plantuml / Mermaid output
//! (fact multisets, order-normalized — `exportcmp`) between oracle and
//! oxidizr; run it sparingly, like the oracle itself.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};

mod exportcmp;
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
        Some("fuzz") => {
            let secs = match args.next() {
                None => 10,
                Some(s) => s.parse::<u64>().with_context(|| {
                    format!("fuzz: SECONDS-PER-TARGET must be a number, got `{s}`")
                })?,
            };
            ensure!(
                args.next().is_none(),
                "usage: cargo xtask fuzz [SECONDS-PER-TARGET]"
            );
            cmd_fuzz(secs)
        }
        Some("export-verify") => cmd_export_verify(&mut args),
        Some("oracle") => {
            let oracle = find_oracle()?;
            match args.next().as_deref() {
                Some("gen") => oracle_gen_args(&oracle, &mut args),
                Some("verify") => {
                    ensure!(args.next().is_none(), "unexpected argument");
                    cmd_oracle_verify(&oracle)
                }
                Some("landscape") => {
                    ensure!(args.next().is_none(), "unexpected argument");
                    cmd_oracle_landscape(&oracle)
                }
                _ => {
                    bail!(
                        "usage: cargo xtask oracle gen [--plantuml [--c4] | --mermaid | --wsd \
                         | --theme] [--fixture NAME]… | verify | landscape"
                    )
                }
            }
        }
        Some("e2e") => cmd_e2e(&mut args),
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
         cargo xtask bench                    cargo bench --workspace --features \\\n\
         \"                                      oxidizr-e2e/e2e-bench (target/criterion)\n\
         cargo xtask bench save <LABEL>       snapshot criterion results into bench-baseline/<LABEL>/\n\
         cargo xtask bench cmp <LABEL>        critcmp bench-baseline/<LABEL> vs target/criterion\n\
         cargo xtask cov                      cargo llvm-cov --workspace --summary-only (report only)\n\
         cargo xtask fuzz [SECONDS]           cargo-fuzz job mirror (M6-d): build every target,\n\
         \x20              seed from testdata/, run each SECONDS (default 10; CI nightly)\n\
         \n\
         cargo xtask vendor --src <clone>     vendor reference fixtures into testdata/\n\
         cargo xtask e2e bless [FILTER]       regenerate e2e/corpus sidecars (Phase 1T);\n\
         cargo xtask e2e mutate [SEED]        generate e2e/corpus/mutated fixtures\n\
         \x20              FILTER matches `<kind>/<name>` substrings; review the diff\n\
         cargo xtask oracle gen [--fixture N]…  generate JSON goldens via the reference binary\n\
         \x20   [--plantuml|--mermaid|--wsd|--theme] + --fixture N…\n\
         \x20     generate diagram (golden/plantuml[-c4|mermaid|websequencediagrams]/<N>/) or\n\
         \x20     theme (golden/theme/<N>.json) goldens\n\
         cargo xtask oracle verify              re-run goldens and byte-compare (slow)\n\
         cargo xtask oracle landscape          A/B the landscape generator (slow, §P4-M3 O-4)\n\
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

// ---------------------------------------------------------------- e2e corpus

/// `cargo xtask e2e bless [FILTER]` — regenerate the `workspace.e2e.json`
/// sidecars from observed behavior (Phase 1T, P1T-D3). The resulting git
/// diff is the review artifact.
/// `cargo xtask e2e …` — bless / mutate dispatch (Phase 1T).
fn cmd_e2e(args: &mut impl Iterator<Item = String>) -> Result<()> {
    match args.next().as_deref() {
        Some("bless") => {
            let filter = args.next();
            ensure!(
                args.next().is_none(),
                "usage: cargo xtask e2e bless [FILTER]"
            );
            cmd_e2e_bless(filter.as_deref())
        }
        Some("mutate") => {
            let seed = match args.next() {
                None => 7,
                Some(s) => s
                    .parse::<u64>()
                    .with_context(|| format!("seed `{s}` is not a u64"))?,
            };
            ensure!(
                args.next().is_none(),
                "usage: cargo xtask e2e mutate [SEED]"
            );
            cmd_e2e_mutate(seed)
        }
        _ => bail!("usage: cargo xtask e2e bless [FILTER] | mutate [SEED]"),
    }
}

fn cmd_e2e_bless(filter: Option<&str>) -> Result<()> {
    let reports = oxidizr_e2e::bless::bless(filter).map_err(anyhow::Error::msg)?;
    for line in reports {
        println!("{line}");
    }
    Ok(())
}

fn cmd_e2e_mutate(seed: u64) -> Result<()> {
    let reports = oxidizr_e2e::mutate::write_corpus(seed).map_err(anyhow::Error::msg)?;
    let count = reports.len();
    for line in reports {
        println!("{line}");
    }
    println!("{count} mutations written (seed {seed}) — bless to generate sidecars");
    Ok(())
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
    /// GUI clippy (eframe shell + the in-process `oxidizr view` entry,
    /// §P3-M8): `oxidizr-viewer` AND `oxidizr-cli` with `gui`. The default
    /// quartet omits it (keeps `cargo test --workspace` headless, P3-D1);
    /// CI runs it as an extra leg.
    Gui,
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
            "gui" => Some(Self::Gui),
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
            Self::Gui => "gui",
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
            Self::Gui => &[
                "clippy",
                "-p",
                "oxidizr-viewer",
                "-p",
                "oxidizr-cli",
                "--all-targets",
                "--features",
                "oxidizr-viewer/gui,oxidizr-cli/gui",
                "--",
                "-D",
                "warnings",
            ],
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
            format!("unknown CI step `{arg}` (fmt|clippy|test|refs|lint|gui|windows)")
        })?;
        if !out.contains(&step) {
            out.push(step);
        }
    }
    Ok(out)
}

/// cargo honoring the `cargo` wrapper cargo sets for subcommands when available.
/// 2R R-tool-02 (same non-test-panic class as the bench-copy unwraps):
/// `repo_root` failing means the binary lives outside `<root>/crates/xtask` —
/// an error to report, not a panic to crash on.
fn cargo_cmd() -> Result<Command> {
    let program = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(program);
    cmd.current_dir(repo_root()?);
    Ok(cmd)
}

/// Stream a gate to the terminal; `Ok(false)` on nonzero exit (no fail-fast —
/// `ci` runs every selected step and reports them all at the end).
fn run_step(step: Step) -> Result<bool> {
    let args = step.cargo_args();
    println!("\n==> {} `$ cargo {}`", step.label(), args.join(" "));
    let status = cargo_cmd()?
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
/// see ci.yml note), criterion defaults to `target/criterion` anyway. Unlike
/// CI, the local mirror passes `--features e2e-bench` so the whole-corpus
/// benches are always measured (P1T-D7: CI skips them, they are local dev).
fn cmd_bench() -> Result<()> {
    let status = cargo_cmd()?
        .args([
            "bench",
            "--workspace",
            "--features",
            "oxidizr-e2e/e2e-bench",
        ])
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
                // 2R R-tool-02: a `read_dir` entry always has a file name —
                // but the charter says a broken answer surfaces as an
                // error, never a panic (`unwrap` is test-only).
                let file_name = p
                    .file_name()
                    .with_context(|| format!("no file name: {}", p.display()))?;
                fs::copy(&p, dst.join(&name).join(file_name))?;
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
                // 2R R-tool-02: same contract as the save leg above.
                let file_name = p
                    .file_name()
                    .with_context(|| format!("no file name: {}", p.display()))?;
                fs::copy(&p, dest.join(file_name))?;
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
    let probe = cargo_cmd()?
        .args(["llvm-cov", "--version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if !probe.is_ok_and(|s| s.success()) {
        bail!("cargo-llvm-cov not found — the nix dev shell provides it (agents/plan/WORKFLOW.md)");
    }
    let status = cargo_cmd()?
        .args(["llvm-cov", "--workspace", "--summary-only"])
        .status()
        .context("failed to run cargo llvm-cov")?;
    ensure!(status.success(), "cargo llvm-cov failed ({status})");
    Ok(())
}

// ------------------------------------------------------------------------ fuzz (M6-d)

/// cargo-fuzz targets and their seed roots (relative to the repo root):
/// `(target, &[(testdata dir, extension)])`. Seed corpus = the vendored
/// fixture corpus itself (`PHASE_1` §P1-D11: "fixture seed corpus").
const FUZZ_TARGETS: &[(&str, &[(&str, &str)])] = &[
    ("dsl_parse", &[("testdata/dsl", "dsl")]),
    ("dsl_fmt", &[("testdata/dsl", "dsl")]),
    (
        "json_read",
        &[
            ("testdata/golden", "json"),
            ("testdata/read-corpus", "json"),
            ("testdata/validation", "json"),
            ("testdata/probes", "json"),
        ],
    ),
    // §P4T T-5 (gap 3): the Phase-4 parsers. Importer seeds are the
    // authored fixture corpus; the query/pathfilter grammars ride their
    // committed seed tables under fuzz/seeds/ (the corpus dir stays
    // gitignored scratch); diff_pair opens the vendored `.dsl` set.
    (
        "import_sniff",
        &[
            ("crates/oxidizr-import/tests/fixtures/c4", "puml"),
            ("crates/oxidizr-import/tests/fixtures/mermaid", "mmd"),
        ],
    ),
    ("query_parse", &[("fuzz/seeds/query_parse", "txt")]),
    ("pathfilter", &[("fuzz/seeds/pathfilter", "txt")]),
    ("diff_pair", &[("testdata/dsl", "dsl")]),
];

/// Fuzz job mirror (nightly in CI): build all targets once, stage the fixture
/// seeds, then run each target time-boxed. Crash artifacts land in
/// `fuzz/artifacts/<target>/` and the nonzero exit fails the job.
fn cmd_fuzz(secs: u64) -> Result<()> {
    let root = repo_root()?;

    // The nix dev shell exports CC/CXX/AR/RANLIB for the mingw-w64 cross
    // toolchain (windows-gnu leg, flake.nix `windowsCross`). cc-rs obeys them
    // even for HOST builds — libfuzzer-sys compiles its libFuzzer runtime
    // with the mingw g++ and even picks libFuzzer's *Windows* sources for a
    // Linux target. CI runners have a clean environment; stripping locally
    // keeps this mirror equal to CI (agents/notes/env/fuzz-host-toolchain.md).
    let mk_cmd = || -> Result<Command> {
        let mut cmd = cargo_cmd()?;
        for key in ["CC", "CXX", "AR", "RANLIB"] {
            cmd.env_remove(key);
        }
        Ok(cmd)
    };

    println!("\n==> fuzz build `$ cargo fuzz build`");
    let status = mk_cmd()?.args(["fuzz", "build"]).status().context(
        "cargo fuzz not found — the nix dev shell provides it (agents/plan/WORKFLOW.md)",
    )?;
    ensure!(status.success(), "cargo fuzz build failed");

    for (target, seeds) in FUZZ_TARGETS {
        // The corpus dir is writable scratch (gitignored): libFuzzer stores
        // new interesting inputs right next to the staged fixtures.
        let dir = root.join("fuzz/corpus").join(target);
        fs::create_dir_all(&dir)?;
        let mut staged = 0usize;
        for (sub, ext) in *seeds {
            let leaf = Path::new(sub)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            for file in walk_files(&root.join(sub), ext)? {
                let name = file
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                fs::copy(&file, dir.join(format!("{leaf}-{name}")))?;
                staged += 1;
            }
        }
        println!("\n==> fuzz `{target}` ({staged} seeds, {secs}s) `$ cargo fuzz run {target}`");
        let status = mk_cmd()?
            .args([
                "fuzz",
                "run",
                target,
                "--",
                &format!("-max_total_time={secs}"),
                "-timeout=10",
                "-rss_max_mb=2048",
            ])
            .status()
            .context("failed to run cargo fuzz")?;
        ensure!(
            status.success(),
            "fuzz target `{target}` FAILED (crashes: fuzz/artifacts/{target}/)"
        );
    }
    println!("\nfuzz: all targets clean");
    Ok(())
}

// ---------------------------------------------------------------- oracle plumbing

/// §P4-M5 E-1: the discovery/export/normalize PRIMITIVES live in
/// `oxidizr_cli::oracle` (shared with the `oxidizr oracle` user command,
/// P4-D6 "xtask stays a dev shim"); the legs below drive them over the
/// VENDORED corpus. Error text moved with the functions.
fn find_oracle() -> Result<PathBuf> {
    Ok(oxidizr_cli::oracle::find_oracle(None)?)
}

/// The oracle export prologue (2R R-tool-01: the create-dirs + run + exit-
/// status leg the json/plantuml/theme runners used to triplicate). Delegated
/// to the shared engine since §P4-M5 E-1.
fn run_oracle_export(oracle: &Path, workspace: &Path, outdir: &Path, format: &str) -> Result<()> {
    Ok(oxidizr_cli::oracle::run_export(
        oracle, workspace, outdir, format,
    )?)
}

/// Run `structurizr export --format json` and return the produced file path.
fn reference_export_json(oracle: &Path, workspace: &Path, outdir: &Path) -> Result<PathBuf> {
    run_oracle_export(oracle, workspace, outdir, "json")?;
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

/// Run `structurizr export --format <format>` (plantuml, plantuml/c4plantuml
/// or mermaid) and return the `<ext>` files the app wrote (sorted by name).
/// The reference names diagram files `structurizr-<workspaceId>-<key><ext>`
/// (+ a `-key` legend file per view with a legend — the C4 and Mermaid
/// exporters emit none, their `createLegend` is null); an empty set is
/// legitimate only for viewless workspaces, which the golden corpus excludes
/// (nothing to pin).
fn reference_export_plantuml(
    oracle: &Path,
    workspace: &Path,
    outdir: &Path,
    format: &str,
    ext: &str,
) -> Result<Vec<PathBuf>> {
    run_oracle_export(oracle, workspace, outdir, format)?;
    walk_files(outdir, ext)
}

/// Run `structurizr export --format theme` and return the single
/// `<stem>-theme.json` the reference wrote (workspace-stem naming, M0).
fn reference_export_theme(oracle: &Path, workspace: &Path, outdir: &Path) -> Result<PathBuf> {
    run_oracle_export(oracle, workspace, outdir, "theme")?;
    let stem = workspace
        .file_stem()
        .and_then(|s| s.to_str())
        .context("fixture without usable file stem")?;
    let produced = outdir.join(format!("{stem}-theme.json"));
    ensure!(
        produced.is_file(),
        "expected output missing: {}",
        produced.display()
    );
    Ok(produced)
}

/// Golden normalization — §P4-M5 E-1: the function MOVED to
/// `oxidizr_cli::oracle` (shared with the `oxidizr oracle` user command);
/// the corpus legs call it through this dev-shim wrapper (P4-D6). The unit
/// tests moved with it.
fn normalize_reference_output(raw: &[u8]) -> Vec<u8> {
    oxidizr_cli::oracle::normalize_reference_output(raw)
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
             ## Import fixtures (`!docs`/`!adrs` — shipped at P4-M2, D7 retired by P4-D13)\n\n\
             These fixtures run the real importers at context close (`end()`); the OXZ-5003\n\
             deferral is a tombstone (P4-M2, `agents/plan/build/IMPORT-DOCS-ADRS.md`). The\n\
             machine-readable corpus classification (every corpus file: clean or registered\n\
             first-error + code) is the `KNOWN_ERRORS` register in\n\
             `crates/oxidizr-dsl/tests/m4_fixtures.rs` — `docs/`/`decisions/` keep failing\n\
             on their `com.structurizr.example.*` importer FQNs (\"… was not found\", exactly\n\
             like the reference without plugin jars); the `big-bank-plc` and\n\
             `include-directory` entries now import cleanly.\n\n\
             {}\n",
            deferred
                .iter()
                .map(|p| format!("- `{p}`"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    )?;

    println!(
        "vendored {} .dsl fixtures + {} JSON files into testdata/ ({} import fixtures)",
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
    // (WRITE-CATALOG witnesses), including the JSON ones (`m3b-defaults` is a
    // read→write byte witness), as are the read-corpus workspaces.
    candidates.extend(walk_files(&root.join("testdata").join("probes"), ".dsl")?);
    candidates.extend(walk_files(&root.join("testdata").join("probes"), ".json")?);
    candidates.extend(walk_files(
        &root.join("testdata").join("read-corpus"),
        ".json",
    )?);
    candidates.retain(|p| p.file_stem().is_some_and(|s| s == name));
    match candidates.as_slice() {
        [] => bail!("fixture `{name}` not found under testdata/dsl|probes|read-corpus"),
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

/// The `oracle gen` golden mode — the mutually exclusive flag combination
/// (JSON workspace goldens by default; diagram corpora per dialect; the
/// theme corpus from M4-d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenMode {
    Json,
    Plantuml,
    C4,
    Mermaid,
    Wsd,
    Theme,
}

/// `oracle gen` option parsing: `--fixture NAME` repeats, `--plantuml`
/// selects the diagram goldens, `--c4` (with `--plantuml`) the
/// `plantuml/c4plantuml` dialect, `--mermaid` the Mermaid goldens
/// (`golden/mermaid/`, M4-d), `--wsd` the `WebSequenceDiagrams` goldens
/// (`golden/websequencediagrams/`, P4 §P4-M2 N-5) and `--theme` the theme
/// goldens (`golden/theme/`, also M4-d).
fn oracle_gen_args(oracle: &Path, args: &mut impl Iterator<Item = String>) -> Result<()> {
    let mut fixtures = Vec::new();
    let mut plantuml = false;
    let mut c4 = false;
    let mut mermaid = false;
    let mut wsd = false;
    let mut theme = false;
    while let Some(a) = args.next() {
        if a == "--plantuml" {
            plantuml = true;
            continue;
        }
        if a == "--c4" {
            c4 = true;
            continue;
        }
        if a == "--mermaid" {
            mermaid = true;
            continue;
        }
        if a == "--wsd" {
            wsd = true;
            continue;
        }
        if a == "--theme" {
            theme = true;
            continue;
        }
        ensure!(a == "--fixture", "unexpected argument `{a}`");
        let name = args.next().context("--fixture needs a name")?;
        fixtures.push(name);
    }
    ensure!(!c4 || plantuml, "oracle gen --c4 needs --plantuml too");
    ensure!(
        usize::from(plantuml) + usize::from(mermaid) + usize::from(wsd) + usize::from(theme) <= 1,
        "oracle gen: --plantuml, --mermaid, --wsd and --theme are mutually exclusive"
    );
    let mode = if plantuml && c4 {
        GenMode::C4
    } else if plantuml {
        GenMode::Plantuml
    } else if mermaid {
        GenMode::Mermaid
    } else if wsd {
        GenMode::Wsd
    } else if theme {
        GenMode::Theme
    } else {
        GenMode::Json
    };
    if mode == GenMode::Json && fixtures.is_empty() {
        fixtures.push("workspace-properties".into());
    }
    ensure!(
        mode == GenMode::Json || !fixtures.is_empty(),
        "oracle gen --plantuml/--mermaid/--wsd/--theme needs at least one --fixture"
    );
    cmd_oracle_gen(oracle, &fixtures, mode)
}

fn cmd_oracle_gen(oracle: &Path, fixtures: &[String], mode: GenMode) -> Result<()> {
    let root = repo_root()?;
    let golden_dir = root.join("testdata").join("golden");
    if mode == GenMode::Plantuml
        || mode == GenMode::C4
        || mode == GenMode::Mermaid
        || mode == GenMode::Wsd
    {
        let (subdir, format, ext) = match mode {
            GenMode::Mermaid => ("mermaid", "mermaid", ".mmd"),
            GenMode::Wsd => ("websequencediagrams", "websequencediagrams", ".wsd"),
            GenMode::C4 => ("plantuml-c4", "plantuml/c4plantuml", ".puml"),
            _ => ("plantuml", "plantuml", ".puml"),
        };
        let base = golden_dir.join(subdir);
        for name in fixtures {
            let dsl = find_fixture(&root, name)?;
            let tmp = scratch(&format!("{name}-{subdir}"));
            let produced = reference_export_plantuml(oracle, &dsl, &tmp, format, ext)
                .with_context(|| format!("oracle gen --{subdir} `{name}`"))?;
            ensure!(
                !produced.is_empty(),
                "oracle gen --{subdir} `{name}`: reference produced no {ext} files \
                 (viewless workspace — nothing to pin)"
            );
            let out = base.join(name);
            let _ = fs::remove_dir_all(&out);
            fs::create_dir_all(&out)?;
            for path in &produced {
                let file = path.file_name().context("bad diagram file name")?;
                fs::copy(path, out.join(file))?;
            }
            let _ = fs::remove_dir_all(&tmp);
            println!("golden: {}/ ({} {ext})", out.display(), produced.len());
        }
        return Ok(());
    }
    if mode == GenMode::Theme {
        let base = golden_dir.join("theme");
        fs::create_dir_all(&base)?;
        for name in fixtures {
            let dsl = find_fixture(&root, name)?;
            let tmp = scratch(&format!("{name}-theme"));
            let produced = reference_export_theme(oracle, &dsl, &tmp)
                .with_context(|| format!("oracle gen --theme `{name}`"))?;
            let golden = fs::read(&produced)?;
            let _ = fs::remove_dir_all(&tmp);
            let out = base.join(format!("{name}.json"));
            fs::write(&out, &golden)?;
            println!("golden: {} ({} bytes)", out.display(), golden.len());
        }
        return Ok(());
    }
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
    // 2R R-tool-01: the legs answer their failure LINES instead of mutating
    // a shared counter; this fold prints them (after the streaming `ok:`
    // lines) and bails with the count, as the old `failures == 0` did.
    let (goldens, mut failed) = verify_json_legs(oracle, &root)?;
    let (sidecar_sets, sidecar_failed) = verify_sidecar_legs(oracle, &root)?;
    failed.extend(sidecar_failed);
    let (diagram_sets, diagram_failed) = verify_diagram_legs(oracle, &root)?;
    failed.extend(diagram_failed);
    let (theme_sets, theme_failed) = verify_theme_legs(oracle, &root)?;
    failed.extend(theme_failed);
    for line in &failed {
        println!("{line}");
    }
    ensure!(
        failed.is_empty(),
        "{} golden(s) failed verification",
        failed.len()
    );
    println!(
        "oracle verify: {} json + {sidecar_sets} sidecar + {diagram_sets} diagram + {theme_sets} theme golden set(s) match",
        goldens.len()
    );
    Ok(())
}

/// `cargo xtask oracle landscape` — §P4-M3 O-4 Category B leg (run
/// sparingly, like `export-verify`). Writes the witness alpha/beta
/// fixtures under the scratch dir, runs BOTH generators (reference
/// `generate system-landscape -i <dir> -o out.json` with `TZ=UTC`, ours
/// in-process through [`oxidizr_api::pipeline::execute`]) and compares
/// SEMANTIC fact vectors: workspace name + scope, the element
/// `kind|name|description|url` set and the relationship
/// `source -> destination | description` triples. Endpoint ids are
/// RESOLVED TO NAMES (the shared counter legitimately differs where the
/// scan order differs); properties are not compared at all (the loader
/// regenerates the `structurizr.*` keys per parse).
/// The witness alpha/beta fixtures for the `oracle landscape` leg (the
/// same shapes the 2026-11-21 witness ran: Alice merge + container-bearing
/// systems + the `First`-strategy edge set).
const LANDSCAPE_FIXTURES: [(&str, &str); 2] = [
    (
        "alpha",
        "workspace \"Alpha\" {\n\
         \x20   model {\n\
         \x20       alice = person \"Alice\" \"Alpha Alice\" {\n\
         \x20           url \"https://alpha.example/alice\"\n\
         \x20           tag \"Staff\"\n\
         \x20       }\n\
         \x20       billing = softwareSystem \"Billing\" \"Alpha billing description\" {\n\
         \x20           api = container \"Billing API\"\n\
         \x20       }\n\
         \x20       crm = softwareSystem \"CRM\" \"Alpha CRM description\"\n\
         \x20       alice -> billing \"Uses\" \"HTTP\" {\n\
         \x20           url \"https://rel/uses\"\n\
         \x20           tag \"CustomRel\"\n\
         \x20       }\n\
         \x20       alice -> crm \"Uses\"\n\
         \x20       billing -> crm \"Synchronises with\"\n\
         \x20   }\n\
         }\n",
    ),
    (
        "beta",
        "workspace \"Beta\" {\n\
         \x20   model {\n\
         \x20       alice = person \"Alice\" \"Beta Alice\" {\n\
         \x20           tag \"External\"\n\
         \x20       }\n\
         \x20       crm = softwareSystem \"CRM\" \"Beta CRM description\" {\n\
         \x20           ui = container \"CRM UI\"\n\
         \x20       }\n\
         \x20       warehouse = softwareSystem \"Warehouse\" \"Beta warehouse\"\n\
         \x20       alice -> crm \"Uses\"\n\
         \x20       alice -> crm \"Reports on\"\n\
         \x20       crm -> warehouse \"Pushes orders to\"\n\
         \x20   }\n\
         }\n",
    ),
];

fn cmd_oracle_landscape(oracle: &Path) -> Result<()> {
    use oxidizr_api::pipeline::{ApiOptions, ExitClass, Request, execute};
    const FIXTURES: [(&str, &str); 2] = LANDSCAPE_FIXTURES;
    let tmp = scratch("landscape");
    let _ = fs::remove_dir_all(&tmp);
    for (tag, text) in FIXTURES {
        let dir = tmp.join(tag);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("workspace.dsl"), text)?;
    }
    let produced = tmp.join("landscape.json");
    let output = Command::new(oracle)
        .env("TZ", "UTC")
        .arg("generate")
        .arg("system-landscape")
        .arg("-i")
        .arg(&tmp)
        .arg("-o")
        .arg(&produced)
        .output()
        .with_context(|| format!("failed to run {}", oracle.display()))?;
    ensure!(
        output.status.success(),
        "reference generate system-landscape failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let theirs = fs::read(&produced)?;
    let ours = execute(
        Request::Generate {
            inputs: vec![tmp.clone()],
            include: None,
            exclude: None,
            strategy: oxidizr_api::RelationshipsStrategy::First,
            output: None,
        },
        &ApiOptions::default(),
    );
    let _ = fs::remove_dir_all(&tmp);
    ensure!(
        matches!(ours.exit, ExitClass::Ok),
        "oxidizr generate system-landscape exited {:?}: {:?}",
        ours.exit,
        ours.log
    );
    let ours = ours.bytes.context("oxidizr generate produced no bytes")?;
    let ours = landscape_facts(&ours)?;
    let theirs = landscape_facts(&theirs)?;
    if ours == theirs {
        println!("ok: landscape facts match ({} facts)", ours.len());
        return Ok(());
    }
    let mut failed = Vec::new();
    for line in ours.iter().filter(|l| !theirs.contains(l)) {
        failed.push(format!("MISMATCH ours only: {line}"));
    }
    for line in theirs.iter().filter(|l| !ours.contains(l)) {
        failed.push(format!("MISMATCH theirs only: {line}"));
    }
    for line in &failed {
        println!("{line}");
    }
    bail!(
        "landscape facts differ ({} mismatched fact line(s))",
        failed.len()
    );
}

/// The semantic fact vector of a landscape JSON (sorted within sections;
/// endpoint ids resolved through the document's own element ids).
fn landscape_facts(bytes: &[u8]) -> Result<Vec<String>> {
    let doc: serde_json::Value = serde_json::from_slice(bytes).context("landscape JSON")?;
    let text = |v: &serde_json::Value| v.as_str().unwrap_or_default().to_string();
    let mut facts = vec![
        format!("name = {}", text(&doc["name"])),
        format!("scope = {}", text(&doc["configuration"]["scope"])),
    ];
    let mut names = serde_json::Map::new();
    let mut elements = Vec::new();
    for (key, kind) in [("people", "Person"), ("softwareSystems", "Software System")] {
        for element in doc["model"][key].as_array().cloned().unwrap_or_default() {
            names.insert(
                text(&element["id"]),
                serde_json::Value::String(text(&element["name"])),
            );
            elements.push(format!(
                "element {kind} | {} | {} | {}",
                text(&element["name"]),
                text(&element["description"]),
                text(&element["url"])
            ));
        }
    }
    let mut rels = Vec::new();
    for key in ["people", "softwareSystems"] {
        for element in doc["model"][key].as_array().cloned().unwrap_or_default() {
            let source = text(&element["name"]);
            for rel in element["relationships"]
                .as_array()
                .cloned()
                .unwrap_or_default()
            {
                let destination = names
                    .get(&text(&rel["destinationId"]))
                    .map_or_else(|| "?".to_string(), text);
                rels.push(format!(
                    "rel {source} -> {destination} | {}",
                    text(&rel["description"])
                ));
            }
        }
    }
    elements.sort();
    rels.sort();
    facts.extend(elements);
    facts.extend(rels);
    Ok(facts)
}

/// Top-level `golden/*.json` only — the subdirectories are the diagram
/// (plantuml[-c4], mermaid) and theme corpora, verified by their legs.
fn verify_json_legs(oracle: &Path, root: &Path) -> Result<(Vec<PathBuf>, Vec<String>)> {
    let mut failed: Vec<String> = Vec::new();
    let mut goldens: Vec<PathBuf> = fs::read_dir(root.join("testdata").join("golden"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect();
    goldens.sort();
    ensure!(!goldens.is_empty(), "no committed goldens to verify");
    for golden_path in &goldens {
        let name = golden_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("bad golden name")?;
        let dsl = find_fixture(root, name)?;
        let tmp = scratch(name);
        let produced = reference_export_json(oracle, &dsl, &tmp)
            .with_context(|| format!("oracle verify `{name}`"))?;
        let fresh = normalize_reference_output(&fs::read(&produced)?);
        let _ = fs::remove_dir_all(&tmp);
        let committed = fs::read(golden_path)?;
        if fresh == committed {
            println!("ok: {name}");
        } else {
            failed.push(format!(
                "MISMATCH: {name} (reference output drifted from committed golden)"
            ));
        }
    }
    Ok((goldens, failed))
}

/// Saved-sidecar legs (P3-M5 / M5-1, deferred from M3-5): the reference
/// must load the JSON the viewer's save path writes. Per top-level JSON
/// golden fixture: (1) IDENTITY — open the fixture through the real api
/// pipeline, save it with `commands::write_workspace` (the Ctrl+S path),
/// and compare the reference's json→json re-export of OUR sidecar with
/// its re-export of the committed golden, byte-equal after normalization.
/// Comparing the two RE-EXPORTS isolates our writer from any reference
/// json→json quirk. (2) EDITED — run the bundled layered engine over the
/// first non-filtered view with ≥ 2 items, save the overlay through
/// `apply_overlay` + `write_workspace`, and require the reference to load
/// it: the `model` section unchanged, every written view placement
/// (id → x/y) surviving the load VERBATIM, and the placement map
/// different from the stored one (the geometry actually flowed).
fn verify_sidecar_legs(oracle: &Path, root: &Path) -> Result<(usize, Vec<String>)> {
    use oxidizr_api::open;
    use oxidizr_api::pipeline::{ApiOptions, Input};
    use oxidizr_viewer::commands;

    let mut failed: Vec<String> = Vec::new();
    let mut fixtures = 0_usize;
    let mut goldens: Vec<PathBuf> = fs::read_dir(root.join("testdata").join("golden"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
        .collect();
    goldens.sort();
    for golden_path in &goldens {
        let name = golden_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("bad golden name")?;
        let fixture = find_fixture(root, name)?;
        let opened =
            open(&Input::File(fixture.clone()), &ApiOptions::default()).map_err(|outcome| {
                anyhow::anyhow!(
                    "open {} failed (exit {:?})",
                    fixture.display(),
                    outcome.exit
                )
            })?;
        let workspace = &opened.workspace;
        let tmp = scratch(name);
        fs::create_dir_all(&tmp)?;

        // (1) IDENTITY: the save path with zero edits, vs the golden.
        let sidecar = tmp.join(format!("{name}.json"));
        commands::write_workspace(&sidecar, workspace)
            .with_context(|| format!("sidecar save `{name}`"))?;
        let ours = reference_export_json(oracle, &sidecar, &tmp.join("ours"))
            .with_context(|| format!("oracle sidecar load `{name}`"))?;
        let ours_norm = normalize_reference_output(&fs::read(&ours)?);
        let theirs = reference_export_json(oracle, golden_path, &tmp.join("theirs"))
            .with_context(|| format!("oracle golden re-export `{name}`"))?;
        let theirs_norm = normalize_reference_output(&fs::read(&theirs)?);
        if ours_norm == theirs_norm {
            println!("ok: {name} (sidecar identity)");
        } else {
            failed.push(format!(
                "MISMATCH: {name} (the reference's re-export of our saved sidecar \
                 differs from its re-export of the committed golden)"
            ));
        }

        // (2) EDITED: real layered geometry through the reference loader.
        let edited_ok = sidecar_edited_leg(oracle, &tmp, name, workspace, &ours_norm, &mut failed)?;
        if edited_ok {
            println!("ok: {name} (sidecar edited)");
        } else if !failed.iter().any(|l| l.contains(name)) {
            println!("ok: {name} (sidecar edited skipped — no view with >= 2 items)");
        }
        let _ = fs::remove_dir_all(&tmp);
        fixtures += 1;
    }
    Ok((fixtures, failed))
}

/// The EDITED half of the saved-sidecar leg (§P3-M5 M5-1): layered
/// geometry for the first non-filtered view with ≥ 2 items, saved through
/// `apply_overlay` + the viewer save path, loaded back by the reference.
/// Failures append lines; the answer says whether the leg actually ran.
fn sidecar_edited_leg(
    oracle: &Path,
    tmp: &Path,
    name: &str,
    workspace: &oxidizr_core::Workspace,
    ours_norm: &[u8],
    failed: &mut Vec<String>,
) -> Result<bool> {
    use oxidizr_core::views::{AutomaticLayout, ColorScheme, View};
    use oxidizr_viewer::{commands, layout, scene};

    for view in workspace.views.all_views() {
        if matches!(view, View::Filtered(_)) {
            continue;
        }
        let Ok(built) = scene::build(workspace, view.key(), ColorScheme::Light, None, None) else {
            continue;
        };
        if built.items.len() < 2 {
            continue;
        }
        let overrides = layout::apply(&built, &AutomaticLayout::default())
            .with_context(|| format!("sidecar edited `{name}`: layout run"))?;
        let overlay = commands::LayoutOverlay {
            view_key: view.key().to_owned(),
            elements: overrides.elements,
            relationships: overrides.relationships,
            dimensions: None,
        };
        let applied = commands::apply_overlay(workspace, view.key(), &overlay);
        let edited = tmp.join(format!("{name}-edited.json"));
        commands::write_workspace(&edited, &applied)
            .with_context(|| format!("sidecar edited save `{name}`"))?;
        let fresh = reference_export_json(oracle, &edited, &tmp.join("edited"))
            .with_context(|| format!("oracle EDITED-sidecar load `{name}`"))?;
        let fresh_val: serde_json::Value =
            serde_json::from_slice(&normalize_reference_output(&fs::read(&fresh)?))?;
        let ours_val: serde_json::Value = serde_json::from_slice(ours_norm)?;
        let written_val: serde_json::Value = serde_json::from_slice(&fs::read(&edited)?)?;
        if fresh_val.get("model") != ours_val.get("model") {
            failed.push(format!(
                "MISMATCH: {name} (the edited sidecar lost model content \
                 through the reference loader)"
            ));
            return Ok(false);
        }
        let mut written = BTreeMap::new();
        collect_xy_maps(&written_val, &mut written);
        let mut echoed = BTreeMap::new();
        collect_xy_maps(&fresh_val, &mut echoed);
        let mut stored = BTreeMap::new();
        collect_xy_maps(&ours_val, &mut stored);
        if written != echoed {
            failed.push(format!(
                "MISMATCH: {name} (view placements changed through the \
                 reference load: {} written vs {} echoed)",
                written.len(),
                echoed.len()
            ));
        } else if written == stored {
            failed.push(format!(
                "MISMATCH: {name} (the edited sidecar kept the stored \
                 placements — the overlay did not flow)"
            ));
        } else {
            return Ok(true);
        }
        return Ok(false);
    }
    Ok(false)
}

/// id → (x, y) for every JSON object carrying all three (elementViews,
/// deployment-node views…). Vertices (x/y without an id) never match;
/// integer coordinates only (the stored grid, `to_px`).
fn collect_xy_maps(value: &serde_json::Value, out: &mut BTreeMap<String, (i64, i64)>) {
    match value {
        serde_json::Value::Object(map) => {
            let id = map.get("id").and_then(serde_json::Value::as_str);
            let x = map.get("x").and_then(serde_json::Value::as_i64);
            let y = map.get("y").and_then(serde_json::Value::as_i64);
            if let (Some(id), Some(x), Some(y)) = (id, x, y) {
                out.insert(id.to_owned(), (x, y));
            }
            for child in map.values() {
                collect_xy_maps(child, out);
            }
        }
        serde_json::Value::Array(items) => {
            for child in items {
                collect_xy_maps(child, out);
            }
        }
        _ => {}
    }
}

/// Diagram goldens (M2-d, C4 leg M3-d, Mermaid leg M4-d):
/// `testdata/golden/<subdir>/<fixture>/*<ext>`, byte-exact (no
/// normalization — the diagram writer injects no harness artifacts).
/// The fixture name is the directory name.
fn verify_diagram_legs(oracle: &Path, root: &Path) -> Result<(usize, Vec<String>)> {
    let mut diagram_sets = 0_usize;
    let mut failed: Vec<String> = Vec::new();
    for (subdir, format, ext) in [
        ("plantuml", "plantuml", ".puml"),
        ("plantuml-c4", "plantuml/c4plantuml", ".puml"),
        ("mermaid", "mermaid", ".mmd"),
        ("websequencediagrams", "websequencediagrams", ".wsd"),
    ] {
        let puml_base = root.join("testdata").join("golden").join(subdir);
        let mut puml_dirs: Vec<PathBuf> = fs::read_dir(&puml_base)
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_dir())
                    .collect()
            })
            .unwrap_or_default();
        puml_dirs.sort();
        for dir in &puml_dirs {
            let name = dir
                .file_stem()
                .and_then(|s| s.to_str())
                .context("bad fixture")?;
            let dsl = find_fixture(root, name)?;
            let tmp = scratch(&format!("{name}-{subdir}"));
            let fresh = reference_export_plantuml(oracle, &dsl, &tmp, format, ext)
                .with_context(|| format!("oracle verify `{name}` ({subdir})"))?;
            let mut fresh_files: BTreeSet<(String, Vec<u8>)> = BTreeSet::new();
            for path in &fresh {
                let n = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                fresh_files.insert((n.to_owned(), fs::read(path)?));
            }
            let _ = fs::remove_dir_all(&tmp);
            let mut committed_files: BTreeSet<(String, Vec<u8>)> = BTreeSet::new();
            for path in walk_files(dir, ext)? {
                let n = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                committed_files.insert((n.to_owned(), fs::read(&path)?));
            }
            if fresh_files == committed_files {
                println!("ok: {name} ({subdir}, {} files)", fresh_files.len());
            } else {
                failed.push(format!(
                    "MISMATCH: {name} ({subdir} output drifted from committed goldens)"
                ));
            }
            diagram_sets += 1;
        }
    }
    Ok((diagram_sets, failed))
}

/// Theme goldens (M4-d): `testdata/golden/theme/<fixture>.json`, the raw
/// `--format theme` output (`ThemeUtils.toJson`, P2-V1 — no harness
/// artifacts), byte-exact.
fn verify_theme_legs(oracle: &Path, root: &Path) -> Result<(usize, Vec<String>)> {
    let mut theme_sets = 0_usize;
    let mut failed: Vec<String> = Vec::new();
    let mut theme_goldens: Vec<PathBuf> =
        fs::read_dir(root.join("testdata").join("golden").join("theme"))
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == "json"))
                    .collect()
            })
            .unwrap_or_default();
    theme_goldens.sort();
    for golden_path in &theme_goldens {
        let name = golden_path
            .file_stem()
            .and_then(|s| s.to_str())
            .context("bad theme golden name")?;
        let dsl = find_fixture(root, name)?;
        let tmp = scratch(&format!("{name}-theme"));
        let produced = reference_export_theme(oracle, &dsl, &tmp)
            .with_context(|| format!("oracle verify `{name}` (theme)"))?;
        let fresh = fs::read(&produced)?;
        let _ = fs::remove_dir_all(&tmp);
        let committed = fs::read(golden_path)?;
        if fresh == committed {
            println!("ok: {name} (theme)");
        } else {
            failed.push(format!(
                "MISMATCH: {name} (theme output drifted from committed golden)"
            ));
        }
        theme_sets += 1;
    }
    Ok((theme_sets, failed))
}

/// `cargo xtask export-verify [--format c4|plantuml|mermaid] [--fixture N]…`
/// — P2-D9 + §P4-M5 E-2 semantic comparator (Category B, run sparingly).
/// Requires `$OXIDZR_STRUCTURIZR_BIN` (or `structurizr` on PATH) AND a
/// pre-built `oxidizr` binary (`cargo build -p oxidizr-cli`). Runs both
/// exporters over each fixture, parses both into [`exportcmp`] fact
/// multisets (order-normalized; whitelist = comment/blank lines only) and
/// diffs. The C4 macro inventory lives in [`exportcmp::C4_MACROS`].
fn cmd_export_verify(args: &mut impl Iterator<Item = String>) -> Result<()> {
    let oracle = find_oracle()?;
    let root = repo_root()?;
    let ours =
        env::var_os("OXIDZR_BIN").map_or_else(|| root.join("target/debug/oxidizr"), PathBuf::from);
    ensure!(
        ours.is_file(),
        "oxidizr binary not found at {} (set $OXIDZR_BIN or run `cargo build -p oxidizr-cli`)",
        ours.display()
    );
    let mut format = "c4".to_string();
    let mut fixtures: Vec<String> = Vec::new();
    let mut explicit = false;
    while let Some(a) = args.next() {
        if a == "--fixture" {
            explicit = true;
            let name = args.next().context("--fixture needs a name")?;
            fixtures.push(name);
        } else if a == "--format" {
            format = args.next().context("--format needs a name")?;
        } else {
            bail!("unexpected argument `{a}`");
        }
    }
    // Dialect table: (default-corpus dir, exporter format, extension,
    // fact grammar). The goldens dirs double as the fixture-name inventory.
    let (corpus, fmt, ext, facts): (&str, &str, &str, fn(&str) -> exportcmp::FactSet) =
        match format.as_str() {
            "c4" => (
                "plantuml-c4",
                "plantuml/c4plantuml",
                ".puml",
                exportcmp::facts,
            ),
            "plantuml" => ("plantuml", "plantuml", ".puml", exportcmp::facts_plantuml),
            "mermaid" => ("mermaid", "mermaid", ".mmd", exportcmp::facts_mermaid),
            other => bail!("unknown export-verify format `{other}` (c4 | plantuml | mermaid)"),
        };
    // Default corpus: all goldens for the dialect (already byte-pinned) +
    // `bulk-operations` (D13: byte-order deviation but semantically equal —
    // E-2 witnessed the same line-reorder drift in the other two dialects).
    if !explicit {
        let base = root.join("testdata").join("golden").join(corpus);
        if let Ok(entries) = fs::read_dir(&base) {
            for e in entries.flatten() {
                if e.path().is_dir() {
                    if let Some(n) = e.path().file_name().and_then(|n| n.to_str()) {
                        fixtures.push(n.to_owned());
                    }
                }
            }
        }
        if !fixtures.iter().any(|f| f == "bulk-operations") {
            fixtures.push("bulk-operations".into());
        }
        fixtures.sort();
    }
    ensure!(!fixtures.is_empty(), "no fixtures to verify");
    let mut failures = 0_usize;
    for name in &fixtures {
        let dsl = find_fixture(&root, name)?;
        let tmp_ref = scratch(&format!("{name}-ev-ref"));
        let tmp_our = scratch(&format!("{name}-ev-our"));
        let ref_files = reference_export_plantuml(&oracle, &dsl, &tmp_ref, fmt, ext)
            .with_context(|| format!("export-verify `{name}` (oracle)"))?;
        let our_files = run_oxidizr_export(&ours, &dsl, &tmp_our, fmt, ext)
            .with_context(|| format!("export-verify `{name}` (oxidizr)"))?;
        let diffs = compare_export_dirs(&ref_files, &our_files, facts)?;
        let _ = fs::remove_dir_all(&tmp_ref);
        let _ = fs::remove_dir_all(&tmp_our);
        if diffs.is_empty() {
            println!("ok: {name} (semantic)");
        } else {
            failures += 1;
            println!("MISMATCH: {name}");
            for d in &diffs {
                println!("  {d}");
            }
        }
    }
    ensure!(
        failures == 0,
        "{failures} fixture(s) failed semantic comparison"
    );
    println!(
        "export-verify ({fmt}): {} fixture(s) semantically match",
        fixtures.len()
    );
    Ok(())
}

/// Run `oxidizr export --format <fmt>` over `workspace` and return the
/// diagram files produced with extension `ext` (sorted).
fn run_oxidizr_export(
    bin: &Path,
    workspace: &Path,
    outdir: &Path,
    format: &str,
    ext: &str,
) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(outdir)?;
    let output = Command::new(bin)
        .arg("export")
        .arg("--workspace")
        .arg(workspace)
        .arg("--format")
        .arg(format)
        .arg("--output")
        .arg(outdir)
        .arg("--quiet")
        .output()
        .with_context(|| format!("failed to run {}", bin.display()))?;
    if !output.status.success() {
        bail!(
            "oxidizr export failed ({}):\n{}",
            workspace.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    walk_files(outdir, ext)
}

/// Semantic (order-insensitive) diff of two diagram file sets, classified
/// with the dialect's fact grammar.
/// Reports: files only in one side; shared files with non-empty fact diffs.
fn compare_export_dirs(
    ref_files: &[PathBuf],
    our_files: &[PathBuf],
    facts: fn(&str) -> exportcmp::FactSet,
) -> Result<Vec<String>> {
    let mut diffs = Vec::new();
    let ref_map: BTreeMap<String, PathBuf> = ref_files
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| (n.to_owned(), p.clone()))
        })
        .collect();
    let our_map: BTreeMap<String, PathBuf> = our_files
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| (n.to_owned(), p.clone()))
        })
        .collect();
    for name in ref_map.keys().filter(|n| !our_map.contains_key(*n)) {
        diffs.push(format!("missing file: {name}"));
    }
    for name in our_map.keys().filter(|n| !ref_map.contains_key(*n)) {
        diffs.push(format!("extra file: {name}"));
    }
    for (name, rp) in &ref_map {
        if let Some(op) = our_map.get(name) {
            let rtext =
                fs::read_to_string(rp).with_context(|| format!("reading {}", rp.display()))?;
            let otext =
                fs::read_to_string(op).with_context(|| format!("reading {}", op.display()))?;
            for d in exportcmp::diff_facts(&facts(&rtext), &facts(&otext)) {
                diffs.push(format!("{name}: {d}"));
            }
        }
    }
    Ok(diffs)
}

#[cfg(test)]
mod tests {
    use super::{Step, resolve_ci_steps};

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
}
