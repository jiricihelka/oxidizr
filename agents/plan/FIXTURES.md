# Reference test fixtures index (future golden corpus)

All paths relative to the pinned monorepo clone (`ad34036`, v6.2.4). These are the raw
material for oxidizr's DSL→JSON golden harness (agents/plan/TESTING.md). License:
Apache-2.0 — keep attribution when vendoring. Note: DSL tests assert in-memory state;
**DSL+JSON pairs** exist only where listed in §B. `spring-petclinic/workspace.json` and
the `backwardsCompatibility/` files are pre-v6 outputs → read-compat corpus, not
write-goldens (they contain `location`, `branding`, `implementation`, …).

## A. `structurizr-dsl/src/test/resources/dsl/` — 84 top-level `.dsl` fixtures

| Feature area | Files |
| --- | --- |
| canonical examples | `getting-started.dsl`, `getting-started-short.dsl`, `test.dsl`, `financial-risk-system.dsl`, `amazon-web-services.dsl`, `amazon-web-services-local.dsl`, `big-bank-plc.dsl` |
| identifiers | `identifiers.dsl`, `hierarchical-identifiers*.dsl` (4), `hierarchical-identifiers-and-deployment-nodes-{1,2,3}.dsl` |
| groups | `groups.dsl`, `groups-nested.dsl`, `group-url.dsl`, `group-without-brace.dsl`, `find-elements-in-flat-group.dsl`, `find-elements-in-nested-group.dsl` |
| relationships | `no-relationship.dsl`, `relationship-already-exists.dsl`, `relationship-without-identifier.dsl`, `this.dsl`, `exclude-relationships.dsl`, `exclude-implied-relationship.dsl`, `include-implied-relationship.dsl` |
| archetypes | `archetypes.dsl`, `archetypes-for-{custom-elements,defaults,extension,implicit-relationships}.dsl`, `archetypes-from-workspace-extension-{parent,child}.dsl` |
| constants/vars | `constant.dsl`, `constants-and-variables-from-workspace-extension-{parent,child}.dsl` |
| deployment | `deployment-environment-empty.dsl`, `deployment-groups-flat.dsl`, `deployment-groups-hierarchical.dsl`, `deployment-view-animation.dsl` |
| dynamic views | `dynamic.dsl`, `dynamic-view-with-{custom-elements,explicit-ordering,explicit-relationships}.dsl`, `parallel1..3.dsl` |
| filtered views | `filteredviews.dsl` |
| animation | `static-view-animation.dsl`, `custom-view-animation.dsl` |
| views misc | `views-without-keys.dsl`, `multiple-view-tokens.dsl`, `color-schemes.dsl`, `shapes.dsl` |
| image views | `image-view.dsl` + `image-views/` (workspace-via-{file,source,url}.dsl, diagram.puml/.mmd/.dot, image.svg/png) |
| include | `include-file.dsl`, `include-directory.dsl`, `include-url.dsl`, `include-implied-relationship.dsl` + `include/` (model.dsl, model/software-system/model.dsl, docs/section.md) |
| workspace extension | `extend/` (extend-workspace-from-{dsl,json}-{file,url}.dsl, workspace.dsl, workspace.json, 1..4.dsl) |
| docs | `docs/workspace.dsl` + `docs/docs/{workspace,softwaresystem,container,component}/*.md` (nested doc tree incl. README.md conventions) |
| ADRs | `decisions/workspace.dsl` + `decisions/{adrtools,madr,log4brains}/…` (real ADR corpora of each format) |
| scripts | `script-{inline,external,external-with-parameters,in-dynamic-view}.dsl` + `test.{js,groovy,kts,rb}`, `test-with-parameters.groovy` |
| plugins | `plugin-with{out,-}-parameters.dsl` + `plugins/structurizr-dsl-plugin-1.0.0.jar` |
| finders | `find-element.dsl`, `find-element-hierarchical.dsl` |
| DSL source retention | `source-parent.dsl`, `source-child.dsl`, `source-not-retained.dsl` |
| text/encoding/errors | `text-block.dsl`, `multi-line.dsl`, `multi-line-with-error.dsl`, `utf8.dsl`, `iso-8859.dsl`, `workspace-with-bom*.dsl`, `unexpected-tokens-{before,after,in}-workspace.dsl`, `multiple-{model,workspace}-tokens.dsl` |
| workspace properties | `workspace-properties.dsl` |
| other | `bulk-operations.dsl`, `enterprise.dsl` (expect error — keyword removed), `custom-view-animation.dsl` |

Nested multi-file workspaces: `big-bank-plc/` (include tree + `.adoc` docs + ADR dir +
styles), `spring-petclinic/` (`workspace.dsl` + `workspace.json` — full real workspace).

## B. DSL↔JSON pairs (usable as write-goldens after regeneration with v6.2.4 tooling)

- `structurizr-export/src/test/resources/amazon-web-services.{dsl,json}` — deployment
  heavy, environments, deploymentGroups arrays, health checks.
- `structurizr-export/src/test/resources/big-bank-plc.json` (DSL lives in dsl module).
- `structurizr-export/src/test/resources/groups.{dsl,json}`.
- `structurizr-dsl/.../spring-petclinic/workspace.json` — read-oracle (v5-era output).

## C. JSON-only corpus (reader compat + validation)

- `structurizr-client/src/test/resources/structurizr-36141-workspace.json` — reference
  workspace used across tests.
- `backwardsCompatibility/`: `structurizr-31-workspace.json`, `structurizr-36141…json`,
  `structurizr-39459…json`, `views-without-order.json` — legacy shapes.
- `workspaceValidation/*.json` (16): unique-name and dangling-reference violations —
  one per validation rule (people/systems/container/component/child-node names, view
  key uniqueness, missing elements/relationships referenced by views, env-scoped
  duplicate node names, filtered-view missing base…). → Validation error catalogue.
- `theme.json`, `structurizr-logo.png` — theme/logo formats.
- `structurizr-dsl/.../themes/theme.json` — theme loading for DSL tests.

## D. Behavior test classes worth mirroring (assertion source of truth)

`structurizr-dsl/src/test/java/com/structurizr/dsl/` — 83 classes, notably
`DslTests` (full-file parses incl. default view keys `SystemContext-001`…),
`StructurizrDslParserTests`, `TokenizerTests`, `TokensTests`, `IdentifierScopeParserTests`,
`IncludeParserTests`, `ArchetypeParserTests`, `DeploymentNodeParserTests`,
`HealthCheckParserTests`, `ExpressionParserTests`/filter tests, `WorkspaceExtensionTests`,
`SourceRetentionPolicyTests`, `*ScriptTests`, `PluginTests`, `FilteredViewParserTests`.
`structurizr-core/src/test/...` — ModelTests, RelationshipTests, ViewSetTests
(createDefaultViews), CanonicalNameGeneratorTests.
`structurizr-client/src/test/.../io/json/JsonTests` + `JsonWriterTests` — byte-level JSON.

## E. Notes / caveats

- ~10 DSL tests + all API tests tagged `IntegrationTest` (Docker/network) — skip.
- No public `structurizr/samples` repo; this is the only fixture corpus.
- Cookbook DSL examples referenced by `CookbookTests` live in docs repo `dsl/cookbook/`
  (in docs clone) — extra fixture source.
- Regenerate goldens with the unified `structurizr` app (v6.2.4 / 2026.10.26) — record
  version in `testdata/README.md` when vendoring.
