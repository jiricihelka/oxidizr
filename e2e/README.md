# e2e — the regression corpus (Phase 1T; export matrix at 2T)

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
    themes/<name>/theme.json  installed theme seen by the reference default
                            source <workspace-parent>/themes (optional)
    workspace.e2e.json      blessed sidecar — regenerate: cargo xtask e2e bless
```

An entry has at least one of `workspace.dsl` / `workspace.json`. DSL-only
entries get `dsl2json` + `fmt` sections; a `workspace.json` is the explicit
`json2json` feed (raw-JSON failure classes like OXZ-4001 enter this way) and
the `validate` target when there is no DSL; without it, `json2json` re-reads
the `dsl2json` output.

The sidecar also carries the **export matrix** (2T, `build/PHASE_2T.md`):
`export.{plantuml,plantuml-c4,mermaid,websequencediagrams}` blocks — exit
class, OXZ codes, files written — recorded whenever the entry's input parses
with ≥ 1 view (DSL first, else JSON). Exports run through `Request::Export`
into per-entry scratch dirs under the entry's own parent, so a `themes/`
dir is picked up by the reference theme chain with no runner hook; the runs
are hermetic (`allow_net` false — the last format is a compat probe that
never writes files).

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
