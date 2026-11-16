# oracle-app-harness — `structurizr export` adds things the library never does

`cargo xtask oracle gen|verify` drives the structurizr CLI app
(2026.10.26), not the pinned library. The app's load step injects TWO
artifacts into the export that the library's reader/writer/parser never
produce — goldens are defined as bytes MINUS these (discovered while
goldening the M5 extends fixtures, 2026-11-16):

1. **`structurizr.inspection.*` properties** (`error/warning/info/ignore`)
   on every loaded workspace — handled since M0 by
   `normalize_reference_output`. Pitfall found at M5: when they were the
   ONLY members of workspace-level `properties` (e.g. after
   `workspace extends` cleared the retained DSL), deleting the four lines
   left an empty `"properties" : {\n  }` shell — the NON_EMPTY library
   writer can never emit an empty properties object, so normalize now drops
   the emptied braces too (pass 2, comma fix-up included).
2. **Default views**: if the loaded workspace has ZERO views (and a
   non-empty model), the app generates landscape + per-system-context (+
   per-container) views — keys `SystemLandscape-001`…, `generatedKey: true`,
   names `System Landscape View`/`System Context View: X`. This is the
   V-10 pipeline (`AbstractCommand.loadWorkspace` →
   `createDefaultViews()`; recorded 2026-11-14 in PROGRESS.md errata) — the
   DSL parser does NOT do this (`DslTests.test_extendWorkspaceFromDslFiles`
   sees exactly its one view), so these bytes are app, not contract. They
   are **indistinguishable** from legitimate DSL auto-keyed views
   (`systemLandscape { }` yields the identical shape) → NOT normalizable
   away.

Consequence for goldening: only fixtures whose workspaces already carry a
views block (and identifier-stamped elements, else random
`structurizr.dsl.identifier` UUIDs) are byte-goldenable. Viewless fixtures
(`source-child.dsl`, `extend-workspace-from-json-file.dsl`) stay
Category-A-covered only. `extend/4.dsl` is the extends witness.

Also: `xtask oracle verify` used to abort on `m3b-defaults` — `find_fixture`
scanned only `.dsl`; it now resolves JSON fixtures in `probes/` +
`read-corpus/` too (the verify path had been broken for that golden since
M3 — it is not part of the default gate).

Probe recipe: `structurizr export --workspace X.dsl --format json --output D`
by hand, compare against `testdata/golden/`.

Last useful: 2026-11-16
