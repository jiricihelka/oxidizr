# Structurizr — raw feature/functionality inventory

Research notes. Sources: local clone `/tmp/nix-shell.uni100/opencode/structurizr` (monorepo v6.2.4, commit 2026-11-07) and
`/tmp/nix-shell.uni100/opencode/structurizr-docs`. Read-only exploration for oxidizr (Rust reimpl, full DSL/JSON compatibility).
Paths below are relative to those repo roots unless stated.

---

## 1. Domain model (`structurizr-core/src/main/java/com/structurizr/`)

### 1.1 Workspace root — `Workspace.java` extends `AbstractWorkspace` (both `com/structurizr/`)
- `AbstractWorkspace` fields (CONFIRMED source): `id` (long), `name`, `description`, `version` (string), `lastModifiedDate` (Date),
  `lastModifiedUser`, `lastModifiedAgent`, `thumbnail`, `properties` (Map<String,String> — top-level workspace JSON `properties`), `configuration`? (client-side notion).
- `Workspace` adds: `model` (Model), `views` (ViewSet), `documentation` (Documentation). `WorkspaceBuilder` exists.
- **`structurizr.dsl` base64 lives in workspace TOP-LEVEL `properties`** (CONFIRMED in golden `spring-petclinic/workspace.json` lines 191-193:
  `"properties": { "structurizr.dsl": "..." }`) — NOT in configuration.properties.
- (Older `enterprise` and `location` REMOVED from Java model; still documented in JSON schema — see §3.)

### 1.2 Model — `model/Model.java`
- Holds: `people` (Set<Person>), `softwareSystems` (Set<SoftwareSystem>), `relationships` (Set<Relationship>),
  `deploymentEnvironments` (Set<DeploymentEnvironment>, ordered), `groups` (Map<String,List<...>> per kind: people/systems/containers/components/nodes),
  `properties` (Map<String,String>), `structureVerification`? no — `idGenerator` (IdGeneratorStrategy).
- Factory methods: `addPerson(spec/name)`, `addSoftwareSystem`, `Person.addSoftwareSystem`? (no — containment via parent),
  `SoftwareSystem.addContainer`, `Container.addComponent`, `addDeploymentEnvironment(name, description)`,
  `getDeploymentEnvironmentByName`, `addDeploymentNode(...)`, `DeploymentNode.add(...)` for infrastructureNode/softwareSystemInstance/containerInstance.
- `Model.getCanonicalName(...)` delegates to `CanonicalNameGenerator`.
- Element lookup: `getElement(id)`, `getElements()` (all, incl. deployment elements), `getPerson(id)`, `getSoftwareSystem(id)` etc.
- Relationship lookup: `getRelationship(id)`; `Relationship.addLabeledValue` for view-only labels.

### 1.3 Hierarchy & identity
- `ModelItem` (abstract; `id`, `tags`, `perspectives`, `url`, `properties` via `SelfReferencedInstance`? no — properties live on Element/Relationship/Model/Configuration)
  → `addTags(String...)`, `getTags()` (comma-separated string), `getTagsAsSet()` (LinkedHashSet: default tags first, then added).
- `Element extends ModelItem` (`name`, `description`, `location` REMOVED? — no: `Element.location` REMOVED in v6 (was `Location` enum);
  REMOVED confirmed via changelog-library.md; JSON schema still has `location` enum NONE/ON_PREMISES/CLOUD/COLLOCATION → schema lags Java).
- `SoftwareSystem extends Container_ (GroupableItem)`; children: `containers`; `parent` = null (systems are top-level).
- `Container`: `parent` (SoftwareSystem), children: `components`; groupable.
- `Component`: `parent` (Container); groupable.
- `Person` / `PersonSystem`? — `Person` (standalone or `buildingBlock`? no) fields: `name`, `location`? — Person keeps `location`? NO:
  Person.location REMOVED together with Element.location (changelog). Person fields: name, description, tags, url, perspectives, properties.
- `DeploymentEnvironment` (`id` UUID string, `name`, `description`, `nodes` tree, `properties`): `getNodeByCanonicalName`, `getDefaultDeploymentNode()`.
- `DeploymentNode`: `name`, `useAutomaticInstances?` — `instances` (int, default 1), `description`, `platform`? (has `setPlatform`, `usage`, `visibility`? no),
  children: `children` (DeploymentNode tree), `infrastructureNodes`, `softwareSystemInstances`, `containerInstances`; `parent`; `healthChecks` (List<HealthCheck>).
- `InfrastructureNode`: name/description/technology + healthChecks.
- `SoftwareSystemInstance` / `ContainerInstance`: reference `softwareSystem`/`container` via `instanceOfId` (JSON field `instanceId`? — JSON: `softwareSystemInstance.instanceOfId`? actually field `instanceOf` in Java, JSON key `instanceId` per schema; VERIFY in oxidizr against schema `SoftwareSystemInstance`), `numberOfInstances`, `dependencies` (Set<ContainerInstance>), `containerInstances`, `healthChecks`, `deploymentGroup` (String) + `deploymentGroups` (List<String>).
- `HealthCheck`: `name`, `interval` (default 60 s), `timeout` (default 0 ms), `healthy`? — fields: name, httpHeader, query? implementation: interval=60, timeout=0 defaults, `healthy` boolean? (has `setHealthy`; interval/timeout as above).
- `InteractionStyle` enum: `Synchronous` (default), `Asynchronous`.

### 1.4 Implicit tag stacks (exact, `model/Tags.java` + `getDefaultTags()` overrides)
- Constants: `Element`, `Relationship`, `Person`, `Software System`, `Container`, `Component`, `Deployment Node`, `Infrastructure Node`,
  `Software System Instance`, `Container Instance`, `Synchronous`, `Asynchronous`.
- Default tags per class (`getDefaultTags()` returning LinkedHashSet[Element, <Kind>]): Person→[Element, Person]; SoftwareSystem→[Element, Software System];
  Container→[Element, Container]; Component→[Element, Component]; DeploymentNode→[Element, Deployment Node]; InfrastructureNode→[Element, Infrastructure Node].
  (PersonSystem? n/a.)
- `SoftwareSystemInstance`/`ContainerInstance` ARE `DeploymentElement implements ModelItem` (do NOT extend Element) — constructors `addTags(SOFTWARE_SYSTEM_INSTANCE / CONTAINER_INSTANCE)`; docs 71: default tags = parent system/containers tags + the instance tag (no leading `Element`).
- `Relationship.getDefaultTags()` (CONFIRMED source lines 160-175): if `linkedRelationshipId != null` → **empty set** (linked/implied rels get NO tags);
  else `[Relationship]` + (Synchronous|Asynchronous) ONLY IF `interactionStyle` explicitly set. `Model.addRelationship` default passes `interactionStyle = null`
  ⇒ **normal DSL relationship tags = exactly `"Relationship"`** (CONFIRMED in golden spring-petclinic: `"tags" : "Relationship"`; docs 71 agrees).
  NOTE: current DSL has NO way to set interactionStyle (no token; relationship block children = tags/url/properties/perspectives only) — Synchronous/Asynchronous only via JSON/Java API.
