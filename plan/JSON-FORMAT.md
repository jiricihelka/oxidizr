# Structurizr JSON format (field-level quick reference)

Truth = Jackson serialization of core objects by `structurizr-client` (`io/json/JsonReader`/
`JsonWriter`); OpenAPI schema `structurizr-json/structurizr.yaml` (info.version 6.1.0)
**lags the code** — drift marked below. Verified against goldens @ v6.2.4 `ad34036`.
Field names below are JSON keys (camelCase). `(verified)` = checked in source/goldens.

> **2026-11-14 errata — `plan/build/WRITE-CATALOG.md` is now authoritative** for every
> byte-level write rule (P1-M3a). Corrections to the notes below, all probe-verified:
> - Inclusion is `NON_EMPTY` only (`NON_NULL` is dead — `setSerializationInclusion` is
>   called twice, the last wins). **Primitive `int`/`boolean` and their boxed `Integer 0`
>   /`Boolean false` with `NON_NULL` are NEVER "empty", so always written**: `id:0`,
>   `x:0`/`y:0`, `instanceId`, `enterpriseBoundaryVisible:true` (default),
>   `externalBoundariesVisible:false`, `response:false`.
> - Non-null POJO beans always serialize: `configuration{}`, `documentation{}` (also on
>   every Documentable element — SoftwareSystem/Container/Component), `views.configuration`
>   `{ styles{}, terminology{} }`.
> - View `name` is ALWAYS written and COMPUTED (exact per-kind strings in WRITE-CATALOG
>   §3.1); `@JsonIgnore` on ANY accessor suppresses the whole Jackson property.
> - Deployment instances serialize with NO `name`/`description` (reader derives from the
>   linked element); `DynamicView.deploymentEnvironment` is invisible both directions.
> - `properties` map ordering is decided: **replicate Java `HashMap` bucket order**
>   (WRITE-CATALOG §1), not comparator-canonicalize.

## 1. Writer/reader rules (byte-level contract)

- Writer: POJO keys in Jackson field order (happens-alphabetical at every level — the
  Workspace/ViewSet POJOs are declared alphabetically); **free-form `properties` maps
  are NOT sorted** — Java `HashMap` iteration (hash-bucket) order, proven from reference
  output 2026-11-14 (PHASE_1 V-11; replicate or comparator-canonicalize at M3).
  NON_NULL **and NON_EMPTY** inclusion
  (empty strings/arrays/objects/maps omitted); pretty print, 2-space indent, `\n`;
  empty object `{ }` / empty array `[ ]` (space inside); date fields
  `yyyy-MM-dd'T'HH:mm:ss'Z'` (UTC). `lastModifiedDate`/`Agent`/`User` round-trip when
  present in input; nothing is stamped on write (export is deterministic).
- Reader: unknown properties ignored (`FAIL_ON_UNKNOWN_PROPERTIES=false`) → read
  tolerance for legacy keys is free; but legacy *shapes* need explicit handling:
  `enterpriseContextViews` alias (`@JsonSetter` on ViewSet), views without `order`,
  autoLayout `applied`/`implementation`, element `location`, `branding`, `user(s)`…
- ids: **strings** everywhere except `workspace.id` (number). One shared counter
  (elements+relationships) in definition order starting at 1.
- tags: one comma-separated string, default tags first, custom appended. No arrays of
  tags anywhere (except FilteredView `tags` — see §5).

## 2. Workspace top level

`{ id?, name?, description?, version?, lastModifiedDate?, lastModifiedUser?,
lastModifiedAgent?, thumbnail?, properties?, model, views, documentation, configuration }`
(verified petclinic golden: empty `configuration {}` / `documentation {}` serialize as `{}`).
`properties["structurizr.dsl"]` = base64 DSL when portable; `version` arbitrary string.

## 3. Model

`{ people[], softwareSystems[], deploymentNodes[], customElements?, properties? }`
- **No model-level `relationships` array** — each element nests `relationships[]`
  (verified goldens). Reader must dedupe (a rel appears under its source only; implied
  ones too).
