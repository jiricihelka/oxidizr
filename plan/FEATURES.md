# Structurizr — feature inventory (quick reference)

Source: monorepo @ `ad34036` (library **v6.2.4**, 2026-11-07) + docs clone. Details and
class locations: `STRUCTURIZR.md`. `(verified)` = checked against source directly.

## 1. What Structurizr is

- "Models as code" for the C4 model: one workspace → many diagrams. Two file types:
  **DSL** (`.dsl`, human-written) → **JSON** (`.json`, exchange format for all tooling).
- Since Feb 2026 ("Structurizr vNext") one monorepo/binary replaces the old java lib,
  CLI, Lite, on-prem server and standalone MCP server. Library = Apache-2.0, Java 21.

## 2. Domain model (what DSL/JSON describe)

- `Workspace` = `model` + `views` + `documentation` + top-level `properties` +
  id (long) / name / description / version (string) / lastModified{Date,User,Agent} /
  thumbnail. Base64 DSL is embedded in **workspace top-level
  `properties["structurizr.dsl"]`** (verified in golden) when portable (§5 portability).
- Elements (C4 static): `Person`, `SoftwareSystem` → `Container` → `Component`; plus
  custom `element` (tags `[Element]` only). Hierarchy via parent; lookup by id or
  canonical name: `Person[X]`, `SoftwareSystem[S]:Container[C]:Component[K]`,
  `env:DeploymentNode[A]:DeploymentNode[B]`, instances `...:ContainerInstance[s:c]`
  (`CanonicalNameGenerator`).
- Deployment: `DeploymentEnvironment` (UUID id, ordered; DSL keyword
  `deploymentEnvironment` — the old `deploy {}` scope is GONE (verified: no token)) →
  tree of `DeploymentNode` (instances count, health checks, platform, usage) →
  `InfrastructureNode`, `SoftwareSystemInstance`, `ContainerInstance`. Instances link
  the static element via JSON key **`instanceId` (int)** (verified
  `StaticStructureElementInstance`), plus `numberOfInstances`, `dependencies`
  (container-instance ids), `deploymentGroup`(s), `healthChecks`.
- `HttpHealthCheck` (verified): `name`, `url`, `headers` map, `interval` (s, default 60
  per DSL docs), `timeout` (ms, default 0).
- `Relationship`: id, description, technology, tags, url, properties, perspectives,
  `linkedRelationshipId` (set on implied/instance-replicated rels), interactionStyle
  (`Synchronous`/`Asynchronous` — **DSL cannot set it; plain rels get tag
  `Relationship` only** (verified + golden)). Source/destination type matrix enforced
  (e.g. node→node only; instance→infrastructure only).
- IDs: ONE sequential integer counter shared by elements AND relationships in model
  order, starting at 1 (`SequentialIntegerIdGeneratorStrategy`) (verified). JSON ids are
  **strings** everywhere except `workspace.id` (number).
- Groups: one `group` string on people/systems/container/components/nodes
  (`GroupableItem`); `group "Name"` statement or `group "Name" { }` scope.
- Perspectives: `perspective "name" [description] [value] [url]` on elements/rels
  (v6.1+), serialized as `perspectives` array.
- **Removed in v6 — do NOT implement despite older docs/schema: element/person
  `location`** (dead `Location.java` enum remains, unused (verified)), `enterprise`,
  `branding`, `user`/`users` in JSON output.

## 3. Implicit tags (exact, `model/Tags`) (verified)

| Kind                     | Default tags (order: defaults first, then custom)                  |
| ------------------------ | ------------------------------------------------------------------- |
| Person                   | `Element,Person`                                                     |
| SoftwareSystem           | `Element,Software System`                                            |
| Container                | `Element,Container`                                                  |
| Component                | `Element,Component`                                                  |
| custom element           | `Element`                                                            |
| DeploymentNode           | `Element,Deployment Node`                                            |
| InfrastructureNode       | `Element,Infrastructure Node`                                        |
| SoftwareSystemInstance   | parent system's tags + `Software System Instance` (NO `Element`)     |
| ContainerInstance        | parent container's tags + `Container Instance` (NO `Element`)        |
| Relationship (normal)    | `Relationship` only                                                  |
| Implied/linked rel       | **no tags at all** + `linkedRelationshipId`                          |

Tags serialize as ONE comma-separated string, default tags first.

## 4. Views (`view/`)

- Types ↔ DSL keyword ↔ JSON key: SystemLandscape↔`systemLandscape`↔`systemLandscapeViews`;
  SystemContext↔`systemContext`↔`systemContextViews` (read alias
  `enterpriseContextViews`); Container↔`container`; Component↔`component`;
  Dynamic↔`dynamic`; Deployment↔`deployment` (scoped by environment); Filtered↔`filtered`
  (`baseViewKey`,`view`,`mode` Include|Exclude,`filter` expr); Custom↔`custom`; image
  views↔`image { plantuml|mermaid|kroki|image }` → CustomView. `customViews` exists in
  Java, not yet in OpenAPI schema. Filtered view serializes `{baseViewKey, mode,
  tags[] }` — the filter terms are an ARRAY under `tags` (verified source; legacy
  `filter` string only as read-tolerance).