- `ModelItem.getTags()` order: DEFAULT tags first, then explicitly added (LinkedHashSet) — e.g. person with custom tag "Admin" → "Element,Person,Admin".

### 1.5 ID generation & canonical names
- `validation/IdGeneratorStrategy` interface; `SequentialIntegerIdGeneratorStrategy` — ONE counter shared by elements AND relationships in the model, in definition order ("1","2","3"... strings in Java).
- `found()` variant? — `SequentialIntegerIdGeneratorStrategy` implements `generateNextId`/`setValue`/`found(String id)` (updates max seen).
- `model/CanonicalNameGenerator`:
  - Person: `Person[<name>]`
  - SoftwareSystem: `SoftwareSystem[<name>]`
  - Container: `SoftwareSystem[<sys>]:Container[<name>]`
  - Component: `SoftwareSystem[<sys>]:Container[<c>]:Component[<name>]`
  - DeploymentNode: `<envName>:DeploymentNode[<name>]`, nested: `<env>:DeploymentNode[<parent>]:DeploymentNode[<child>]`
  - InfrastructureNode: `...:InfrastructureNode[<name>]`
  - SoftwareSystemInstance: `...:SoftwareSystemInstance[<name>]` where name = system name + optional `[<n> instances]` suffix when instances≠1? — instance canonical uses parent node chain + `SoftwareSystemInstance[<systemName>]`.
  - ContainerInstance: `...:ContainerInstance[<systemName>:<containerName>]`.
- `Model.getElementByCanonicalName(String)`, `getDeploymentNodeByName` etc.

### 1.6 Groups
- `model/GroupableItem` interface: `getGroup()`/`setGroup(String)`. People, systems, containers, components, deployment nodes (and instances) support one `group` string.
  Serialized as property `group` on the element JSON (verify: yes, schema `group` string on person/system/container/component/deploymentNode etc.).

### 1.7 Views — `view/` package
- `ViewSet`: `configuration` (Configuration), views collections:
  - `systemLandscapeViews`, `systemContextViews`, `containerViews`, `componentViews`, `dynamicViews`, `deploymentViews`, `filteredViews`, `customViews`,
    `onpremisesViews`?? REMOVED, and `imageViewViews`? — the `image` view DSL token maps to a `CustomView` with key `Image-...` and an embedded content? — NO: DSL `image` creates a `CustomView`? VERIFY (from parser notes: image content { plantuml|mermaid|kroki|image } builds `CustomView` with vendor="Image", plus embedded? treat as CustomView+property).
  - JSON keys: `systemLandscapeViews`, `systemContextViews`, `containerViews`, `componentViews`, `dynamicViews`, `deploymentViews`, `filteredViews`; back-compat `@JsonSetter("enterpriseContextViews")` maps to systemContextViews (ViewSet.java). `customViews` exists in Java but NOT in OpenAPI schema.
- `View` (abstract): `key`, `autoLayout` (AutomaticLayout), `background`? (has `background` color, `paperSize`? — has `setPaperSize(PaperSize)` enum for print, JSON `paperSize`? VERIFY in oxidizr), `animation` (Animation), `dimensions` (width/height), `views` included/excluded element/relationship filtering:
  - `getElements()`/`addIncludedElement(s)`, `remove...` → JSON `includedElements` (array of element ids), `includedRelationships` (array of relationship ids), `excludedElements`, `excludedRelationships`, `includedGroups`? — JSON: `includedElements`, `excludedElements`, `includedRelationships`, `excludedRelationships`.
  - `View.location` REMOVED.
- `ListView` (View) → `RelationshipView` (View) → typed views:
  - `SystemLandscapeView`, `SystemContextView` (scope system; `addAllInboundConnections`, `addPeers`, `addElements`...), `ContainerView` (scope system, `addAllInfluencers`, `addUsingRelationships`), `ComponentView` (scope container; `addScopeElements`, `addDirectDependencies`), `DynamicView` (scope element; ordered `DynamicElementViewModel`/`DynamicRelationshipViewModel` entries; parallel sequences `startParallelSequence()`/`endParallelSequence()`; `add(relationshipId, description, order)`; entries serialized as `elements`/`relationships` arrays with `id`+`order`+`description`),
  - `DeploymentView` (scope = deployment environment name; `addAll`? `setDeploymentEnvironment(name)`; JSON `deploymentViews[].environment`).
  - `CustomView` (`vendor`, `tooling`, `content` embedded, `url`, `version`).
- `FilteredView` extends View: `baseViewKey`, `mode` ("Include"|"Exclude"), `filter` (string, comma-separated tags/predicates) — its JSON `tags` is an ARRAY? — note: FilteredView has `getTags()`? Filtered views carry `includedElements` etc. computed; in JSON: `key`, `view` (base key), `mode`, `filter`, plus element/relationship inclusion arrays.
- `View.animation`? → `Animation` with `steps` (AnimationStep: order, elements, relationships).
- `AutomaticLayout` (`view/AutomaticLayout.java`): fields `rankDirection` (enum, DEFAULT = **LeftRight**), `rankSeparation` (def 100), `nodeSeparation` (def 50),
  `edgeSeparation` (def 50), `vertices` (boolean, default true). RankDirection enum values EXACT: `TopBottom, BottomTop, LeftRight, RightLeft` (JSON strings same case).
  `enabled`? no — enabled is not a field; DSL `autolayout` presence enables it. NO `distance`, NO `seed`, NO `optimise`.
  JSON (golden) shows legacy keys `applied` (bool) and `implementation` ("Graphviz"/"Dagre") written by tooling but NOT model fields — reader ignores unknown.
- `ElementView` (per-view geometry: `id`, `x`, `y`, `width`, `height`, `z`, `orientation`) — serialized JSON has ONLY `id`, `x`, `y` when x/y set (NON_NULL; width/height/z only when non-default? observed golden: `{"id":"...","x":...,"y":...}`; `z` 0 omitted, `orientation` null omitted).
- `RelationshipView`: `id`, `x`, `y`, `points` (`Points` with `Point[]` x/y pairs), `distance`? — JSON: `id`, `x`, `y`, `points.points[]`; plus Java-only `url` and `jump` (`jump` int? `Jump`? — Java has `setUrl`, `getJump`/`setJump` (relationship jump distance) — not in schema).
- `TerminalLocations`? (Java `RelationshipView.terminalLocations` start/end enum) — present in Java; check schema (likely not). VERIFY in oxidizr before implementing.