- **No `deploymentEnvironments` container** — nodes carry `environment` (string) on every
  DeploymentNode/InfrastructureNode/instance; top-level nodes grouped per environment
  (verified aws golden: `"environment": "Live"`). Environment description/properties
  are NOT in JSON (verify).
- Legacy/schema-only, do not write: `model.enterprise`; `location` on person/system
  (still present in v5-era goldens like `spring-petclinic/workspace.json` — that file
  predates v6!).

Element fields:

| Kind | Fields (besides id/name/description/tags/url?/properties?/perspectives?/relationships) |
| --- | --- |
| Person | `group` |
| SoftwareSystem | `group`, `containers[]` |
| Container | `group`, `technology`, `components[]` |
| Component | `group`, `technology` |
| CustomElement | (kind from tags; tags `Element` only) — key `customElements` (verify write) |
| DeploymentNode | `environment`, `instances`, `children[]`, `infrastructureNodes[]`, `softwareSystemInstances[]`, `containerInstances[]`, `healthChecks?`(?), `platform`?, `usage`? (verify which are serialized) |
| InfrastructureNode | `environment`, `technology`, `healthChecks?` |
| SoftwareSystemInstance | `instanceId` (int = system id), `numberOfInstances`, `dependencies[]` (container-instance ids, string), `environment`, `deploymentGroup` (legacy string) + `deploymentGroups[]` (verified array in aws golden), `healthChecks[]` |
| ContainerInstance | same minus numberOfInstances? (verify) |

`HttpHealthCheck`: `{ name, url, interval?, timeout?, headers{} }`.

`Relationship`: `{ id, sourceId, destinationId, description?, technology?, tags?, url?,
properties?, perspectives?, interactionStyle? ("Synchronous"|"Asynchronous"),
linkedRelationshipId? }` (verified). Implied rels: `linkedRelationshipId` + NO tags.

`Perspective`: `{ name, description?, value?, url? }`.

## 4. Views (`views` = ViewSet)

Collections: `systemLandscapeViews`, `systemContextViews`, `containerViews`,
`componentViews`, `dynamicViews`, `deploymentViews`, `filteredViews`, `imageViews`,
`customViews` (★ Java+code has both imageViews+customViews; schema lists only
`imageViews`), `configuration`.
Read alias: `enterpriseContextViews` → systemContextViews. (verified `ViewSet.java`)

Common view fields: `key, order, title?, description?, properties?, background?`,
`dimensions{width,height}`, `automaticLayout?`, `paperSize?` (enum), `animations?`
(steps `{order, elements[], relationships[]}`), `elements[]`, `relationships[]`.
- **v6 removed included/excluded element/relationship lists** (verified: `View.java`/
  `ModelView` have no such fields; no `includedElements` in ANY test JSON). The per-view
  `elements[]`/`relationships[]` arrays are the inclusion list AND geometry: entries may
  be `{id}` only (included, no layout), `{id,x,y}` (+`z?`, `orientation?` element;
  `points{points[{x,y}]}`, `vertices?`, `routing?`, `position?`, `description?`, `order?`,
  `response?` relationship — legacy fields listed in schema; also Java-only ★:
  RelationshipView `url`, `jump`, terminalLocations — verify before writing).
- Type-specific: SystemContext `softwareSystemId` + `enterpriseBoundaryVisible`;
  Container `softwareSystemId` + `externalSoftwareSystemBoundariesVisible`;
  Component `containerId` + `externalContainerBoundariesVisible`;
  Dynamic `elementId` + `externalBoundariesVisible` + ordered elements/relationships
  `{id, order, description?, technology?}`; Deployment `environment` (+ `softwareSystemId?`);
  Filtered `{key, order?, title?, baseViewKey, mode "Include"|"Exclude", tags[] }` —
  filter terms serialize under **`tags` as an ARRAY** (verified source; legacy
  `filter` string read-tolerance: verify);
  Image `{ elementId?, content?, contentLight?, contentDark?, contentType? }` (★ light/dark
  Java-only); Custom `{ vendor?, tooling?, content?, url?, version? }` (verify keys).
