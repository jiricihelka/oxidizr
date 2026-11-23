# `diff` JSON output schema (schema 1)

Machine-readable contract for `oxidizr diff A B --format json`. The document
is oxidizr's own format — it is NOT Structurizr JSON and never feeds the
JSON byte contract. Implementation: `crates/oxidizr-api/src/diff.rs`
(`Diff::to_json`).

## Versioning policy

The `schema` key carries the version as a NUMBER. Changes to this
document shape are **additive-only** within a major oxidizr version:
new optional keys, new `field`/`kind` values, new paths. A breaking
change (renamed key, changed type, changed semantics of an existing
key) requires bumping `schema`. Consumers must ignore unknown keys.

## Document shape

All top-level keys are ALWAYS present (empty arrays included); map key
order is the order below and is part of the contract (stable bytes for
a given pair of workspaces). Arrays are sorted deterministically by
(`path`, then `field`/`kind`).

```json
{
  "schema": 1,
  "added":     [{ "path": "…", "kind": "…" }],
  "removed":   [{ "path": "…", "kind": "…" }],
  "changed":   [{ "path": "…", "field": "…", "before": …, "after": … }],
  "idChanged": [{ "path": "…", "before": "…", "after": "…" }]
}
```

| Key | Element fields | Notes |
|---|---|---|
| `added` / `removed` | `path`, `kind` | identities present on exactly one side |
| `changed` | `path`, `field`, `before`, `after` | per-field diffs of matched pairs |
| `idChanged` | `path`, `before`, `after` | matched pairs whose (unstable) id drifted — informational, never an error signal |

An absent field value is JSON `null` in `before`/`after` (the uniform
"absent" convention).

## Paths and identity

Items are keyed by canonical identity, never by id (ids regenerate on
every parse). Paths are dotted, with the canonical-name tail bracketed:

| Path | Identity |
|---|---|
| `model.people[Alice]` | canonical name (`Person://Alice` — the scheme IS the type) |
| `model.softwareSystems[Pay].containers[api].components[db]` | nested model items |
| `model.relationships[Person://A → SoftwareSystem://B (uses)]` | `Relationship://src -> dst (description)` |
| `model.deploymentNodes[Default/docker]…` | deployment chain tail (instances `Name#<n>`) |
| `views[<key>]`, `views.styles.elements[<tag>]`, `views.styles.relationships[<tag>]` | view key / style (kind, tag) |
| `workspace` | workspace metadata (name, description, version, properties) |

Locked-in consequences of identity-by-canonical-name:

- an element NAME change or a relationship DESCRIPTION change IS an
  identity change — a `removed` + `added` pair, never a `changed` entry;
  a person rename therefore also moves every relationship whose
  canonical key embeds the name (paired `removed`/`added`), and view
  membership/geometry fields follow the new name;
- duplicate canonical names (an invalid workspace — validation flags
  it) pair positionally: the Nth occurrence with the Nth;
- a canonical name that cannot resolve (dangling reference) falls back
  to an id-bearing key (`Unresolved://<id>`) — such pairs generally
  will NOT match across a fix.

## Fields compared

Per item: `name`, `description`, `url`, `tags` (the EFFECTIVE list —
kind defaults prepended, deduped, then the stored custom tags; the two
loaders represent the same list differently, so stored-only comparison
would flag every cross-format pair), `group`, `technology`,
`instances`, `deploymentGroups`, plus one `properties.<key>` field per
free-form property. Empty text compares as ABSENT (`""` on one loader,
omitted on the other — the JSON writer omits empty text). Views compare
`kind`, `title`, `description`, scope ids (`softwareSystemId`,
`containerId`, `elementId`, `environment`, `baseViewKey`, …), boundary
flags, `filterMode` / `filterTags`, `paperSize`, and membership
`elements` / `relationships` (sorted canonical-name arrays) — ALWAYS,
membership is not layout.

Geometry fields — `position[<canon>]`, `vertices[<canon>]`,
`automaticLayout`, `dimensions`, `animations` — are omitted entirely
under `--ignore-layout`. `ImageView` content (base64 payload) is never
compared. Deployment `healthCheck` details are not compared (v1 set).

PROVENANCE STAMPS are never compared: any property key starting with
`structurizr.dsl` — the loader regenerates `structurizr.dsl.identifier`
(a fresh UUID per item) and the `structurizr.dsl` source snapshot on
every parse, so comparing them would make `diff(a, a)` non-empty for
every DSL input. User property keys are unaffected.

## Exit code

`oxidizr diff` exits `0` with an empty document, `1` (`differences
found`) when any category is non-empty; `2` usage and `3` I/O
unchanged. `--quiet` suppresses output; the exit code carries the
result.
