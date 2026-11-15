# COMPAT.md — the compatibility contract (full text)

AGENTS.md §Always/Never enforces this contract in short form; this file is the
detail. When behavior is ambiguous, the official tooling defines it — do not guess.

## Source of truth

- Distilled quick reference: everything in `agents/plan/` (summarized from the sources
  below; index in `README.md`, pinned versions + local-clone instructions there).
- Docs: [docs.structurizr.com/dsl](https://docs.structurizr.com/dsl) (language guide,
  keywords, semantics — `dsl/71-language.md` in the docs repo is the authoritative
  grammar page), [docs.structurizr.com/workspaces/file-types](https://docs.structurizr.com/workspaces/file-types),
  [docs.structurizr.com/commands](https://docs.structurizr.com/commands). Docs source:
  [github.com/structurizr/structurizr.github.io](https://github.com/structurizr/structurizr.github.io).
- Reference implementation (Java monorepo, Apache-2.0, v6+):
  [github.com/structurizr/structurizr](https://github.com/structurizr/structurizr) —
  study `structurizr-core` (domain model), `structurizr-client` (JSON reader/writer —
  the actual serialization rules), `structurizr-dsl` (parser), `structurizr-export`.
  The `structurizr/java`, `/cli`, `/lite`, `/ui` repos are archived stubs from the
  pre-v6 era; use the monorepo.
- JSON schema: `structurizr-json/structurizr.yaml` in the monorepo (OpenAPI 3.0) — note
  it **lags the Java code**; the code is the truth.
- DSL/JSON test fixtures: `structurizr-dsl/src/test/resources` (107 `.dsl` fixtures) and
  `structurizr-client/src/test/resources` (back-compat workspaces). There is no public
  `structurizr/samples` repo.

Some artifacts on Maven Central are older GPL-licensed versions — when studying or
vendoring, check the license of the specific version you touch.

## Hard requirements

- **JSON is the contract.** Field names, structure, and types must match the Structurizr
  JSON format exactly (camelCase keys as defined by the schema, not by Rust convention).
  Never rename, reorder semantics, or "improve" fields.
- Reproduce reference behaviors, e.g.: element and relationship IDs drawn from ONE
  sequential integer counter in definition order, serialized as strings in JSON
  (`workspace.id` is the only numeric ID); implicit tag stacks (`Element`,
  `Relationship`, `Person`, `Software System`, `Container`, ...) — details in
  `FEATURES.md`; DSL source retention rules (`structurizr.dsl` property);
  verify each of these against the reference implementation, not against memory.
  Note the v6 model REMOVED things still seen in older docs/schema (`location`,
  `enterprise`, `branding`, `env()`, `uses`, `deploy {}` scope) — support them only as
  tolerant errors/reads, not as features.
- DSL support must cover the full grammar: `workspace`/`model`/`views`/`configuration`
  scopes, scoped definitions (people, systems, containers, components, groups),
  relationship statements (incl. `this`, implicit source, archetype arrows `--x->`,
  `-/->` removal, implied relationships), archetypes, `deploymentEnvironment` scopes
  with instances/health checks, `!include`, `!const`/`!var` with `${VAR}` substitution
  (env-var fallback), `workspace extends`, `!docs`/`!adrs`, `!elements`/`!relationships`
  finders, `scope shared`, `!identifiers hierarchical`, styles, themes, filtered views,
  dynamic views (incl. `parallel`), custom/image views, auto-layout, text blocks,
  comments (`//`, `#`, `/* */`) and line continuations.
- Parser errors must carry file, line and the offending source line (the reference
  emits no column; adding one is fine, omitting line info is not).
- Scheduling note (2026-11-14): the `!docs`/`!adrs` **importers** are sequenced into
  Phase 4 (`DEVIATIONS.md` D7, `build/PHASE_4.md` P4-D13); until they land, the
  directives parse but load with an explicit deferred-import error — this is the
  ONLY grammar item not delivered by Phase 1. Everything above remains required.
- The reference's *expected behavior* is never wrong: if oxidizr disagrees, oxidizr
  changes. If the reference itself looks broken, capture the divergence in an
  `agents/plan/` doc BEFORE deviating, and check `DEVIATIONS.md` §Locked.
