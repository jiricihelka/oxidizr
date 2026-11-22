# PHASE_4D.md — detailed plan: documentation reset — short, human-first (interlude)

**Status: SCAFFOLDED 2026-11-22 (user directive) — not started.**
Position: the LAST hardening phase (after P4T + P4R, so prose describes
the final code), immediately before the release tail (§P4-M5 E-4/E-5:
version bump, CHANGELOG, cargo-dist, `v0.4.0`).

> Naming note: this file's decisions are **P4D-Dn**; the Phase 4 feature
> plan (`PHASE_4.md`) uses **P4-Dn**. When citing in prose, cite the file.

## P4D-D0 Charter (user directive, 2026-11-22)

Go through ALL documentation — user, programmer and agent facing,
including code comments, error messages and READMEs — and make it short
and understandable. Concretely:

1. **Human-facing text** (READMEs, rustdoc incl. `//!`/`///`, CLI help:
   `about`/arg docs, diagnostic/error message texts, `docs/*.md`,
   viewer UI strings, CHANGELOG) contains NO development-process
   references (phase/milestone/decision ids — 302 `§P…` refs across 51
   Rust files today) and NO references to the reference Java
   implementation (46 Rust files mention `structurizr`-the-implementation;
   the CLI `--help` alone has 25 "reference" phrasings).
2. **Agent-facing docs** (`agents/notes/`, `AGENTS.md`) shed process
   narration that is not needed to work; `agents/plan/` KEEPS everything —
   it is the process memory and the ONLY home of port provenance
   ("ported from X.java @ pin", pins, V-n items) — user decision
   2026-11-22. `PROGRESS.md`/`RESUME.md` keep their structure (they are
   the journal).
3. Every directory gets a short README saying what lives in it.
4. Every Rust module keeps its `//!` header (all 139 have one — the pass
   rewrites them shorter, it does not create from zero).
5. The ROOT README becomes the user-facing documentation (install,
   quickstart, command table, exit codes, links).

**What is NOT stripped**: the compat promise itself ("workspaces stay
consumable by Structurizr tooling, JSON is the contract" — one honest
line, the product pitch stays), format-level facts (`.dsl` syntax,
`structurizr.*` property names, Jackson `" : "` style output, OXZ codes),
and file paths in help text. Rule of thumb: text may describe WHAT the
tool does and WHAT the user should do; it may not tell the story of WHERE
the behavior came from.

## P4D-D1 Mechanism — the strip rules (one pass, mechanical, per crate)

- Delete `§Pn-…`, `P4-Dn`, `P3-M9`, `(V-n)`, `(W-n)`, "milestone",
  "wave", "interlude", "the reference decides/does X", "ported from",
  monorepo pins, `agents/plan/…` path citations — from doc comments and
  string literals that reach humans (`about`, help, messages, UI strings).
