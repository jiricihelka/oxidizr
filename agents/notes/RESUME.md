# RESUME — session checkpoint (updated 2026-11-23, **§P4R CLOSED (`c42d711`), §P4D CLOSED (`41e2a3c`), E-1…E-3 done (M5a–M5c), E-4 cargo-dist wired (M5d, at HEAD) — next and LAST: E-5 (0.4.0 bump + CHANGELOG + `docs/RELEASING.md` + README pass + close ritual + LOCAL `v0.4.0` tag, NEVER push/publish); after that the P4 arc is DONE** )

## Active now (2026-11-23)

- **E-4 DONE (M5d)** — cargo-dist wiring, all green: `dist plan` a clean
  three-release manifest (`oxidizr` via cli `dist = true` [publish-false
  override], `oxidizr-lsp`, `oxidizr-viewer` via `features = ["gui"]`),
  5 targets (gnu linux ×2, macOS ×2, windows-msvc), sha256 + sh/ps1
  installers. **Config = `dist-workspace.toml` (v1), NOT `[workspace.metadata.dist]`**
  — the Cargo.toml form panics cargo-dist (`couldn't find the release!?`)
  whenever installers are set; pitfall note + fix documented at
  `agents/notes/pitfalls/cargo-dist-installer-panic.md`. `.github/workflows/release.yml`
  is pure `dist generate --mode ci` output (PR plan leg = the dry-run); the
  per-OS smoke is the companion `.github/workflows/release-smoke.yml`
  (checksum verify + `--version` tag-match + `dsl2json` on
  `testdata/dsl/color-schemes.dsl`; GUI checksum-verified only).
  `.github/actionlint.yaml` scopes the generated file's SC2086/SC2129
  ignores; `cargo xtask lint` green, quartet green.
- **E-5 NEXT (version tail)** — bump `workspace.package.version`
  `0.1.0`→`0.4.0` (+ `Cargo.lock` + every `--version` golden incl.
  `crates/oxidizr-lsp/tests/version.rs` pin), `CHANGELOG.md` (Keep-a-Changelog:
  v0.1.0–v0.3.0 reconstructed from PROGRESS logs + v0.4.0 section + the 3
  frozen churn lines from `agents/notes/process/api-churn-v0.4.0.md`),
  `docs/RELEASING.md` (tag → CI → dist → changelog checklist; crates.io
  publishing SKIPPED for v0.4.0 — record), README command-list pass
  (through `generate`/`oracle`), then the P4 close ritual: full tier +
  bless 307×unchanged + PROGRESS `P4-M5` log + status board + this file +
  LOCAL `git tag v0.4.0`. **Never push** — user's explicit action.
- **Category B env**: `$OXIDZR_STRUCTURIZR_BIN=/
  nix/store/5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/
  structurizr` (TZ=UTC for the legs).

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

- Sub-piece: per-crate fmt/test/doc-zero + comment-only audit
  (`git diff -U0`, drop `//` lines, strip trailing comments, compare code
  sides pairwise — must match 1:1) → `cargo xtask ci` green → commit.
- Closes: quartet + full tier + bless 307×unchanged + Category B where the
  diff touches generator/parser code.
- refs gate: doc-comment path tokens anchored at `agents/`, `crates/`,
  `testdata/`, `bench-baseline/` must resolve; hygiene leg = no process
  ids/provenance in shipped-crate human text.
- dev shell (`nix develop`): `dist` 0.34.0 + `actionlint` 1.7.12 live here;
  release CI installs its OWN dist via installer.sh.

## Done earlier (this arc, committed)

- E-4 M5d (cargo-dist: `dist-workspace.toml`, generated `release.yml`,
  `release-smoke.yml`, `actionlint.yaml`, pitfall note; row stamped).
- E-3 M5c (Layered polish: `ranks_are_tightest_feasible` + `spread_channels`
  lane pass), E-2 M5b (dialect comparators), E-1 M5a (`oxidizr oracle`).
- P4D closed `41e2a3c` (docs phase, D-1…D-8; hygiene leg in refs forever).
- P4R closed `c42d711` (quartet + full tier + bless + Category B green;
  allow ratchet 106→101; churn frozen for E-5; bench baseline `4T`).