### 1.8 Styles — `view/Styles.java`, `ElementStyle.java`, `RelationshipStyle.java`
- `Styles`: `elements` List<ElementStyle>, `relationships` List<RelationshipStyle>, `paperSize`? (Styles has `setPaperSize`? no — Theme/Styles both have styles list; `Styles` also has `terminology`? no).
- `ElementStyle` (JSON `elementStyles[]`): `tag` (required), `shape` enum EXACT [Shape.java]: Box, RoundedBox, Circle, Ellipse, Hexagon, Diamond, Cylinder, Bucket, Pipe, Person, Robot, Folder, WebBrowser, Window, Terminal, Shell, MobileDevicePortrait, MobileDeviceLandscape, Component (DSL/JSON use same names; no Cube/File/Sequence). `background`, `stroke`, `strokeWidth`, `colour` (JSON key `colour`!), `border` enum EXACT [Border.java]: Solid, Dashed, Dotted. `opacity`, icon` (URL or data-URI), `width`, `height`, `fontSize`, `font`, `metadata` (List<Metadata>), `description` (bool), `iconPosition` (Java-only, VERIFY).
  `fontSize`, `font`? (Java has `font` → JSON `font`? schema has `font`), `opacity`, `icon` (URL or data-URI), `iconPosition`? (Java-only? present: `iconPosition` not in schema? VERIFY), `width`, `height`, `metadata` (List<Metadata> order-able: `display` enum Name/Description/Technology? plus `fontSize`), `description` (boolean), `orientation`? Java-only `setOrientation` (TopLeft...)? skip, `addSeparator`? no.
- `RelationshipStyle` (JSON `relationshipStyles[]`): `tag`, `thickness`, `color` (JSON key `color`! asymmetric vs `colour` for elements), `dashed`, `style` enum (Solid, Dashed, Dotted), `opacity`, `routing` enum (Orthogonal? — Routing: Orthogonal, Polyline? — `Routing` values: Orthogonal, Polyline? and curved? VERIFY), `jump` (Jump? boolean/int), `linePosition`? `position` (int 0-100 label position), `fontSize`, `width` (label width), `metadata` (List<Metadata>), `description` boolean.
- KEY asymmetry: ElementStyle JSON key = `colour`; RelationshipStyle JSON key = `color`. (Jackson property names.)

### 1.9 Configuration — `view/Configuration.java`
- Fields: `developmentMode` (bool, default true), `styles` (Styles), `theme` (String, deprecated, default "default"), `themes` (String[]),
  `terms`? — `terminology` (Terminology), `properties` (Map incl. keys `structurizr.plantuml.export.*`, `structurizr.agent`? etc.),
  `user`/`users` (deprecated `User` class REMOVED from JSON; DSL `users` block still tokenized), `visibility`? — DSL token exists, maps to Configuration? VERIFY.
- `STRUCTURIZR_CLOUD_SERVICE_THEMES_URL = "https://static.structurizr.com/themes/"` (default theme base).
- `Terminology`: `softwareSystem` (e.g. "Application"), `container` (e.g. "Service"), `component` (e.g. "Code"), `relationship` (e.g. "Flow"),
  `diagram`?? — Java also has `infrastructureNode` (Java-only, not in schema). Default all null (unset).
- `Metadata` inner class: `display` (enum: Name? — actually Metadata.Display enum Name/Description/Technology/Group/Instance/Properties/Reference? — verify: enum in `com.structurizr.view.Metadata`), `fontSize`.

### 1.10 Documentation — `documentation/`
- `Documentation` (sections recursive), `Section` (id string "1", "1.1"... orderable, title, content (markdown), language, links), `Image` (content, contentType, width, height, title, alt), `Link` (name, hyperlink).
- Decisions: `decision/Decision.java` abstract (id, name, decisionStatus/date, description, option(s) title+description, consequence(s) title+description, date (Date), status, rationale) + `impl/SimpleDecision`/`SimpleDecisionStatus` etc.; `DecisionStatus` ("Accepted"...), `DecisionItem`.
- `SetOfDecisions` collection with `getOrderedByStatusThenId` etc.

### 1.11 Validation — `validation/`
- `ValidationUtils` static checks: names/ids/descriptions/tags non-empty rules, URL/identifier rules; `WorkspaceScopeValidator` (`WorkspaceScopeValidatorFactory`) enforces one-parent rules (`DefaultWorkspaceScopeValidator`, `SharedScopeWorkspaceScopeValidator`).
- Errors surface as `ValidationException`/`ModelValidationException` with aggregated messages.

### 1.12 Misc core
- `util/SystemLandscapeGenerator` — copies landscape-relevant elements into a new workspace (used by `generate system-landscape`).
- `filter/` — `ElementFilter`/`RelationshipFilter`/`Filter` with `ElementFilterType` (Tag, Type, Name, Group, Id, Technology, Instance?, Expression? — `and`/`or`/`not` expressions: `AllElementFilter`? implemented as tag/type/regex filters + logical composition) — used by filtered views and inspections.
- `Comparison`? ABSENT (no ViewPortal/comparison in core — grep found nothing: force-directed graph/tree/comparison are UI-side features, not model features).

---

## 2. DSL language (`structurizr-dsl/src/main/java/com/structurizr/dsl/` + `structurizr-docs/dsl/`)

### 2.1 Files
- `StructurizrDslParser.java` (main), `StructurizrDslTokens.java` (all keywords), `Tokenizer.java` (lines/tokens), `NameValueParser.java`,
  `IdentifiersRegister.java`, `DynamicViewContentParser.java`, `ImageViewContentParser.java`, `DecisionsParser.java`, `DocsGenerator`? `WorkspaceParser.java` (extends/merge), `ThemeParser.java`,
  contexts: `AbstractScopeDslContext`, `ModelScopeDslContext`, `PersonScopeDslContext`, `SoftwareSystemScopeDslContext`, `ContainerScopeDslContext`, `ComponentScopeDslContext`,
  `GroupScopeDslContext`, `DeploymentEnvironmentDslContext`, `DeploymentNodeScopeDslContext`, `CustomElementDslContext` (`element "..." { name/description/metadata/tags/url/properties }` for `!element` finders),
  `*ViewDslContext`s, `FilteredViewDslContext`, `StylesDslContext`/`ElementStyleDslContext`/`RelationshipStyleDslContext`, `ConfigurationDslContext`,
  `DynamicViewParallelSequenceDslContext` (keyword `parallel`), `PluginDslContext` (`!plugin`), `ScriptDslContext` (`!script`), `FindElementsDslContext` etc.

### 2.2 Complete token inventory (verbatim, `StructurizrDslTokens.java`)
- Assignment `=`; model: `element`, `person`, `softwareSystem`, `container`, `component`, `group`, `->`, `-/>(remove)`, `name`, `metadata`, `description`, `technology`,
  `instances`, `tags`, `tag`, `url`, `properties`, `perspectives`/`perspective` (+`description`,`value`,`url`), `this`.
- top: `workspace`, `extends`, `scope`, `model`, `archetypes`, `views`, `enterprise` (token exists, handler throws "removed/deprecated" error),
  `deploymentEnvironment`, `deploymentGroup`, `deploymentNode`, `infrastructureNode`, `instanceOf`, `softwareSystemInstance`, `containerInstance`, `healthCheck`.
- views: `custom`, `systemLandscape`, `systemContext`, `container`, `component`, `dynamic`, `deployment`, `filtered`, `image`,
  `include`/`exclude` (in view), `animation`+`animationStep`, `autolayout`, `default`, `title`, `description`; content-source tokens `plantuml`, `mermaid`, `kroki`, `image`.
- styles: `styles`, `element`/`relationship` style scopes, `shape`, `background`, `stroke`, `strokeWidth`, `colour`/`color`, `icon`, `iconPosition`, `opacity`,
  `border`, `fontSize`, `width`, `height`, `metadata`, `description`, `thickness`, `dashed`, `routing`, `jump`, `style`, `position`,
  `theme`, `themes`, `light`/`dark` (color scheme for image views? — light/dark are theme-adjacent tokens).
- configuration: `configuration`, `visibility`, `terminology` (+`relationship`, `softwareSystem`, `container`, `component`), `metadata` (symbols config), `users`.
- directives: `!include`, `!docs` (+`exclude`), `!adrs`, `!decisions` (+`exclude`), `!constant` (deprecated), `!const`, `!var`, `!identifiers`, `!impliedRelationships`,
  `!ref` (deprecated), `!extend` (deprecated), `!element`, `!elements`, `!relationship`, `!relationships`, `!plugin`, `!script`,
  `!components` (+`classes`,`source`,`filter`,`strategy`(+`technology`,`matcher`,`filter`,`supportingTypes`,`name`,`description`,`url`,`forEach`)).
- relationship archetypes: prefix `--` + name + `->` (e.g. `--reads->`).

### 2.3 Grammar/structure semantics (`StructurizrDslParser.java` + docs `dsl/71-language.md`)
- Top-level: optional `workspace "name" "desc" { ... }` or bare statements (implicit workspace); `model {}`, `views {}`, `configuration {}` scopes optional; anything else runs directly against the right context ("top-level shorthand" allowed: `person`, `softwareSystem`, relationship statements, etc.).
- Element statements (CONFIRMED verbatim, docs `dsl/71-language.md`):
  - `person <name> [description] [tags] { ... }` — NO technology arg; `softwareSystem <name> [description] [tags] { }`;
  - `container <name> [description] [technology] [tags] { }`; `component <name> [description] [technology] [tags] { }`;
  - custom `element <name> [metadata] [description] [tags] { }` (default tags: [Element] only);
  - `deploymentNode <name> [description] [technology] [tags] [instances] { }`; `infrastructureNode <name> [description] [technology] [tags] { }`;
  - `softwareSystemInstance|containerInstance|instanceOf <identifier> [deploymentGroups] [tags] { }` — identifier refers to system/container; deploymentGroups = comma-separated ids;
    default tags = parent element's tags + `Software System Instance` / `Container Instance` (docs 71, "In addition to the software system's tags").
  - **Identifiers via assignment**: `clinicEmployee = person "Clinic Employee" "An employee..."` (CONFIRMED in golden's base64 DSL); inside blocks `x = container "..." { }`.
  - Block children mirror docs 71 "Permitted children" lists (description/tags/url/properties/perspectives + nested kinds + group + relationships).
  - `healthCheck <name> <url> [interval] [timeout]` (single statement, no block; interval s default 60, timeout ms default 0).
  - `group` block nesting table: Model→people+systems, SoftwareSystem→containers, Container→components; also `group "Name"` as child statement to set element's group.
- Relationship statements (CONFIRMED docs 71): `id? = <identifier> -> <identifier> [description] [technology] [tags] { tags|url|properties|perspectives }`;
  source-implicit form `-> <identifier> [description] [technology] [tags]` (source = enclosing element, same as `this -> ...`); relationship archetypes used as arrow: `a --https-> b "desc"`.
  Default tags: `Relationship` only. Source/destination type matrix enforced (Person/System/Container/Component↔anything; DeploymentNode→DeploymentNode only;
  InfrastructureNode→node/infra/instances; *Instance→InfrastructureNode only). `-/&gt;` inside `deploymentEnvironment` removes replicated instance relationship.
- `uses` keyword REMOVED (legacy DSL). `this` = the element in scope (relationship source position). Reusing identifiers via `-> id` is just a normal relationship to the identified element.
- Scoping: `scope shared { ... }` (`SCOPE_TOKEN` "shared") allows elements to be referenced across systems; default scope = strict (one-parent). `ModelScopeDslContext`, `SharedScopeDslContext`.
- Groups: `group "name" { ... }` at person/system/container/component/deployment-node scopes.
- Deployment: `deploymentEnvironment "name" "desc" { deploymentNode ... } }` — node tree nests (`deploymentNode` inside `deploymentNode`); children per docs 71: group, deploymentNode, infrastructureNode, softwareSystemInstance, containerInstance, instanceOf, relationships, description/technology/instances/tags/url/properties/perspectives. `instances` both positional arg and child statement. `healthCheck <name> <url> [interval] [timeout]` (interval s def 60, timeout ms def 0). Deployment groups: `deploymentGroup`/`deploymentGroups` positional arg on instances + `deploymentGroup "name"` child stmt on nodes/instances. `instanceOf <id>` = alias keyword for softwareSystemInstance/containerInstance. Model-element relationships are replicated onto instances; `-/&gt;` removes a replicated relationship.
- Archetypes (CONFIRMED docs `dsl/06-archetypes.md`): scope `archetypes { }` (in model); definitions by assignment: `application = container { technology "Java" tag "Application" description/technology/tags/properties/perspectives/url defaults }` (bases: person|softwareSystem|container|component); archetypes extend archetypes: `springBootApplication = application { ... }`; relationship archetypes: `sync = -> { tags "Synchronous" }`, extend via `https = --sync-> { technology "HTTPS" }`. Application: element — archetype id replaces the kind keyword: `webapp = springBootApplication "Web Application"`; relationship — arrow form `a --https-> b "desc"`.
- Implied relationships: toggle `!impliedRelationships <true|false>` (parser default = on) or `!impliedRelationships <fqcn>` (portable-breaking);
  default strategy `CreateImpliedRelationshipsUnlessAnyRelationshipExistsStrategy`; created rels carry `linkedRelationshipId` (getter `getLinkedRelationshipId`) and NO tags (see §1.4).
- Variables/constants: `!const "NAME" "value"` (immutable; redefine → error), `!var "NAME" "value"` (redefinable), lookup: constants map → `System.getenv(name)`;
  `${NAME}` substitution across all strings (preprocessing pass in `StructurizrDslParser`); name charset `[a-zA-Z0-9-_.]`; `env()`/`$()` LEGACY, gone.
- Text: `//` line comments (stripped unless inside strings?), `#` full-line comments, `/* ... */` multi-line; line continuation `\` at EOL; text blocks `"""multi-line"""` (tokenizer keeps raw newlines; used in view titles/descriptions/doc content).
- Includes: `!include ./relative.dsl` (relative to including file; nested includes ok; also supports directories? no — files; URL includes? `!include` file path only) — `structurizr-docs/dsl/31-includes.md`.
- Workspace extension: `workspace "..." extends "file-or-url" { ... }` — base can be `.dsl` or `.json`; merge = base model + overlay; ids preserved from base? (extension merges workspaces; `WorkspaceParser.java`); docs `dsl/32-workspace-extension.md`.
- Docs/ADRs: `!docs ./docs [exclude ...]` scans dir for markdown → workspace documentation (`DocsParser`? inside parser: `structurizr-docs/dsl/51-docs.md`); `!adrs ./adr` / `!decisions ./adr` with format from folder (`DecisionsParser` supports `structurizr` (workspace.dsl style .md/.json), `adr-tools`, `madr`, `log4brains`) + optional exclude; docs `dsl/52-adrs.md`.
- `!identifiers flat` (default) | `hierarchical` (parent identifiers joined with `.` — `IdentifiersRegister`); identifier regex: `\w[a-zA-Z0-9_-]*` (must start with word char); duplicate identifier → parse error.
- `!element "System A"` / `!elements [tag...]`? — element PREDICATES: `!element X` finds by name-path; `!elements "tag"` filter strings (tag/type/regex per §1.12 filter syntax) usable as relationship endpoints and in `include`/`exclude` in views.
- `!plugin com.foo.MyPlugin` loads a `StructurizrDslPlugin` (SPI `structurizr-dsl-plugin`? interface `StructurizrDslPlugin` with `setWorkspace`/`run`) — `docs/dsl/62-plugins.md`? VERIFY page name; `!script` = JS via javax.script? (`docs/dsl/61-scripts.md` — scripting with Nashorn/GraalJS in newer builds).
- Styles in DSL: `views { styles { element "Tag" { ...props } relationship "Tag" { ... } } themes { url "..." } }`; `theme "name"` (deprecated → configuration theme URL); `ThemeParser.java` maps theme names to `https://static.structurizr.com/themes/<name>/theme.json`; bare `themes {}` scope holds URL lines; default theme URL const: `ThemeParser.DEFAULT_THEME_URL = "https://static.structurizr.com/themes/default/theme.json"`.
- Image views: `image { plantuml { ...raw } | mermaid { ... } | kroki "..." { ... } | image "path|data-uri" }` → creates `CustomView` (vendor "Image"?) via `ImageViewContentParser.java`; content stored in custom view `content` (embeds source or image URL), key `Image-NNN`? (generated).
- Dynamic view statements: id-ordered relationship refs `1 = a -> b "uses" "HTTP"`? inside `dynamic "sys-or-id" "title" { ... }`; shorthand `a -> b "msg" [technology] [id]` auto-increments order; `parallel { ... }` for parallel sequences (`DynamicViewParallelSequenceDslContext` → view.startParallelSequence/endParallelSequence); `animationStep N { elements... relationships... }`? (animation via `animation` keyword inside view).
- Filtered views: `filtered "baseKey" "key" "title" "desc" { include "tags" / exclude "tags" }` (mode + filter string).
- View inclusion statements: `include *` = add everything in scope (golden-confirmed), or `include`/`exclude` with identifiers, name-paths, filter strings (tags/types via core `ElementFilter`/`RelationshipFilter`), group names. Filtered-view `include`/`exclude` take filter strings (comma terms).
- DSL portability (CONFIRMED docs 71 §workspace + golden): after parse, when DSL is "portable" the parser stores base64 flattened DSL in **workspace top-level `properties["structurizr.dsl"]`** (golden line 191). Non-portable if any of: `workspace extends`, `!impliedRelationships <fqcn>`, `!include <file|directory>`, `!script`, `!plugin`, `!docs`, `!adrs`/`!decisions`, `!components`, `icon <file>`, `theme <file>`, `logo <file>`. Workspace property `structurizr.dsl.source=false` discards source even when portable. Every element gets property `structurizr.dsl.identifier` (golden shows hierarchical dotted ids like `springpetclinic.webapplication.ownercontroller`); relationships get it ONLY when explicitly named.
- Error format: `StructurizrDslParserException` wrapping: `"<message> at line N of <absolute-file-path>: <trimmed line text>"` — NO column numbers; first error thrown aborts parse (`parse()` collects? — throws on first).
- Other parser behaviors: relationship statement `A -> B` where either side is a system/container/component/person/instance found by name-path across scopes incl. deployment envs (name resolution order: identifier → full name → canonical); `metadata` token = alias for `tags` (DSL uses `metadata "..."` to add tags); `tags` on view elements n/a; element `location` statement REMOVED (no `location` token — grep: token absent from tokens file, confirmed).