- Every view: `key` (auto-generated `Prefix-001`…), title, description, autoLayout,
  background, dimensions, animation steps. v6 REMOVED the legacy
  included/excluded-element/relationship lists — the per-view `elements[]`/
  `relationships[]` arrays are both the inclusion list and the geometry:
  `ElementView` {id[,x,y,width,height,z,orientation — mostly only id,x,y serialized]},
  `RelationshipView` {id[,x,y,points[]] (+ Java-only url, jump, terminalLocations)}.
- `AutomaticLayout` (verified fields): `rankDirection` (TopBottom|BottomTop|LeftRight|
  RightLeft, default **LeftRight**), `rankSeparation` 100, `nodeSeparation` 50,
  `edgeSeparation` 50, `vertices` true. Presence = enabled. No `distance`/`seed`.
- Dynamic views: ordered relationship entries `{id, order, description, technology?}`;
  parallel sequences (`parallel` keyword); animation steps per view.
- Default views (landscape + context per system + container per system, keys
  `SystemLandscape-001`…) are generated by the **application** layer when a model has no
  views (`ViewSet.createDefaultViews`), NOT by the DSL/JSON libraries.
- Filter expressions (used by filtered views & finders): `tag`, `type`, `name`, `id`,
  `group`, `technology` matches, `element.name==`, `->container->` forms, and/or/not.

## 5. DSL language (structurizr-dsl + docs `dsl/71-language.md`)

- Hand-rolled tokenizer (no ANTLR). Indentation-scoped contexts; one parser per keyword;
  per-context permitted-token sets (else "is not valid here").
- Top level: optional `workspace "name" "desc" { ... }` or bare statements; scopes
  `model {}`, `views {}`, `configuration {}`; `scope shared {}` relaxes one-parent rule.
- Identifiers by assignment: `x = person "Name" "Desc"`; `!identifiers flat|hierarchical`
  (hierarchical = dotted `sys.container.comp`). Identifier chars: word char then
  `[a-zA-Z0-9_-]*`; duplicates are errors.
- Positional forms (order matters!):
  `person <name> [description] [tags]` (**no technology**, verified docs),
  `softwareSystem <name> [desc] [tags]`, `container/component <name> [desc] [technology] [tags]`,
  `element <name> [metadata] [desc] [tags]`,
  `deploymentNode <name> [desc] [technology] [tags] [instances]`,
  `infrastructureNode <name> [desc] [technology] [tags]`,
  `softwareSystemInstance|containerInstance|instanceOf <id> [deploymentGroups] [tags]`,
  `healthCheck <name> <url> [interval] [timeout]`.
  Children blocks: name/description/technology/metadata(=alias for tags)/tags/url/
  properties/perspectives/group + nested kinds + relationship statements.
- Relationships: `[id = ]a -> b [description] [technology] [tags] { ... }`; implicit
  source inside element scope (`-> b`, `this`); archetypes as arrows `a --https-> b`;
  `-/-> b` removes a replicated instance relationship (deployment scope).
- Implied relationships: on by default; `!impliedRelationships <true|false|fqcn>`;
  strategy `CreateImpliedRelationshipsUnlessAnyRelationshipExistsStrategy`; created rels
  have `linkedRelationshipId`, no tags.
- Archetypes: `archetypes { }` in model; `springBootApplication = application { tech "…";
  tag "…" … }` (bases person|softwareSystem|container|component, chainable); applied as
  keyword: `app = springBootApplication "…"`; relationship archetypes: `sync = -> {...}`,
  `https = --sync-> {...}`, used as arrow.
- Variables: `!const "NAME" "value"` (immutable), `!var` (redefinable); `${NAME}`
  substitution pass over all strings, **fallback to environment variables**
  (`structurizr.feature.dsl.environment`). No `env()`/`$()` function (legacy, gone
  (verified: no token)).
- Includes/extension: `!include <file|dir|url>` (nested; DSL fragments or workspaces;
  JSON ok); `workspace "…" extends <dsl|json>` (overlay merge);
  `!docs <dir> [exclude …]` → markdown docs tree; `!adrs`/`!decisions <dir> [exclude …]`
  → ADR import (formats: structurizr, adr-tools, MADR, log4brains, auto-detected).
- Finders: `!element "Name"` / `!elements <filter>` / `!relationship(s) …` used as
  relationship endpoints & in view include/exclude.
- Advanced: `!components <sys> <container> { classes <source> strategy { … } }` (Java
  component discovery — Java-specific, low priority); `!plugin <fqcn>` (SPI jar);
  `!script` (JSR-223: JS/Groovy/Kotlin/Ruby, exposes parser+workspace+parameters).
