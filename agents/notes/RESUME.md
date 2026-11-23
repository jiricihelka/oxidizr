# RESUME — session checkpoint (updated 2026-11-23, **§P4R CLOSED (`c42d711`); §P4D IN FLIGHT: D-1 `6f8fd6d`, D-2 `f605cbe`, D-3 `bce0ed9`, D-4 `a33fb16`, D-5 `8a3430f`, D-6 `ec3e16f`+`a6caf09`, D-7 committed (HEAD) — next D-8 (close ritual → hand to release tail), E-4 (cargo-dist + release.yml), E-5 (0.4.0 bump + CHANGELOG + LOCAL `v0.4.0` tag, NEVER push)**)

## Active now (2026-11-23)

- **P4D-D-7 DONE (hygiene leg + agent-doc trim)** — `cargo xtask refs` now
  carries a third leg (`check_hygiene`): FAILs on process ids (`§P`,
  `P4-D2`-family, `M<n>`, `[ONV]-<n>`, `T<n>-x`, `D10/D11`, `Phase <n>`,
  `2R R-`, `R-<area>-<n>`, bare `F1..F19` in COMMENTS only — string-literal
  `F1` is fixture data) and reference-narration (`reference implementation`,
  `Structurizr-Monolith`, `.java`, `the reference` with word boundary,
  `ported from`) in comments + string literals of `crates/oxidizr-*/src`,
  `oxidizr-e2e` excluded, `refs:ignore` honored. Self-gating test
  `shipped_tree_is_hygiene_clean`; 4 unit tests; the quartet picks it up
  with zero workflow change (V1 = extend, settled). V3 closed: `include_str!`
  embeds init TEMPLATE files only — no doc comment is a string source.
  Residual strip rode along (~15 files: bare `M<n>`/`V-8`/`F12`/`T0-c`/`N-2`
  ids, `ElementStyle.java` citation, two `ported from`, the two JVM-codec
  messages minus `D10` + their assertion, the viewer session-geometry UI
  string, stmt_views module-doc grammar fix). Agent-doc trim: AGENTS.md
  status line made durable + refs-leg blurb; t3 note pointer re-pathed.
- **D-8 NEXT (close ritual)** — quartet + FULL e2e tier + bless (307×
  unchanged expected) + Category B (`TZ=UTC cargo xtask oracle verify` +
  export-verify ×3, `$OXIDZR_STRUCTURIZR_BIN=/
  nix/store/5yil7chk3cjg3qmsabq1vzdxv0v4bwm0-structurizr-2026.10.26/bin/
  structurizr`; slow — background chain) + PROGRESS `P4D` milestone log +
  PHASE_4D D-table stamps (D1..D8 + V1/V2/V3 results).
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
