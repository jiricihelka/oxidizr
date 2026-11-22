# oxidizr-inspect

Advisory inspections over a workspace: model and view quality checks
(missing description/technology, empty views, elements not in any view,
relationship gaps…) with per-inspection severities.

Pure model logic over `oxidizr-core::Workspace` — it never changes
workspace bytes, validation or parsing. Severity is property-driven:
`structurizr.inspection.<dotted type>` keys on any holder re-rank or
silence an inspection.

The `types` module holds every inspection type string (the filter values
for `oxidizr inspect --only/--except` and the property-key stem).

Consumed by `oxidizr-api` (the `inspect` command).