- Deprecated but tokenized (error/handled): `!ref`, `!extend`; `!constant` removed →
  error msg "use !const or !var"; `enterprise` token exists but errors (verified).
- Text: `//` and `#` line comments, `/* */` blocks, `\` line continuation, `"""…"""`
  text blocks, UTF-8 (+ BOM) files, ISO-8859 fixture proves encoding handling.
- Styles: `views { styles { element "Tag" {…} relationship "Tag" {…} } themes { url … } }`;
  `theme "<name>"` maps to `https://static.structurizr.com/themes/<name>/theme.json`;
  installed themes via `STRUCTURIZR_THEMES` dir; styles match by tag.
- Portability: if clean, base64 source stored in workspace `properties["structurizr.dsl"]`;
  every element gets `properties["structurizr.dsl.identifier"]` (relationships only if
  named). Non-portable (property omitted): `workspace extends`, `!include`, `!script`,
  `!plugin`, `!docs`, `!adrs/!decisions`, `!components`, `!impliedRelationships <fqcn>`,
  file-based icons/themes/logos. `structurizr.dsl.source=false` forces discard.
- Errors: `<message> at line N of <abs-path>: <trimmed source line>` — file+line+source,
  NO column; parse aborts on first error (verified notes + `StructurizrDslParserException`).

## 6. JSON format (the contract)

- Schema: `structurizr-json/structurizr.yaml` — OpenAPI 3.0, version 6.1.0, **lags the
  Java code**; Java serialization is the truth for v6.2.4.
- Writer rules (Jackson, in `structurizr-client`): keys **alphabetically sorted**,
  NON_NULL + NON_EMPTY omission, dates `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC, 2-space pretty
  print; reader ignores unknown fields.
- Key spellings: JSON style key is **`color`** for element AND relationship styles
  (verified: zero `"colour"` in any repo JSON); the DSL accepts both `colour` and
  `color` tokens. Tags = comma string.
- Schema-vs-code drift — in schema but removed from code: `enterprise`, `branding`,
  `location`, `user`. In code but not schema: `customViews`, RelationshipView `url`/
  `jump`, `terminology.infrastructureNode`, ElementStyle `iconPosition`, instances
  `deploymentGroups`.
- Properties exist at 4 levels: workspace top-level, model, configuration, and
  element/relationship/view.
- Terminology overrides: softwareSystem/container/component/relationship(+infra).
- Back-compat: readers must tolerate legacy shapes (`enterpriseContextViews`, views
  without `order`, legacy `applied`/`implementation` in autoLayout, old `user` block…).

## 7. Beyond DSL/JSON (later scope, know they exist)

- Export formats: json, theme, plantuml (structurizr + c4plantuml, light/dark), mermaid,
  websequencediagrams, DOT, ilograph, static HTML site, png/svg (Playwright), custom by
  FQCN; legend diagrams; naming `structurizr-<id>-<key>.<ext>`.
- Import: decisions (adr-tools/MADR/log4brains), diagrams (C4-PlantUML, Mermaid, image,
  kroki), markdown docs.
- Autolayout: `dot` binary (module kept; server-side layout now JS dagre).
- Inspection: ~30 advisory checks (validate/inspect commands) — not part of JSON contract.
- Client API: push/pull/branches vs Structurizr service, auth = `X-Authorization` header
  (v6; HMAC gone). Encrypted workspaces (AES).
- Application commands: playground local server mcp push pull lock unlock branches create
  delete regenerate-apikey export generate merge validate inspect list version.
- MCP tools: validate/parse/inspect/export-plantuml/export-mermaid + server CRUD.

## 8. Compat traps (must reproduce)

1. One shared id counter (elements+relationships), string ids in JSON.
2. Exact implicit tag stacks & order (§3); implied rels tagless.
3. JSON keys always `color` (DSL accepts `colour` too); comma-string tags; alphabetical
   JSON keys; NON_EMPTY omits empty arrays/objects/strings; UTC-Z date format;
   2-space indent.
4. DSL portability rules for `structurizr.dsl` + `structurizr.dsl.identifier` properties.
5. `${VAR}` with env fallback happens as a preprocessing pass (affects error line
   reporting and portability).
6. `person` has NO technology arg; positional arg orders differ per keyword (§5).
7. Default-view generation & inspections are APP-layer, not parser/library behavior.
8. Instance relationships are separate relationship objects with `linkedRelationshipId`;
   `-/->` removes them.
9. Read tolerance for legacy fields; write only current fields.
10. Style/metadata quirks: `metadata` keyword = alias for tags; autoLayout defaults in
    §4; theme URLs relative-icon resolution against theme base.

## 9. Do NOT implement (removed/deprecated; legacy only worth read-tolerance)

`env()` `$()` · `uses` keyword · `deploy {}` scope · `enterprise {}` · `branding` ·
`location` on elements · `user`/`users` JSON · `!constant` · `!ref`/`!extend`
(prefer clean error) · HMAC API auth · `theme default`.
