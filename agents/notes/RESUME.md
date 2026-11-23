# RESUME — session checkpoint (updated 2026-11-23, **§P4R CLOSED (`c42d711`); §P4D IN FLIGHT: D-1 `6f8fd6d`, D-2 `f605cbe`, D-3 wave-A strip committed, D-4 wave-B strip DONE (commit pending at HEAD+1) — next D-5 (wave-C: lsp/viewer/cli/xtask/e2e + viewer README rewrite), D-6 (message/string-literal pass — docs/*.md pages already rewritten, uncommitted), D-7 (refs hygiene leg + agent-doc trim), D-8 (close) → release tail E-4/E-5 (cargo-dist + `v0.4.0` tag + CHANGELOG, LOCAL)**)

## Active now (2026-11-23)

- **P4D-D-4 DONE (wave-B strip)** — export/import/api/inspect/layout:
  batteries at KEEP-only (residual false positives: our `ReferenceAbort`
  type, Java `TreeSet`/`HashSet`/`LinkedHashSet`/`HashMap` order-contract
  facts, plain-English "referenced"/"probe"/"preference"/"backreferences",
  Base64/U+2028/OXZ codes, pinned `IllegalArgumentException` message text,
  our own probe-corpus citations). Comment-only audit 0 code-side changes;
  quartet green (fixes on the way: `mod@format` doc-link disambiguation,
  `AdrTools` backticks, 24 `\x01` control chars repaired — a cancelled
  wave-B helper wrote chr(1) where a regex `\\1` backreference belonged;
  always double-escape backrefs in rewrite strings).
- **D-5 NEXT (wave-C)** — residual batteries: lsp 33, viewer 199, cli 127,
  xtask 66, e2e 0. Same mechanism: global phrase regex (reuse
  ap_strip.py/ap_strip2.py rule lists) → anchored batch → artifact scan
  (dangling `(`/` :`/`the the` after id removal) → per-crate
  fmt/test-ALL-lines/doc-zero → comment-only audit → ci → commit.
  `viewer` also = README 104→≤45 human-first rewrite; `cli` clap `///`
  help IS user help (rewrite it; update help-assertions same commit);
  string literals in viewer/cli are D-6.
- **Strip scripts reusable**: `/tmp/nix-shell.BoHvza/opencode/{exp,imp,ap}_strip*.py`
  + `{exp,imp,ap}_batch.py` + `waveb_artifacts.py` + `battery_dsl.py`
  (takes file/dir paths); mechanism = approx-line + must-contain fragment +
  marker-normalizing rewrite, `None` deletes, apply descending.
- **D-6 (message pass)** — string literals + docs pages: the five
  `docs/*.md` pages are ALREADY rewritten human-first (uncommitted; ride an
  early D-6 commit). Bless prediction + what may/must not move:
  `agents/notes/process/p4d-d6-bless-prediction.md`. In-test message
  assertions update in the same commit as the message they assert.
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