- `automaticLayout`: `{ rankDirection "TopBottom"|"BottomTop"|"LeftRight"(def)|"RightLeft",
  rankSeparation(100), nodeSeparation(50), edgeSeparation(50), vertices(true) }`
  (verified constants). Legacy `applied`, `implementation` = read-tolerate, don't write.
- Absence of `automaticLayout` = manual layout.

## 5. Styles & configuration

`configuration`: `{ styles?, themes[] (URL strings), terminology?, properties?,
metadataSymbols?, visibility?, users?(legacy), developmentMode?, lastSavedView?,
defaultView?, theme? }` — schema: styles/lastSavedView/defaultView/themes/branding(✗)/
terminology/metadataSymbols/properties (verified). `theme` singular deprecated.
`✗ branding`, `✗ enterprise` removed from Java (v6) — still in schema & v5 goldens.

`styles`: `{ elementStyles[], relationshipStyles[] }` — matched by `tag`.
- ElementStyle: `tag, shape, background, stroke, strokeWidth, color (#hex; DSL accepts
  `colour` too, JSON key is `color` — verified: zero `"colour"` in repo JSONs),
  border(Solid|Dashed|Dotted), opacity, icon, iconPosition(★), width, height, fontSize,
  font?(verify), metadata, description(bool)` + `✗ colourScheme`? (schema lists
  colourScheme — verify current code: light/dark style variants in ElementStyle).
- RelationshipStyle: `tag, thickness, color, dashed, style(Solid|Dashed|Dotted), routing,
  jump, position(0-100), opacity, fontSize, width, metadata, description`.
- `metadata` (element/relationship style metadata list): `{ display?, fontSize? }`.
- `terminology`: `{ softwareSystem, container, component, relationship, enterprise?,
  person?, code?, deploymentNode?, infrastructureNode(★) }` (verified Java + schema).

## 6. Documentation & decisions

`documentation`: `{ sections[], decisions[], images[] }`.
- Section: `{ id ("1","1.1"), title?, content (md/asciidoc), format, language?, order?,
  sections[]?, links[]? }` (verify field set vs core).
- Decision: `{ id, name/title?, description/content?, status, date (ISO Z),
  decisionStatus?, options[], consequence[], rationale? }` — JSON shape varies by era;
  verify against import module's `Decision` + goldens.
- Image: `{ name, content (base64/data-uri), contentType/type, width?, height?, title?,
  alt? }`.

## 7. Theme file format (separate file type)

`{ name, description?, version?, mode? "Light"|"Dark", elements[ElementStyle-no-tag?],
relationships[] }` — verified OCI theme: elements[] entries keyed by `tag` + `icon`
(relative icon paths resolved against theme URL at load). Loaded from URLs or
`$STRUCTURIZR_THEMES/<name>/theme.json`; styles appended under workspace styles.

## 8. Validation & fixtures

- Unique-name / dangling-reference rules enforced on load (`WorkspaceValidationException`;
  16 fixtures in `structurizr-client/.../workspaceValidation/` — see FIXTURES.md).
- Best goldens: `structurizr-export/src/test/resources/{amazon-web-services,big-bank-plc,
  groups}.{dsl,json}` (dsl+json pairs!). `spring-petclinic/workspace.json` is v5-era
  (has `location`, `branding`) — use for READ compat, not for write goldens.
- OpenAPI endpoint context: `GET/PUT /api/workspace/{id}` (docs `openapi/*`).

## 9. Do-not-write legacy cheat sheet

`location` · `enterprise` (model) · `enterprise {}` (workspace) · `branding` ·
`user`/`users` · `enterpriseBoundaryVisible`? (still written for SystemContext — verified
golden has it; keep) · `applied`/`implementation` (autoLayout) · `filter` (filtered view) ·
`colour` (styles) · `style:"..."` on element style · HMAC fields.
