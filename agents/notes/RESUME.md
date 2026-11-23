# RESUME — session checkpoint (updated 2026-11-23, **§P4R CLOSED (`c42d711`); §P4D IN FLIGHT: D-1 `6f8fd6d`, D-2 `f605cbe`, D-3 `bce0ed9`, D-4 `a33fb16`, D-5 wave-C strip committed (HEAD) — next D-6 (message/string-literal pass — docs/*.md pages already rewritten, uncommitted), D-7 (refs hygiene leg + agent-doc trim), D-8 (close) → release tail E-4/E-5 (cargo-dist + `v0.4.0` tag + CHANGELOG, LOCAL)**)

## Active now (2026-11-23)

- **P4D-D-5 DONE (wave-C strip)** — lsp/viewer/cli/xtask (e2e was 0):
  batteries at KEEP-only (residuals: `oracle`/`oxidizr oracle` command
  names, refs.rs checker-grammar examples, `testdata/probes` path tokens,
  "OS preference"/`f32`/Base64/Java-in-example-strings false positives).
  Viewer README rewritten 104→45 lines human-first; cli clap `///` help
  rewritten (only help assertion is `--theme` at tests/cli.rs:576 — a
  flag, unaffected). Comment-only audit: 0 code-side changes except two
  INTENTIONAL xtask HELP string-literal pairs (`mirror (M6-d):`→`mirror:`,
  `(slow, §P4-M3 O-4)`→`(slow)`) — dev-only binary, done early, flagged
  in the commit note; D-6 skips them. Quartet green, bless 307×unchanged.
  **Trap re-tripped**: the mid-line whitespace collapse
  (`(?<=\S)  +(?=\S)`) crushed 108 3-space bullet-continuation indents
  in viewer module docs + refs.rs → `clippy::doc_lazy_continuation`
  under `-D warnings`; repair = re-indent continuation lines ONLY
  (script kept at `/tmp/…/fix_continuations.py`). NEVER run it again on
  comment lines.
- **Strip scripts reusable**: `/tmp/nix-shell.BoHvza/opencode/{cli,xt,vw,lsp}_strip*.py`
  + `{cli,xt}_batch.py` + `battery_dsl.py` (takes file/dir paths) +
  `fix_continuations.py` (bullet-indent repair); mechanism = approx-line +
  must-contain fragment + marker-normalizing rewrite, `None` deletes,
  apply descending.
- **D-6 NEXT (message pass)** — string literals + docs pages: the five
  `docs/*.md` pages are ALREADY rewritten human-first (uncommitted; ride an
  early D-6 commit). Bless prediction + what may/must not move:
  `agents/notes/process/p4d-d6-bless-prediction.md`. In-test message
  assertions update in the same commit as the message they assert.
  Excludes the two xtask HELP lines (done at D-5).
- **D-7** — hygiene leg in `crates/xtask/src/refs.rs` (FAIL on process ids
  / official-impl narration in public doc comments + human strings of the
  shipped crates; allowlist `agents/`, tests; reuse `check_file`/`check_tree`,
  honor `refs:ignore`) + agent-doc trim.
- **D-8** — close ritual: quartet + full e2e tier + bless (307×unchanged
  expected — D-6 stamped any intended moves) + Category B (`TZ=UTC
  cargo xtask oracle verify` + export-verify ×3, `$OXIDZR_STRUCTURIZR_BIN=/
  nix/store/5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/
  structurizr`) + PROGRESS `P4D` log.
- **E-4/E-5** — cargo-dist via `nix develop` (0.34.0, `cargo dist check`),
  `.github/workflows/release.yml`; then 0.4.0 version bump (all path-deps),
  CHANGELOG.md folding the 3 frozen churn lines from
  `agents/notes/process/api-churn-v0.4.0.md`, LOCAL `v0.4.0` tag.
  NEVER push/publish.

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
- Closes: quartet + full tier + bless 307×unchanged + Category B where
  the diff touches generator/parser code.
- refs gate: doc-comment path tokens anchored at `agents/`, `crates/`,
  `testdata/`, `bench-baseline/` must resolve.

## Done earlier (this arc, committed)

- P4R closed `c42d711` (quartet + full tier + bless + Category B green;
  allow ratchet 106→101; churn frozen for E-5; bench baseline `4T`).
- P4D D-1 `6f8fd6d` (root README 92/120 + `docs/README.md`), D-2 `f605cbe`
  (14 crate READMEs ≤17 lines).