- Where a citation carried REAL information (a trap, an invariant), the
  information is restated as a plain sentence ("property order is
  document order — the writer must not sort"); where it was pure provenance,
  it disappears (provenance already lives in `agents/plan/`).
- Error/diagnostic texts: ≤ 2 sentences, name the failure + the fix
  ("`--bin` is not a file: … (pass a path or unset the flag)"). The
  `docs/diagnostics.md` rows shrink to the same shape.
- Budget rule (anti-over-polish): ONE pass per crate; style-only re-diffs
  of already-clean text are rejected in review.

## P4D-D2 The README program

| Target | Content | Budget |
|---|---|---|
| `/README.md` (root) | what oxidizr is + compat promise; install (dist binaries, cargo build + nightly note); quickstart (validate → dsl2json → export → diff); command list one line each; exit-code family; links to `docs/` | ≤ 120 lines |
| `crates/*/README.md` (ALL 14 crates — none exist today) | purpose, key modules, who consumes it, one pointer into `agents/plan/` if deep context exists | ≤ 20 lines each |
| `docs/README.md` (missing today) | index of the user docs pages | ≤ 10 lines |
| the `xtask` crate row (within the 14 above) may stay a stub pointing at `agents/plan/WORKFLOW.md` | | |
| `testdata/`, `e2e/`, `bench-baseline/`, `agents/` | already have READMEs — brevity review only | — |

## P4D-D3 rustdoc pass + the hygiene gate

- Crate waves use the P4R wave grouping (R1…R4) — freshest code first.
- Keep: what a module does, invariants, error behavior. Remove: everything
  D1 lists. `cargo doc --no-deps` warning-free throughout.
- New gate leg (home: the existing `cargo xtask refs` "doc-reference
  integrity" step): FAIL on (a) a `§P`/phase-id pattern, (b) a
  reference-implementation pattern, in public doc comments and human
  string literals of shipped crates — allowlist for `agents/` +
  `tests/` + intentionally-quoted fixture text. The quartet picks it up
  with zero user workflow change.

## P4D-D4 Message/help surface pass

`--help` (short + long), clap `about`/arg docs, viewer UI strings,
`docs/diagnostics.md`, `docs/{diff,inspect,query}-schema.md`,
`docs/lsp-editors.md`. Every text change lands WITH its golden/text
assertions in the same commit: CLI text assertions update in place; the
e2e bless tripwire is expected to move for the affected human-text
artifacts ONLY — recorded in the row stamp (this is the one phase where
bless moves on purpose; a byte-contract artifact moving is still red).

## P4D-D5 Agent-doc trim (where not needed)

`AGENTS.md`: the charter survives, phase-era narration shrinks (commands,
Always/Never, where-things-are stay). `agents/notes/`: rewrite/remove
stale phase narration, keep env/pitfalls/process facts. `agents/plan/`:
untouched except the post-close index + status stamps. TESTING.md keeps
the P4T matrix (it is agent-facing and needed).

## P4D-D6 Milestones (ordered, each ends green — quartet + commit + RESUME)

| Item | Scope | Commit |
|---|---|---|
| **D-1** | Root README + `docs/README.md` | D1 |
| **D-2** | The 14 crate READMEs (+ brevity review of existing ones) | D2 |
| **D-3** | rustdoc strip: wave-A crates (core/ast/diag/dsl/json) | D3 |
| **D-4** | rustdoc strip: wave-B crates (export/import/api/inspect/layout) | D4 |
| **D-5** | rustdoc strip: wave-C crates (lsp/viewer/cli/xtask/e2e) | D5 |
| **D-6** | help + diagnostics + docs/*.md message pass (bless moves on purpose — stamped) | D6 |
| **D-7** | hygiene leg in `xtask refs` + agent-doc trim | D7 |
| **D-8** | Close ritual (quartet + full tier + bless per D4 rule + PROGRESS `P4D` log) → hand over to the release tail (E-4/E-5) | D8 |

## P4D-D7 Verification list

- P4D-V1 — `cargo xtask refs` current semantics: extend vs new subcommand
  (read `refs.rs` before scoping D-7).
- P4D-V2 — enumerate EVERY golden artifact that embeds human text
  (help snapshots? message assertions?) so the D-6 bless delta is fully
  predicted before the pass runs.
- P4D-V3 — miette renders diagnostics from `Issue.message` — confirm no
  doc-comment is a string-literal source (none expected; verify).

## Risks

- **Provenance loss** — mitigated by charter: plan registers + git
  history keep everything; strip is deletion from human text only.
- **Golden churn beyond human text** — D-6 predicts the bless delta first
  (V2); anything not on the predicted list is a bug, stop and investigate.
- **Over-polish / rewrite temptation** — one pass per crate (D1 budget
  rule); P4R owns code shape, P4D owns words; a wave touching code logic
  belongs in neither phase.
- **Compat honesty drift** — the stripped docs must still tell users the
  JSON contract exists (D0 "not stripped" list); the close review checks
  README + `--help` still state the promise.