### 2.4 Permitted-tokens enforcement
- Each `*DslContext.getPermittedTokens()` returns `Set<String>` of legal statements in that scope (parser throws `"... is not valid here"`). Extracted per-context lists in full in earlier session; key ones:
  - ModelScope: person, softwareSystem, deploymentEnvironment, group, archetypes, relationships, !include, properties, !elements/!element as statements allowed, `scope`...
  - SoftwareSystemScope: container, group, name/description/metadata/tags/url/properties/perspectives/archetypes? + relationships (from contained) ...
  - Views scope: all view-kind tokens + styles + themes + `default` + animation.
  - Configuration scope: properties, themes, terminology, visibility, users, developmentMode? (`developmentMode` tokenized? — not in tokens file; set via configuration scope statement `properties`? hmm — `developmentMode` handled as raw statement keyword in ConfigurationDslContext; VERIFY), styles? no.

---

## 3. JSON format (the contract)

### 3.1 Sources
- OpenAPI schema: `structurizr-json/structurizr.yaml` (OpenAPI 3.0, components schemas, workspace `version` property default `"6.1.0"` — note: schema v6.1.0 vs library v6.2.4 → schema LAGS Java code).
- Actual (de)serialization: Jackson on domain objects — `structurizr-client/src/main/java/com/structurizr/io/json/AbstractJsonWriter.java` + `ModelJsonSerializer`? (module-level mixins) and `AbstractJsonReader.java`; `WorkspaceJsonWriter`/`WorkspaceJsonReader`, `ThemeUtils` for themes.

