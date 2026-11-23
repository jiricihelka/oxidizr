# RESUME — session checkpoint (updated 2026-11-23, **§P4R CLOSED (`c42d711`); §P4D CLOSED (D-1 `6f8fd6d` … D-7 `cc5525b`, D-8 close commit at HEAD) — next: release tail E-4 (cargo-dist via `nix develop` + `release.yml`) then E-5 (0.4.0 bump + CHANGELOG + LOCAL `v0.4.0` tag, NEVER push/publish); after that the P4 phase arc is DONE** )

## Active now (2026-11-23)

- **§P4D CLOSED at D-8** — close evidence: quartet green, full tier 8/8,
  bless 307×unchanged every close (never moved — V2 held), Category B green
  (`oracle verify` 4 json + 4 sidecar + 99 diagram + 6 theme;
  `export-verify` 31/31 × 3 `TZ=UTC`; landscape skipped with reason —
  no oracle-path diff). PROGRESS row + `P4D` log + PHASE_4D stamps (D-table,
  V1/V2/V3) all committed with D-8. The hygiene leg now guards refs forever
  (`check_hygiene`, self-gating test).
- **E-4 NEXT (cargo-dist)** — `nix develop` (cargo-dist 0.34.0); add
  `[workspace.metadata.dist]` config + `dist` via `cargo dist init`-style
  files BY HAND (plan first: WORKFLOW.md release rules + PHASE_4.md §P4-M5
  E-4 row); `cargo dist check` green; add `.github/workflows/release.yml`
  mirroring what `cargo dist generate` emits (NO push/publish).
- **E-5 (version tail)** — bump every crate + path-dep to 0.4.0; write
  `CHANGELOG.md` folding the 3 frozen churn lines
  (`agents/notes/process/api-churn-v0.4.0.md`); LOCAL tag `v0.4.0` only.
  Gates before the bump commit: full quartet; bless 307×unchanged after.
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
- Closes: quartet + full tier + bless 307×unchanged + Category B where
  the diff touches generator/parser code.
- refs gate: doc-comment path tokens anchored at `agents/`, `crates/`,
  `testdata/`, `bench-baseline/` must resolve.

## Done earlier (this arc, committed)

- P4R closed `c42d711` (quartet + full tier + bless + Category B green;
  allow ratchet 106→101; churn frozen for E-5; bench baseline `4T`).
- P4D D-1 `6f8fd6d` (root README 92/120 + `docs/README.md`), D-2 `f605cbe`
  (14 crate READMEs ≤17 lines).
