# AGENTS.md

Per-task charter — only what every agent task needs. Everything else lives in
`agents/`: permanent detail in `agents/plan/`, operational memory in
`agents/notes/`. If this charter and a plan doc disagree, the plan doc wins —
fix this file.

## Project

**oxidizr** — a Rust alternative to [Structurizr](https://structurizr.com) (the
"models-as-code" architecture tool, [C4 model](https://c4model.com)). Core
promise: **full compatibility** with Structurizr DSL (`.dsl`) and JSON (`.json`)
workspaces — consumable by the official tooling and back, byte-comparable at the
JSON level where the format defines it. Status: Phase 1 — milestone log in
`agents/plan/PROGRESS.md`, current scope in `agents/plan/build/PHASE_1.md`.

## Where things are

- `agents/notes/` — short operational notes (env setup, paid-for pitfalls, testing,
  process). **Search here before debugging or re-deriving tool behavior**; index in
  `agents/notes/README.md`. Write a note after every non-obvious fix.
- `agents/plan/` — permanent design corpus, decision registers, phase plans; index
  and reading order in `agents/plan/README.md`. `agents/plan/reports/` = untracked
  scratch for user-dropped CI/OS reports.
- `crates/` — Cargo workspace; `agents/plan/build/PHASE_N.md` is authoritative for
  crate topology.
- `testdata/` — byte-exact vendored reference corpus + committed goldens; its
  README documents the contract. Never hand-edit.

## Always / Never

- **JSON is the byte contract.** Structurizr field names, shape and types beat Rust
  convention, always; never rename, reorder semantics, or "improve" a field.
  Full contract + sources of truth: `agents/plan/COMPAT.md`.
- The **reference implementation decides** ambiguous behavior (pinned monorepo,
  see COMPAT.md) — not memory, not old docs. Fix compat mismatches in oxidizr,
  *never* by editing expected outputs or goldens.
- Before implementing any behavior → check `agents/plan/DEVIATIONS.md` (its Locked
  list never deviates). Before building anything Structurizr lacks → check
  `agents/plan/ADDITIONAL_FEATURES.md` and get sign-off for 💡 ideas.
- No `unsafe`; std-only; `#[cfg(target_…)]` only with a justifying comment; errors
  as values (`thiserror` in libs, `anyhow` in binaries); `unwrap()`/`panic!` only
  in tests. Nightly pin in `rust-toolchain.toml`, usages registered in `NIGHTLY.md`.
  More: `agents/plan/WORKFLOW.md` §Code quality / §Code style.
- Commit only after the gates are green; **never push** unless explicitly asked.

## Commands (quick reference)

```sh
cargo xtask ci              # the gate trio before every commit: fmt + clippy + test
cargo xtask ci windows      # + Wine windows-gnu leg (needs `nix develop`)
cargo xtask bench | cov     # bench / coverage job mirrors
cargo xtask oracle verify   # Category B (needs $OXIDZR_STRUCTURIZR_BIN, slow)
```

Full command table, dev-shell policy, CLI/config rules: `agents/plan/WORKFLOW.md`.
Test categories A/B, golden corpus rules, benchmarks: `agents/plan/TESTING.md`.

## Working style: when to ask

- Planning: ask lots of questions — every decision gets recorded with rationale in
  `agents/plan/`.
- Development: implement per the plan without stopping; break the silence ONLY when
  reality contradicts a plan assumption (a V-n item comes out differently, the
  reference behaves surprisingly, a compat gate fails unanticipatedly).
- Commit every green sub-piece (`cargo xtask ci` green = commit) and rewrite
  `agents/notes/RESUME.md` with it — never hold a day of work uncommitted.
- A milestone touching more than ~4 modules gets decomposed in its
  `agents/plan/build/PHASE_N.md` entry before implementation starts.
- After a non-obvious fix: add a ≤ 25-line note under `agents/notes/` and index it —
  the next session must not have to rediscover it.