### 3.2 Serialization rules (AbstractJsonWriter etc.)
- ObjectMapper: `ORDER_MAP_ENTRIES_BY_KEYS`? — writer emits alphabetically-ordered object properties (Jackson `SerializationFeature.ORDER_MAP_ENTRIES_BY_KEYS` + POJO alphabetical? — observed goldens: alphabetical field order); `NON_NULL` + NON_EMPTY inclusion (null/empty omitted); unknown fields ignored on read (`FAIL_ON_UNKNOWN_PROPERTIES=false`).
- Dates: ISO `yyyy-MM-dd'T'HH:mm:ss'Z'` UTC (documentation/decisions `date`).
- `id`: numeric (`long`) on `workspace.id`; STRINGS everywhere else (elements, relationships, views elements ids).
- `tags` = comma-separated STRING "Element,Person[,custom...]" on element/relationship JSON (default tags first); NOT an array (array only in theme? no — theme styles use single `tag`).
- Element views (`{view}.elements[]`) serialize ONLY `{id,x,y}` (+`z`,`orientation` when set); relationship views `{view}.relationships[]` serialize `{id,x,y,points{points[]}}` (+`terminalLocations`,`url`,`jump` when set).
- Dynamic views: `{view}.elements[]` = `{id, order, description?, technology?, ...}`, `{view}.relationships[]` = `{id, order, description}`.
- `filteredViews[]`: `{key, view (base key), mode ("Include"|"Exclude"), filter, includedElements[], includedRelationships[] ...}`.
- `configuration.themes` = array of theme URLs (JSON); `configuration.terminology.{softwareSystem,container,component,relationship}`; Java also has `terminology.infrastructureNode` (not in schema).
- `enterpriseContextViews` — read-back-compat via `@JsonSetter` on ViewSet → maps into `systemContextViews`; never written.
- Schema-vs-Java drift (Java = truth for v6.2.4): REMOVED from Java but still in schema: `enterprise` (workspace), `branding` (configuration), `location` (element/deploymentNode/person? and `Location` enum), `user`/`users`? (User in configuration deprecated/removed from JSON writing), `style` on element (`style` key legacy), `paperSize`? — ADDED in Java, missing from schema: `customViews`, `RelationshipView.url/jump`, `Terminology.infrastructureNode`, `ElementStyle.iconPosition`, `Group` fields? (group present), `perspectives` (schema HAS perspectives? — VERIFY: schema 6.1.0 includes `Perspective` schema — yes, perspectives are in schema), `deploymentGroups` on instances (Java `List<String> deploymentGroups` + legacy `deploymentGroup` string — schema has `deploymentGroup` only? VERIFY), `Metadata` display enum values extended?
- `properties`: three levels — **workspace top-level `properties`** (holds `structurizr.dsl` base64, golden-confirmed), `model.properties`, `configuration.properties` (all string maps). Element/relationship/view-level `properties` too.
- Instance relationship replication: deployment relationships are separate Relationship objects in `DeploymentEnvironment.relationships` with `linkedRelationshipId` pointing at the model relationship; removal via DSL `-/&gt;`; `SoftwareSystemInstance` JSON: `{id, instanceId, numberOfInstances, dependencies[], deploymentGroup, deploymentGroups[], healthChecks[], tags, name, url?, properties}` — `instanceId` = id of the SoftwareSystem.
- Golden sample: `structurizr-dsl/src/test/resources/dsl/spring-petclinic/workspace.json` — canonical example of serialized output incl. implied relationships (tag-less? NO — implied rels DO serialize with `linkedRelationshipId` and NO tags at all: `"tags":""`? omitted? — from notes: implied relationships get NO default tags → serialized with NO `tags` key (NON_EMPTY omission), plus `linkedRelationshipId`).
- `AutomaticLayout` JSON: `{enabled, rankDirection, rankSeparation, nodeSeparation, edgeSeparation, avoidNodeOverlap}` — no `distance`.
- HealthCheck JSON: `{name, interval, timeout, healthy?...}` with defaults interval 60, timeout 0 (omitted when default? NON_EMPTY/NON_NULL still writes numbers — defaults DO serialize as numbers; VERIFY against golden).

