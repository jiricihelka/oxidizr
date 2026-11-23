# RESUME — session checkpoint (updated 2026-11-23, **PHASE 4 COMPLETE — §P4T/§P4R/§P4D closed, release endgame E-1…E-5 all DONE (M5a `696e849` … M5e `3c34a08`), local annotated tag `v0.4.0` created, post-close Windows CI fix at HEAD (M5f). The P1–P4 arc is DONE. Nothing is pending in-repo: re-pushing `main` (CI was red only on the M5f-fixed test), moving the tag, the GitHub Release and crates.io publishing are the USER's explicit actions (never executed here). Next work = whatever the user opens next; the P5 candidate list lives in `agents/plan/PLAN.md`**)

## State at hand-over (2026-11-23)

- **v0.4.0 (local tag, annotated)** — workspace version bumped
  `0.1.0` → `0.4.0` (root `[workspace.package]` + 13 path-dep pins,
  `cargo update --workspace`; `--version` surfaces derive from
  `CARGO_PKG_VERSION`, so zero goldens moved — bless 307×unchanged).
  `CHANGELOG.md` created (v0.1.0–v0.3.0 reconstructed + v0.4.0 + the
  3 frozen Rust-API churn lines verbatim). `docs/RELEASING.md` written
  (tag → CI → smoke → register; **crates.io publishing SKIPPED for
  v0.4.0** — user decision recorded there).
- **Release wiring (M5d)**: `dist-workspace.toml` (v1 config — the
  Cargo.toml `[workspace.metadata.dist]` form panics cargo-dist with
  installers set, see `agents/notes/pitfalls/cargo-dist-installer-panic.md`);
  pure generated `.github/workflows/release.yml` + companion
  `release-smoke.yml`; `.github/actionlint.yaml` scopes the generated
  file's shellcheck ignores. Release set: `oxidizr` (cli `dist = true`
  override) + `oxidizr-lsp` + `oxidizr-viewer` (`features = ["gui"]`),
  5 targets, shell/PS1 installers.
- **Close evidence (M5e)**: quartet + full e2e tier
  (`OXIDZR_E2E_FULL=1 cargo test -p oxidizr-e2e`) + `cargo xtask lint`
  green; bless **307×unchanged**, `git status --porcelain testdata/`
  empty; Category B SKIPPED with reason (diff = versions/docs, no
  generator/parser change since the P4D close where the oracle evidence
  is green). PROGRESS status board + `P4-M5` milestone log + PHASE_4
  row/item stamps all committed with M5e.

## Workflow (measured, `agents/notes/process/parallelism.md`)

- NO subagents by default (measured memory thrash; hard cap 3). Thinking
  work runs sequentially in the parent; parallelize only harness cycles
  via background shell jobs. Two cargo jobs queue on the target-dir lock —
  chain them in one background command; don't poll background shells.
- Script hygiene: write python to a file under the session tmp dir, never
  `python3 -c` with backticks/`\b` (shell mangling — pitfall note
  `agents/notes/pitfalls/agent-shell-quoting.md`); anchored replacement =
  (approx_line, must-contain fragment, new|None) + marker-normalizing
  rewrite (keeps `///` vs `//` vs `//!`, prevents doc-block corruption);
  `None` deletes the line; apply descending.

## Gates quick reference

- Sub-piece: per-crate fmt/test/doc-zero + comment-only audit →
  `cargo xtask ci` green → commit.
- Closes: quartet + full tier + bless 307×unchanged + `testdata/` clean +
  Category B where the diff touches generator/parser code
  (`OXIDZR_STRUCTURIZR_BIN=/nix/store/5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-
  2026.10.26/bin/structurizr`, legs `TZ=UTC`, background only).
- refs gate: doc-comment path tokens anchored at `agents/`, `crates/`,
  `testdata/`, `bench-baseline/` must resolve; hygiene leg = no process
  ids/provenance in shipped-crate human text.
- dev shell (`nix develop`): `dist` 0.34.0 + `actionlint` 1.7.12 live
  here; release CI installs its OWN dist via installer.sh. After a
  `dist-workspace.toml` change: `dist generate --mode ci` + `dist plan`.
- **Releasing**: follow `docs/RELEASING.md`. Never push from a session.

## Done earlier (committed, this arc)

- M5f (post-close fix: `generate_probes` skip-note assertion compared a
  hardcoded `/` against `Path::display`'s `\` — first native-Windows CI
  run; fix compares the joined+rendered path; host 6/6 + Wine 6/6 +
  quartet green; pitfall note `windows-path-separator-assertions.md`).
- M5e (release chore + P4 close, tag `v0.4.0` local at `3c34a08`),
  M5d (cargo-dist), M5c (Layered polish), M5b (dialect comparators),
  M5a (`oxidizr oracle`).
- P4D closed `41e2a3c`; P4R closed `c42d711` (churn frozen);
  P4T closed (matrix frozen, 307 re-blessed at T-3).
