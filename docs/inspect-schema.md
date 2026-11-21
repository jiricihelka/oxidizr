# `inspect` JSON output schema (schema 1)

Machine-readable contract for `oxidizr inspect WORKSPACE --format json`
(§P4-M1, F-inspect-parity). The document is oxidizr's own format — it is
NOT Structurizr JSON and never feeds `oxidizr-json` (the byte contract
is untouched; the inspections are advisory, P4-D7). Range owner:
`crates/oxidizr-api/src/pipeline.rs` (`inspect_cmd`); the violations
themselves are produced by `crates/oxidizr-inspect` (the
`structurizr-inspection` port — type strings and messages verbatim from
the pinned reference).

## Versioning policy

Identical to the diff schema (`docs/diff-schema.md` §Versioning): the
`schema` key carries the version as a NUMBER; document-shape changes
are additive-only within a major oxidizr version; a breaking change
bumps `schema`; consumers ignore unknown keys.

## Document shape

Both top-level keys are ALWAYS present (the empty run reports an empty
array). Key order below is part of the contract (stable bytes).
`violations` is in the ENGINE RUN ORDER — workspace, model (elements in
canonical id order, relationships under their source), views — exactly
the order the human format prints; it is never sorted.

```json
{
  "schema": 1,
  "violations": [
    { "type": "model.element.noview", "severity": "error", "message": "…" }
  ]
}
```

- `type` — `Inspection.getType()`: the dotted inspection type
  (`workspace.scope`, `model.container.technology`,
  `views.view.key`, `views.styles.element.metadata`, …). It is also the
  property-key stem and the `--only`/`--except` filter value.
- `severity` — one of `"error" | "warning" | "info"` (lowercase).
  `"ignore"` NEVER appears: the engine keeps `ignore` violations (the
  reference counts them in `structurizr.inspection.ignore`) and the CLI
  drops them before this document is written.
- `message` — the reference message verbatim (the same string the human
  format prints after the second `|`).

The `--severity-threshold` / `--only` / `--except` filters shape the
array before serialization; run order and dedup semantics are
unaffected (no dedup exists — one entry per inspection hit).

## Exit code

`oxidizr inspect` exits 1 (the `ExitClass::Differences` class, the
documented P4-D1 extension of the P1-D8 contract) iff at least one
violation is REPORTED after the filters; a clean (or fully filtered)
run exits 0.
