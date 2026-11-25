# ERROR-MESSAGES.md — message provenance registry (P5R-V1)

Purpose: P5R loosened the freeze for **oxidizr-coined** diagnostic text
("error messages can improve"). This registry is the guard: a wave may
reword a message ONLY if its family here says `IMPROVABLE`, and even then
the OXZ code, severity, exit class and the firing condition never change
(PHASE_5R.md §P5R-D0). Unclear provenance = FROZEN.

Measured 2026-11-25: 105 `verbatim`-marked sites in `src` (non-test), per
crate: dsl 45, core 11, json 10, api 9, viewer 8, import 7, export 4,
diag 3, inspect 2, lsp/cli/e2e/xtask 1 each.

## Verdict classes

| Class | Meaning | Reword? |
|---|---|---|
| `FROZEN/REFERENCE` | text mirrors the pinned reference implementation (marked VERBATIM in-tree or documented verbatim in the compat corpus) | NEVER |
| `FROZEN/OWN-GOLDEN` | oxidizr-coined, but byte-pinned by a committed golden/CLI test whose diff would touch testdata/ or a Category-B comparison | only with a same-commit, reviewed golden/test update — and never to satisfy a failure |
| `IMPROVABLE` | oxidizr-coined, no byte pin beyond prose assertions | yes: same-commit test + `docs/diagnostics.md` sync |

## Families (measured sites)

1. **Parser diagnostics — `oxidizr-dsl` (45 sites) — `FROZEN/REFERENCE`.**
   `Expected: <grammar production> at line <n> of <path>` — the
   `person <name> [description] [tags]` forms mirror the reference grammar
   strings; CLI goldens and the oracle negative legs compare them.
2. **Inspect report messages — `oxidizr-inspect` — `FROZEN/REFERENCE`.**
   Ported verbatim from `structurizr-inspection` including quirks
   (P4-M1 contract in `build/PHASE_4.md`); the `<Type>Inspection` consts
   are Java class names = provenance.
3. **Ported one-liners — various — `FROZEN/REFERENCE`.**
   e.g. “Two or more workspaces must be provided” (`oxidizr-api/generate`).
   Identify by the in-tree VERBATIM marker.
4. **JSON read/write diagnostics — `oxidizr-json` (10) — `FROZEN/REFERENCE`
   by default.** Jackson-mirrored text where marked; per-site check allowed:
   a marked site is FROZEN, an unmarked site falls to class 2/3 rules.
5. **Diagnostic plumbing — `oxidizr-diag` (3) — mixed.** The “too many
   errors” cap note is pinned by CLI goldens → `FROZEN/OWN-GOLDEN`; codes
   and severities are the catalog (`docs/diagnostics.md`) — NEVER churn.
6. **CLI surface notes — `oxidizr-cli`** (`cannot read \`…\` (no such file
   or directory)`, `note:` lines, the `view`/`oracle` texts, deferred-import
   "is not supported yet") — `FROZEN/OWN-GOLDEN` (T-6 exact-stderr note
   goldens; D-6-era prose contract). Reword only with same-commit golden
   update + review; keep the `note:` shape.
7. **Core/DSL model validation beyond parser arms — `oxidizr-core` (11) —
   per-site:** VERBATIM-marked = class 1; the rest = `IMPROVABLE`.
8. **Export/theme errors — `oxidizr-export` (4) + themes** — `IMPROVABLE`
   (Rust Error Display text; the exported BYTES are the contract, the
   error prose is not; where a CLI golden pins it, class 2 applies).
9. **Importer errors — `oxidizr-import` (7)** — VERBATIM-marked reference
   mirrors = class 1; the rest `IMPROVABLE`.
10. **Viewer/layout/lsp internal errors — `SceneError`/`SaveError`/
    `ExportError` (viewer)/`LayoutError`/lsp** — `IMPROVABLE` (surfaced in
    UI status lines/window titles, not in stdout contracts).
11. **Oracle/dev-tooling text — `cli/src/oracle.rs`, `xtask`** —
    `IMPROVABLE` (dev surfaces; xtask not product at all).

## Per-site procedure (every wave that rewords)

1. Read the surrounding comment: a `VERBATIM`/reference citation ⇒
   `FROZEN/REFERENCE`, stop.
2. `grep` the exact text in `crates/*/tests`, `testdata/`, e2e goldens:
   pinned ⇒ `FROZEN/OWN-GOLDEN` (update in same commit only if the text is
   ours and the update is reviewed as part of the improvement).
3. Otherwise `IMPROVABLE`: reword + same commit update any prose assertions
   + `docs/diagnostics.md` row if the message is catalog-listed +
   CHANGELOG line ONLY if a Rust API changed (prose alone is not API churn).
