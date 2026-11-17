# e2e — the Phase 1T regression corpus

This tree is OURS (authored, blessable, reviewed like code). It is
deliberately **not** `testdata/`: that tree is the vendored byte-contract
against the reference (never hand-edited); the manifests here assert
*use-cases* (exit class + OXZ code set + structural counts), never bytes and
never message text (`agents/plan/build/PHASE_1T.md`).

Layout:

```
corpus/<kind>/<nn-feature>-<name>/
    workspace.dsl           DSL entry (children: !include / extends files)
    workspace.json          JSON entry / explicit json2json feed (optional)
    workspace.e2e.json      blessed sidecar — regenerate: cargo xtask e2e bless
```

An entry has at least one of `workspace.dsl` / `workspace.json`. DSL-only
entries get `dsl2json` + `fmt` sections; a `workspace.json` is the explicit
`json2json` feed (raw-JSON failure classes like OXZ-4001 enter this way) and
the `validate` target when there is no DSL; without it, `json2json` re-reads
the `dsl2json` output.

Kinds: `valid` (realistic workspaces), `invalid` (one failure class per
entry), `mutated` (generated — T3). The same corpus feeds the benches (T5).

Rules:

- Sidecars are generated, never hand-edited; their git diff is the
  behavior-change review record.
- Volatile fields (`structurizr.dsl.identifier` UUIDs, `lastModified*`)
  never enter the assertions.
- Every code in a manifest must exist in the OXZ registry
  (`crates/oxidizr-e2e/tests/coverage.rs`).
- Public (non-original) workspaces may only enter with a license +
  attribution row in `PROVENANCE.md` (Apache-2.0/MIT/BSD-compatible, else
  refused).
