# oxidizr-core

The domain model: workspace, people, software systems, containers,
components, relationships, deployment nodes and instances, views, styles,
configuration — plus the validation catalog and workspace mutation
operations (`viewops`).

Dependency-free by design: diagnostics travel out as plain values. Model
instances are immutable once built (`WorkspaceBuilder::build`), and field
order mirrors the JSON contract.

Key modules: `model`, `views`, `styles`, `builder`, `viewops`, `validate`,
`canonical` (the comparison view `diff` uses).

Consumed by every other crate.