### 3.3 Version handling
- JSON `version` string field (arbitrary); schema documents `"6.1.0"` as current format marker; `structurizr-client` readers don't validate it (ignore-unknown everywhere = forward/back tolerant).

---

## 4. Export (`structurizr-export/src/main/java/com/structurizr/export/`)

- Base: `Exporter` (abstract; `getKey()`, `getMimeType()`, `getDefaultFileExtension()`, theme/style plumbing, `outputFolder?`), `DiagramExporter extends Exporter`
  (`export(workspace) → Collection<Diagram>`; per-view methods `export(SystemLandscapeView...)` etc.), `Diagram` (key, name, description, definition, legend (Diagram), frames (List<Diagram>) for animation, fileExtension, mimeType, location? REMOVED).
- `WorkspaceExporter` (abstract, exports whole workspace): `JsonWorkspaceExporter` (`.json`), `JsonWorkspaceThemeExporter` (theme from styles).
- Diagram exporters present:
  - `plantuml/StructurizrPlantUMLExporter` (dialect `!theme structurizr`-style? emits `!include`-free skinparam block; key `plantuml`, ext `.puml`),
  - `plantuml/C4PlantUMLExporter` (C4-PlantUML macros: `System_Ext`, `Container_bc` etc.; key `c4plantuml`, ext `.puml`; supports color schemes via constructor `ColorScheme.Light|Dark`; legend support),
  - `mermaid/MermaidDiagramExporter` (flowchart/graph; dynamic→`sequenceDiagram`? C4? — emits `flowchart`/`sequenceDiagram` for dynamic; key `mermaid`, ext `.mmd`),
  - `websequencediagrams/WebSequenceDiagramsExporter` (only dynamic views; ext `.txt`),
  - `dot/DotExporter` (Graphviz DOT; EXISTS in module but NOT wired into CLI export map),
  - `ilograph/ILographExporter` (Rational ILograph; module-only, not in CLI),
  - `WebSequenceDiagrams` above; NO D2, NO PlantUML-shared-layout, NO Graphviz/ELK exporter in Java monorepo (autolayout for exports = none; layout for web comes from JS).
- Dynamic view exports emit `frames` (one Diagram per animation step/parallel step) — CLI writes `-N` suffixed files.
- Legend: C4PlantUML exports an extra `*-key.puml` (when `generateLegend`? default on when styles used? — legend exported if `diagram.getLegend() != null`; CLI writes `<prefix>-<key>-key.<ext>`).
- CLI format→exporter map (`structurizr-application/.../command/ExportCommand.java`):
  `json`→JsonWorkspaceExporter; `theme`→JsonWorkspaceThemeExporter; `plantuml`, `plantuml/structurizr`(+`-light`,`-dark`)→StructurizrPlantUMLExporter;
  `plantuml/c4plantuml`(+light/dark)→C4PlantUMLExporter; `mermaid`; `websequencediagrams`; `static`→StaticSiteExporter (HTML site, application module);
  `png`/`svg`→PlaywrightExporter (renders static site headlessly; optional build; `--url` for remote page, `--mode light|dark`, `--animation true|false`);
  `fqcn` = custom exporter by fully-qualified class name (`findExporter` reflection fallback `loadClass(format, workspacePath)` — any class implementing `Exporter` on classpath/`-l` lib dir? loadClass resolves FQCN).
  Output naming: `structurizr-<workspaceId>-<viewKey>.<ext>` (`prefix(workspaceId)` → "structurizr-N" or "structurizr" when id≤0); json/theme exports: `<input-basename>.json` / `<basename>-theme.json`.
  Diagram exports call `ThemeUtils.loadThemes(workspace, httpClient)` first (resolves theme URLs incl. relative icon paths → absolute URLs).
- Docs pages: `structurizr-docs/commands/04-export.md` (format table), `docs/export/` legacy pages (CLI-era) — `docs.structurizr.com/export/cli` referenced by AGENTS.md.

## 5. Import (`structurizr-import/src/main/java/com/structurizr/importer/`)

- Decisions: `AbstractDecisionsImporter`; implementations: `adrtools/AdrToolsDecisionsImporter` (ADR-tools `0001-*.md` with `# title`, status/decision/options/consequences markdown parsing), `madr/MadrDecisionsImporter` (MADR templates incl. v2/v3 "Considered Options"/"Decision Outcome"), `log4brains/Log4BrainsDecisionsImporter` (front-matter YAML status/date).
  Used by DSL `!adrs`/`!decisions` (auto-detect format by folder contents) and by application `import`? (no import command — decisions import only via DSL directives; diagram import via web only? — also used by `structurizr-client`? — the diagram importers below are used by Structurizr Web/Lite UI (server-side) and by `LocalCommand`? — imported diagrams used by `ui` era; in monorepo they're library features).
- Diagram importers (`diagram/`): `AbstractDiagramImporter` → `plantuml/C4PlantUMLDiagramImporter` (parses C4-PlantUML `System_Boundary`/`Container_Boundary`/`Rel` macros into model+view, layout from C4 placement), `mermaid/MermaidDiagramImporter` (flowchart with C4 stereotypes `<<person>> <<system>> ...`), `image/ImageDiagramImporter` (embeds image as CustomView), `kroki`? (`ImageDiagramImporter` covers kroki? separate? — `kroki` handled by UI; importer set: c4plantuml, mermaid, image, plus `d2`? NO d2).
- Both decision & diagram importers are the ONLY import surface (besides JSON workspace reader). Docs: `structurizr-docs/import/` (diagrams) + `docs/decisions/`.

## 6. Other modules

### 6.1 `structurizr-autolayout/`
- `GraphvizAutomaticLayout` (com.structurizr.layout): feeds DOT of view to bundled `graphviz-java` (nidi3) — `run()` mutates ElementView/RelationshipView coords (x,y, points); supports `rankDirection` mapping, node size estimation by content (`NodeSize` heuristics: shape/width/height/font), `optimise`/rank hints (`GraphAttribute`/`NodeAttribute`/`EdgeAttribute` DOT attribute writers), `Rank` grouping via `addInvisibleEdge`? — key fact: Java autolayout = Graphviz dot; JS (web) uses dagre/ELK; oxidizr will eventually reimplement (ELK or dot) but JSON contract is just final coordinates.

### 6.2 `structurizr-inspection/` (`com.structurizr.inspection`)
- `Inspector` runner; `inspections/` = `DefaultInspector` (aggregate of individual checks), `OrphanElementInspection`, `EmptyDeploymentEnvironmentInspection`, `ContainerWithoutComponentsInspection`? set includes: orphan person/system/container/component detection, missing technology/description?, duplicate names?, relationships across scopes?, element without relationships, deployment nodes without instances... (full list: `DefaultInspector.inspections` — check for `*Inspection` classes: `OrphanPersonInspection`, `OrphanSoftwareSystemInspection`, `OrphanContainerInspection`, `OrphanComponentInspection`, `ElementWithNoRelationshipsInspection`?, `UnusedViewInspection`? — treat as advisory; NOT part of JSON contract; docs `commands/14-inspect.md`).
- `SummaryReporter` outputs single-line `summary:` properties (workspace name, element counts, relationship counts, view counts, "no issues found" or issue counts) — used by `structurizr local`/`inspect` and playground validation.
- Severity `Severity` enum (Warning? Info? Error?) + `Problem`/`Problems` per entity type (model + views packages `inspection.inspections.model` / `.view`).

### 6.3 `structurizr-themes/`
- Resource-only module: `structurizr-themes/default/*` (default theme.json + icon files), plus cloud theme packs? (default theme only in monorepo; cloud themes live at static.structurizr.com).
- `Theme` model in core: `com.structurizr.view.Theme` (`name`, `version`, `mode` ("Light"|"Dark"), `elements` List<ElementStyle>, `relationships` List<RelationshipStyle>) + `ThemeUtils` in structurizr-client (`fromJson`/`toJson`/`installThemes(File dir)`/`loadThemes(workspace)` — copies theme styles into workspace styles; installed themes resolve via `InstalledThemes` singleton → `~/.structurizr/themes`? install dir managed by application: `<user home>/.structurizr`? `AbstractCommand.installThemes` uses `themes` dir next to binary/config + `STRUCTURIZR_THEMES` env override); default themes URL base `Configuration.STRUCTURIZR_CLOUD_SERVICE_THEMES_URL = "https://static.structurizr.com/themes/"`; DSL default theme `ThemeParser.DEFAULT_THEME_URL = ".../default/theme.json"`.

### 6.4 `structurizr-annotation/`
- Java annotations to generate models from code: `@StructurizrStructure`? classes: `com.structurizr.annotation` — `ArchitectureElement`, `ElementType`? actual: `@StructurizrModel`? (small module: annotations + `com.structurizr.discovery`? treat as Java-only, irrelevant to oxidizr compat but note as module).

### 6.5 `structurizr-component/`
- Java component finder for DSL `!components`: `com.structurizr.component.discoverer` — `JavaPackagesComponentDiscoverer`, `SpringComponentDiscoverer` (stereotype detection), `JavaClassFinder` (scans dir/URL jar of `.java`/`.class`?), strategies in `strategy/`: `PackageBasedStrategy`, `AbstractStrategy` (filters: `ComponentFilter`, `SupportingTypeFilter`, `CompositeStrategy`), `ComponentBuilder`/`ComponentSet`; DSL `!components "System" "Container" { classes source ... strategy { matcher ... } }` maps to `ClassesDslContext`? (`ComponentFinderDslContext` in dsl module wires `com.structurizr.component.*`).

### 6.6 `structurizr-client/`
- `io/json`: `WorkspaceJsonReader`/`WorkspaceJsonWriter` (Jackson, §3), `AbstractJsonReader/Writer`.
- `io/git`: `GitWorkspaceResolver`? (`com.structurizr.io.git`) workspace from git repo (used by server command), `io/filesystem`.
- `api/` — Structurizr Web REST client (`BaseApiClient`, `EnterpriseApiClient`? REMOVED with enterprise; `WorkspaceApiClient`, `UserApiClient`, `BranchApiClient`, `DiagramApiClient`, `GenerateIdApiClient`? for id-generation service) used by `push`/`pull`/`branches`/`create`/`delete`/`lock` commands; `http/HttpClient` (allowlist security model, timeouts).
- `view/ThemeUtils`, `WorkspaceScopeValidator`? (core), `listener`? (change listeners `AbstractWorkspaceChangeListener` + `ChangeSet`/`ChangeType` for workspace diffs — used by server for revision history), `diff/`? (`WorkspaceComparator`? maybe `com.structurizr.util.WorkspaceUtils`? — `WorkspaceComparator` in client produces change sets).
- `com.structurizr.util.WorkspaceUtils.loadWorkspace(File)` — DSL-vs-JSON autodetect by extension.

### 6.7 `structurizr-application/` (the unified binary; docs `structurizr-docs/commands/`)
- `Main` + `com.structurizr.command.*`: `playground`, `local` (≙ Lite: `local [path] --port` watch+serve UI at http://localhost:8080; UI assets bundled), `server` (on-premises server w/ DB? "structurizr server" HTTP + WebDAV? — command `server`), `export` (§4), `import`? (no), `pull`, `push`, `branches` (list/create/delete), `create`, `delete`, `lock`/`unlock`, `merge` (branch merge via UI? `merge` command runs browser merge tool), `validate`, `inspect`, `list` (list workspaces for api creds), `generate system-landscape` (`GenerateCommand`: SystemLandscapeGenerator → new dsl+json), `regenerate-apikey`, `mcp` (§6.8). Flags: `-w/--workspace` path or URL(s), `-id`/`--id` enterprise?→workspace id, `-key`/`--api-key`, `-secret`, `--user-email`.
- `AbstractCommand.loadWorkspace`: file/URL, DSL+JSON (`WorkspaceUtils`), installs themes (`./themes` dir or `$STRUCTURIZR_THEMES`), validates scope (`WorkspaceScopeValidatorFactory`), runs `DefaultInspector` (reports via logging), auto-creates default views when model non-empty & views empty (`ViewSet.createDefaultViews()` → landscape + context/container(+autolayout TopBottom)/component views per system; keys `SystemLandscape-001`, `SystemContext-001`, `Container-001`, `Component-001`... sequential `Prefix-NNN`).
- `StaticSiteExporter` (application): self-contained HTML site (index.html + JS UI bundle + workspace.json + theme + icons) used by `export -f static` and PNG/SVG pipeline.
- `PlaywrightExporter(+Impl)`: headless chromium render of static site → PNG/SVG per view (optional module, reflection-loaded).
- `com.structurizr.mcp.Server` — MCP server (`mcp` command; Spring AI `@McpTool` methods; stateless HTTP transport): tools: `validate` (DSL/JSON), `parse` (DSL→workspace JSON?), `inspect`, `export-plantuml`, `export-c4plantuml`, `export-mermaid`, `server-workspace-create/read/update/delete` (CRUD against local server) — docs `structurizr-docs/ai/mcp.md`.
- Packaging: `war`/docker `structurizr/structurizr` (versioned 2026.x); single binary replaces legacy cli/lite/onpremises.

## 7. Ecosystem context (archived repos, docs-only)

- `structurizr/cli` (archived): standalone jar `structurizr-cli` — commands (export/validate/...) fully superseded by `structurizr` application binary (`export` etc.); docs page `structurizr-docs/cli/` now stub pointing to `commands/`.
- `structurizr/lite` (archived): free local UI + DSL live preview jar → replaced by `structurizr local`. `structurizr-docs/lite/` = stub.
- `structurizr/ui` (archived): Angular UI for Web/on-prem; UI code now shipped inside application binary (static site/local/server assets); `structurizr-docs/ui/` stubs.
- `structurizr/onpremises` (archived): Docker on-premises server → replaced by `structurizr server` command; `structurizr-docs/onpremises/` stub.
- SaaS: structurizr.com web (workspaces, git integration, branches) — API-compatible target of `push`/`pull` via structurizr-client `api/`.
- Docs map worth mirroring for oxidizr compat work: `structurizr-docs/{dsl,commands,workspaces,web,lite,cli,onpremises,local,themes,decisions,import,export,ai,features.md,binaries.md}`; key pages: `workspaces/file-types.md` (.json/.dsl/.jar (legacy), workspace "properties"), `features.md` (UI features incl. force-directed graphs, comparison view = client-side, NOT model), `binaries.md` (download/versioning), `commands/00-index? 01-playground...` command reference, `dsl/71-language.md` (authoritative grammar+defaults), `dsl/91-faq.md`.
- Legacy GPL artifacts on Maven Central (structurizr-java ≤ 1.x era, structurizr/java archived) — Apache-2.0 monorepo only source.

---

## Cross-cutting facts for oxidizr (compat traps)

1. ONE integer id counter across elements+relationships; IDs serialized as strings (workspace.id numeric!).
2. Tag string order: default tags first (custom appended, e.g. `Element,Container,Relational Database Schema`). **Normal relationship default tag = `Relationship` ONLY** (Synchronous/Asynchronous added only if interactionStyle explicitly set, which DSL can't). Linked/implied rels = NO tags. Instance elements = parent tags + instance tag (no leading `Element`). `-/&gt;` removes replicated instance rel; instance rels duplicate model rels with `linkedRelationshipId`.
3. `colour` vs `color` key asymmetry (ElementStyle vs RelationshipStyle); FilteredView filter strings; `enterpriseContextViews` read-alias; NON_NULL/NON_EMPTY omission; alphabetical keys; date format UTC `Z`.
4. DSL: `${VAR}` substitution (+env fallback), `!const`/`!var`, `!include`, `workspace extends` (dsl|json), `!docs`/`!adrs`(+exclude), `!identifiers hierarchical`, `!impliedRelationships`, text blocks `"""`, comments `//`,`#`,`/**/`, `\` continuation, `this`, `-/>`, archetypes incl. `--name->`, parallel dynamic sequences, image/custom views, `!plugin`/`!script`, `!components`, `!element(s)`/`!relationship(s)` finders, `scope shared`; errors = line+file+source line (no column).
5. Removed from current Java model (do NOT implement even though schema lists): `enterprise`, `branding`, `location`, `env()`, `$()`, `uses`, `!ref`, `!extend`, `!constant`, `enterprise {}`, `theme default`, Graphviz autolayout in exports? (autolayout module still dot), `deploy {}` scope (now `deploymentEnvironment`), `user`/`users` JSON? (token remains, JSON user deprecated).
6. Java-only (schema lags): `customViews`, RelationshipView `url`/`jump`, Terminology `infrastructureNode`, `iconPosition`, deploymentGroups list.
7. Default views generation + scope validation + inspections run on load in application layer — NOT parser/library responsibility.
8. Themes: default theme URL `https://static.structurizr.com/themes/default/theme.json`; theme loading resolves relative icon paths against theme base; `STRUCTURIZR_THEMES` dir for offline themes; `export -f theme` dumps workspace styles as theme JSON.
