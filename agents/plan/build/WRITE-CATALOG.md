# WRITE-CATALOG.md — the JSON write contract (P1-D6, resolves V-1/V-2/V-11)

Authoritative catalog for `oxidizr-json` (M3): every rule the `structurizr`
reference applies when writing a workspace as JSON (`structurizr-client`
`com.structurizr.io.json.JsonWriter`/`JsonReader` @ monorepo `ad34036`,
library 6.2.4; app 2026.10.26; verified empirically against the oracle output
of `testdata/probes/m3-order.dsl` 2026-11-14). The golden
`testdata/golden/m3-order.json` is the byte-level witness for this file.

Jackson semantics that drive everything (from `AbstractJsonWriter`):

- `MapperFeature.SORT_PROPERTIES_ALPHABETICALLY` — POJO keys are written in
  **case-sensitive alphabetical order by property name** at every level,
  regardless of class hierarchy. No POJO carries `@JsonPropertyOrder`.
- Inclusion is **NON_EMPTY** (`setSerializationInclusion` called twice; the
  last call wins — NON_NULL is dead). "Empty" per Jackson ≥2.9 ([databind
  #1449]): `null`, `""`/empty `CharSequence`, empty
  collection/map/array. **Primitive numbers/wrappers/booleans are NEVER
  empty** (so `id: 0`, `x: 0`, `y: 0`, `instanceId`, primitive booleans like
  `externalBoundariesVisible: false` are ALWAYS written) and `Integer 0` /
  `Boolean false` with explicit `@JsonInclude(NON_NULL)` are written too.
- POJO-typed fields are never "empty" — non-null beans always serialize even
  with no content: top-level `configuration {}`, `documentation {}`, `model {}`
  and `views.configuration { styles {}, terminology {} }` (also
  `documentation {}` on every Documentable element — see §4).
- Pretty print: `DefaultPrettyPrinter` + `DefaultIndenter("  ", "\n")`:
  2-space indent per level, LF line ends, key/value separator `" : "`
  (spaces around the colon), `,` then newline between entries, empty object
  `{ }`, empty array `[ ]`, arrays of SCALARS inline (`[ "a", "b" ]`), arrays
  containing objects expand (`[ {` … `} ]`), no trailing newline at root.
- Dates: `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss'Z'")`, TZ=UTC (fields
  `lastModifiedDate` only; export never stamps).
- String escaping: Jackson defaults — `"` `\` and `< 0x20` only
  (`\b \t \n \f \r`, else `\u00XX`); non-ASCII, `/`, DEL pass through raw.
- Numbers: Java `long`/`int` decimal, no grouping; `workspace.id` is the only
  numeric ID — every other ID is a JSON string.

## 1. Free-form `properties` ordering (V-11 resolution)

`properties` maps are Java `HashMap`s — **serialized in hash-bucket order,
not sorted, not insertion order**. Proof: the probe's person properties
(`structurizr.dsl.identifier`, aaa…mmm inserted in that order) serialize as
`identifier, aaa, ccc, bbb, eee, ddd, ggg, fff, iii, hhh, kkk, jjj, mmm, lll`.

**Decision: replicate exactly.** `oxidizr-json` implements a Java-`HashMap`
order emulator:

1. Per key: `h = java_string_hash_code(s)` (i32 wrap arithmetic,
   `h = 31*h + utf16_unit`); spread `h ^ (h >>> 16)` (logical shift).
2. Capacity: start 16; after each insertion, if `size > 3*cap/4` double cap.
   Final cap only depends on the final size `N`: smallest power of two ≥ 16
   with `N ≤ 3*cap/4` (no removals occur in this codebase).
3. Iteration = buckets `0..cap-1`, entry `i` lands in bucket
   `spread(i) & (cap-1)` (final cap), within-bucket order = insertion order
   (Java 8 resize splits preserve relative order).
4. The emulator needs the **insertion order**, so `properties` is stored as an
   insertion-ordered sequence in the core (a `BTreeMap` would be wrong).
   Insertion order to replay: JSON reads = document order (Jackson
   `MapDeserializer` puts keys as encountered → read-then-write is
   byte-stable); DSL reads = the reference parser's put sequence (M4:
   `structurizr.dsl.identifier` is stored at element-creation time, i.e.
   before any `properties` statement — probe witness).

Accepted edge-case deviations (recorded in `DEVIATIONS.md`):

- Buckets that Java would treeify (≥ 8 colliding keys in one bucket at
  capacity ≥ 64) iterate in tree order there, insertion order here.
  Property-key sets that large are not realistic; a property test pins the
  emulator against the probe vectors.
- Capacity boundary: keys stripped after export (e.g. `structurizr.inspection.*`)
  change the reference's final capacity; goldens are stripped per M0 policy,
  and our maps never contain them.

## 2. Top-level and `model`

| JSON path | Keys (alphabetical; `?`=only when non-empty/non-null) |
| --- | --- |
| workspace | `configuration` (bean, always), `description?`, `documentation` (always), `id` (long, always incl. 0), `lastModifiedAgent?`, `lastModifiedDate?`, `lastModifiedUser?`, `model` (always), `name?`, `properties?` (§1), `thumbnail?`, `version?`, `views` (always) |
| workspace.configuration | `scope?` (enum name `Landscape`/`SoftwareSystem`), `users?` (`[{username, role}]`, TreeSet by username; `role` **omitted when null** — unknown/absent binds null; users DO round-trip), `visibility?` (enum name) |
| model | `customElements?`, `deploymentNodes?`, `people?`, `properties?` (§1), `softwareSystems?` — each array TreeSet-ordered by **numeric id ascending** (§5) |
| model.*.…relationships | nested under the SOURCE element only; TreeSet numeric-id order; implied/linked rels ride along (no tags, `linkedRelationshipId` set, technology copied) |

`ModelItem` visible properties: `id`, `tags` (comma string: derived defaults
++ stored, dedup, insertion order), `url?`, `properties?` (§1),
`perspectives?` (TreeSet by `name`, fields `name`, `description?`, `value?`,
`url?`). `Element` adds `name?`(validation-enforced), `description?`,
`relationships?`. `GroupableElement` adds `group?`.
`canonicalName`/`defaultTags`/`tagsAsSet` are `@JsonIgnore`-suppressed
(annotation on any accessor kills the whole property — Jackson rule).

| Element kind | Extra keys |
| --- | --- |
| Person | — (NOT Documentable: no `documentation`) |
| SoftwareSystem | `containers?`, `documentation` (always `{ }` — lazy bean) |
| Container | `components?`, `documentation` (always), `technology?` |
| Component | `documentation` (always), `technology?` |
| CustomElement | `metadata?` (String, DSL never sets it), `parent` (getter not ignored; always null → never written) |
| DeploymentNode | `children?`, `description?`, `environment` (String, always), `infrastructureNodes?`, `instances` (**String**, default `"1"`; primitive bean → always), `name`, `softwareSystemInstances?`, `containerInstances?`, `technology?`, `deploymentGroups?` (TreeSet sorted) |
| InfrastructureNode | `environment`, `technology?` |
| StaticStructureElementInstance (`softwareSystemInstances[]`/`containerInstances[]`) | full `ModelItem`+`GroupableElement` shared keys (`description?`, `group?`, `url?`, `perspectives?` — deployment elements ARE `GroupableElement`) + `containerId`/`softwareSystemId`, `deploymentGroups?` (TreeSet), `environment`, `healthChecks?` (TreeSet by name, url), `id`, `instanceId` (**int**, always) — **no `name`** (getName `@JsonIgnore` kills the whole Jackson property, both directions; reader derives name from the linked element after validation) |
| HealthCheck | `headers?` — `getHeaders()` returns `new HashMap<>(TreeMap)`, so Jackson emits keys in **HashMap BUCKET order** replayed from UTF-16-sorted insertion (probe 2026-11-14: `{"X-A","X-B"}` writes `X-B` first — writer replays `javahash::entry_order`), `interval` (int, always), `name`, `timeout` (long, always), `url`. READ: `name` AND `url` are required non-null (`HttpHealthCheck.hashCode()` NPEs on Jackson's first `HashSet` insert otherwise — reference witness 2026-11-14); `interval`/`timeout` absent → the bean's `0`/`0` initializers (the 60 s interval is the DSL parser's, not the reader's) |

Instance tag stacks (re-verified): `Software System Instance` / `Container
Instance` only — no `Element`, no parent tags.

## 3. `views` (ViewSet)

`views` keys (alphabetical): `componentViews?`, `configuration` (always:
`styles` bean + `terminology` bean always; `defaultView?`, `lastSavedView?`
(legacy `@JsonGetter`), `metadataSymbols?`, `properties?`, `themes?`
(insertion-order String array), `viewSortOrder?`), `containerViews?`,
`customViews?`, `deploymentViews?`, `dynamicViews?`, `filteredViews?`,
`imageViews?`, `systemContextViews?`, `systemLandscapeViews?`. Unknown read
alias `enterpriseContextViews` → systemContextViews (§6); never written.

Every view object writes `description?`, `generatedKey?` (NON_DEFAULT: only
`true`), `key`, `name` (**computed, always written** — §3.1), `order` (int),
`properties?` (§1), `title?` on top of its type-specific keys. View lists are
TreeSet-ordered `(order asc, key compareToIgnoreCase)`.

| View kind | Type-specific keys |
| --- | --- |
| systemLandscapeViews | `animations?`, `automaticLayout?`, `description…`, `dimensions?`, `elements?`, `enterpriseBoundaryVisible` (**primitive bool, default true, always**), `paperSize?`, `relationships?` |
| systemContextViews | same + `softwareSystemId`, (`enterpriseBoundaryVisible` default true) |
| containerViews | `animations?`, `automaticLayout?`, `dimensions?`, `elements?`, `externalSoftwareSystemBoundariesVisible` (**always**, default false), `paperSize?`, `relationships?`, `softwareSystemId` |
| componentViews | same shape; `containerId`, `externalContainerBoundariesVisible` (default false) |
| dynamicViews | `automaticLayout?`, `dimensions?`, `elementId?`, `elements?`, `externalBoundariesVisible` (**always**, default false), `paperSize?`, `relationships?` — NO `deploymentEnvironment` (private field, no accessors: invisible both directions) |
| deploymentViews | static-view shape + `environment`, `softwareSystemId?` |
| filteredViews | `baseViewKey`, `mode` (`Include`/`Exclude`; **omitted when null** — unknown/absent read value binds null), `tags` (**string array**, TreeSet UTF-16 order) — no elements/relationships (extends View, not ModelView); legacy `filter` read key is IGNORED (no setter) |
| customViews | `animations?` + ModelView shape (no `softwareSystemId`? it is present when scoped — extends ModelView) |
| imageViews | `content?`, `contentDark?`, `contentLight?`, `contentType?`, `elementId?` + View base only |

`elements[]` = `{id, x, y}` — `x`/`y` primitive ints, **always written**
(probe: `"x" : 0, "y" : 0`). `relationships[]` = `RelationshipView`:
`description?`, `id`, `order?` (String!), `position?`, `properties?` (§1),
`response?` (Boolean wrapper NON_NULL — `false` IS written), `routing?`,
`jump?`, `url?`, `vertices?` (array of `{x,y}` ints, insertion order).
ModelView `automaticLayout` (bean when set): `edgeSeparation`,
`nodeSeparation`, `rankDirection` (enum name — **omitted when null**),
`rankSeparation`, `vertices` (**primitive bool** — always written when
autoLayout set). READ defaults are the bean's Java field initializers
`0/0/0/false` + a NULL rank direction (probe `testdata/probes/m3b-defaults.json`);
the `LeftRight/100/50/50/true` defaults belong to the DSL parser, NOT the
reader. `setRankDirection` throws `A rank direction must be specified.` on a
PRESENT-but-unparsable value (unknown enum → null) — the only guarded enum
setter in the model; an absent key never calls it. `dimensions` = `{height, width}` (primitives,
always). `paperSize` = enum name. `animations[]` = `{order, elements[],
relationships[]}` (steps in insertion order; id sets TreeSet).

### 3.1 Computed view `name` values (must replicate exactly)

| Kind | `name` |
| --- | --- |
| SystemLandscapeView | `System Landscape View` |
| SystemContextView | `System Context View: ` + software system name |
| ContainerView | `Container View: ` + software system name |
| ComponentView | `Component View: ` + software system name + ` - ` + container name |
| DynamicView | `Dynamic View: ` + element name; for a container element: `Dynamic View: ` + parent system name + ` - ` + container name; no element: `Dynamic View` |
| DeploymentView | `Deployment View: ` + environment, prefixed `software system name - ` when scoped to a system |
| FilteredView | `Filtered: ` + base view's computed name (base must exist) |
| CustomView | `Custom View: ` + title |
| ImageView | the view's `title` verbatim |

## 4. `documentation`

`{ sections?, decisions?, images? }` — all three always-written-when-nonempty
(TreeSet/List beans, NON_EMPTY hides the empty ones → `{ }`).
`sections` = definition order (ArrayList); section object:
`content`, `elementId?`, `format` (`Markdown`/`AsciiDoc` enum name; **omitted
when null** — unknown/absent binds null, same for `Decision.format`), `order`
(int, always), `title` (`@JsonInclude(ALWAYS)` → **always `""`**; `filename`
is `@JsonIgnore` — read-only: the reader reads `filename`, the write side only
sees the computed `title`).
Element-level documentation: the `documentation {}` bean on
SoftwareSystem/Container/Component always serializes (§2 table).
decisions/images shapes are P4 (`!docs`/`!adrs` deferred) — read leniently,
write only what was read.

## 5. Array & map ordering summary (V-1)

| JSON path | Order rule |
| --- | --- |
| people / softwareSystems / customElements / deploymentNodes / containers / components / children / infrastructureNodes / *Instances / every nested `relationships[]` | **numeric id ascending** (Java `Integer.parseInt` semantics: `-?\d+`, overflow → text compare; non-numeric ids fall back to UTF-16 text ascending) |
| view lists (each kind) | `order` asc, then `key` case-insensitive (Java `compareToIgnoreCase` = UTF-16 units lowercased) |
| view `elements[]` | numeric id ascending (same parseInt fallback) |
| view `relationships[]` | text `id + "/" + order` UTF-16 ascending; null order concatenates as the literal string `null` |
| styles `elements[]` / `relationships[]` | **load order, not re-sorted**: the DSL/builder path inserts in `AbstractStyle.compareTo` order (null `colorScheme` first, then `tag`; both schemes → `colorScheme + "/" + tag` — the Java fields are `TreeSet`s; `Styles::insert_element_style`/`insert_relationship_style`), while the JSON reader keeps **document order** — Jackson binds these two by FIELD (`Styles` has NO setter, so unlike every `setX` above the `TreeSet` is replaced; oracle probes 2026-11-17, `testdata/golden/m3-order.json` + `/tmp/nix-shell.uni100/p2/v1/out5`). The writer replays model order |
| filtered `tags`, animation id sets, `deploymentGroups`, perspectives (by name), healthChecks (by name, then url) | Java `String.compareTo` = **UTF-16 code-unit** lexicographic (≠ Rust `str::cmp` for non-BMP vs U+E000-class keys — implement a `java_ord` comparator) |
| headers map | **HashMap bucket order** replayed from UTF-16-sorted (`TreeMap`) insertion — `new HashMap<>(TreeMap)` copy in `getHeaders()` (writer: `javahash::entry_order`) |
| themes / vertices / sections / animation steps | insertion / definition order |
| every `properties` map | §1 HashMap emulator |

## 6. Reader rules (leniency matrix — `JsonReader`)

`AbstractJsonReader`: unknown properties ignored
(`FAIL_ON_UNKNOWN_PROPERTIES=false`), unknown enum values → null
(`READ_UNKNOWN_ENUM_VALUES_AS_NULL`), `ACCEPT_EMPTY_STRING_AS_NULL_OBJECT`.
Jackson binding additionally gives:

- **Read**: aliases/setters — `enterpriseContextViews` → landscape views
  (REPLACES the set: the last key in document order wins); `theme` (string) →
  `themes[]`; `defaultView`/`lastSavedView`; `users` (`setUsers` is
  package-private — Jackson setter visibility ANY — so users round-trip;
  `Section.filename` binds from the read side);
  `instances` accepts JSON numbers (coerced to string) and validates exactly as
  `DeploymentNode.setInstances`: `Integer.parseInt` first (only `< 1` throws),
  else the `\d*\.\.\d*` (a ≤ b) and `\d*..N` / `\d*..\*` shapes — where the
  middle dots of the last two are UNESCAPED regex dots (backtracking lets
  `12N` match with an empty `\d*`); `""` fails both branches (rejected); an
  ABSENT key never calls the setter (default `"1"` survives). Violations throw
  a read error (`Number of instances must be a positive integer or a range.` /
  `Range upper bound must be greater than the lower bound.`).
- **Read normalizes** (setter-side value changes, oracle probes 2026-11-14):
  style colors `background`/`stroke`/`color` via `Color` — `#rrggbb`
  lowercased, HTML name (148-entry `Color.NAMED_COLORS` table, case
  insensitive, NO `transparent`) → lowercased hex, anything else errors
  `<color> is not a valid hex colour code or HTML colour name.` (present-but-
  null → message `null is not a valid …`); the same setters must normalize
  DSL colors in M4/M5. `themes[]`: trim, skip empty, dedupe URLs
  (`addTheme`); "installed" bare-name themes are filesystem-dependent in the
  reference — oxidizr keeps every name (D3). `lastModifiedDate`: ISO text or
  epoch millis (number/all-digit string) → epoch seconds. Style NUMBERS bind
  through their setters: `strokeWidth` pins to [1, 10], `opacity` (element and
  relationship) and relationship `position` pin to [0, 100] — null leaves the
  field unset; `width`/`height`/`fontSize`/`thickness` are NOT normalized
  (oracle probes 2026-11-17: `strokeWidth 0` reads back `1`, `position 150`
  reads back `100`).
- **Read requires**: healthCheck `name` + `url` non-null (§2 row — the
  reference's own read NPEs; oxidizr surfaces `JsonError::Read`);
  `automaticLayout.rankDirection` present ⇒ must parse (guarded setter, §3).
- **Read-and-keep** (round-trip): every key listed in §2–§4, incl.
  `structurizr.dsl` / arbitrary `properties` maps (document order becomes
  insertion order).
- **Read-and-ignore** (legacy keys — getter-only or absent → unknown):
  element `location`; `enterprise`; `branding`; instance
  `numberOfInstances`/`dependencies`/`deploymentGroup` (string); dynamic
  `deploymentEnvironment`; filtered `filter` object; automaticLayout
  `applied`/`implementation`/`seed`; view `name` (computed on write);
  `canonicalName`; section `title` (`@JsonGetter`, no setter — the
  read side binds `filename` instead; `filename` itself is `@JsonIgnore`
  WRITE-side, so a read→write cycle rewrites `title` as the computed name
  — `""` when the input had no `filename`; `Decision.title` is a normal
  round-tripping property).
- Reader post-step: `workspace.hydrate()` = exactly the M2 validation catalog
  (`validate()`), i.e. JSON loads run validation the same way the app does.
  Instance `name`/`description` derive from the linked element AFTER the
  link checks pass (reader must fill them; validation fixtures prove the
  order).

## 7. Determinism notes

- Export writes `structurizr.dsl` (workspace property) = raw DSL bytes
  base64-encoded — verbatim file content, comments included (M4 must reproduce
  exactly; probe witness).
- `lastModifiedDate`/`User`/`Agent`: round-trip when present, never stamped
  (export is deterministic). The CLI strips the four
  `structurizr.inspection.*` properties that the app's inspector adds — the
  committed goldens follow the M0 rule (reference bytes minus those keys).
- View `order` for DSL-created views = `views.size() + 1` at creation
  (probe: sequential 1..5 across kinds; interim V-4 rule re-confirmed).

## 8. Grammar discoveries from the probe (for M4/M5, logged here once)

- `filtered <baseKey> <include|exclude> "<tags-csv>" [key] [description]`
  (mode lowercase token; tags ONE csv token; base view must not have
  automatic layout).
- `deployment <*|system-identifier> <environment> [key] [description]`.
- dynamic steps reference EXISTING relationships; the scope element may not
  re-appear in a step.
- Deployment groups: declared `id = deploymentGroup "name"` (statement, no
  block) inside `deploymentEnvironment`; referenced by **identifier** from the
  `deploymentGroup "id1,id2"` statement inside a deployment **node** (the only
  context accepting it). References resolve to the group **name**
  (`DeploymentElementParser`); an unresolvable reference is silently ignored
  (backwards-compat branch). Groups never appear as model elements — only as
  the TreeSet-sorted `deploymentGroups` string array on nodes/instances.
  (`group "name" { }` inside an environment is the unrelated ElementGroup.)
- No async (`~>`) arrow in v6 DSL; interactionStyle is never set by the DSL.
- `configuration { properties {} }` entries land in WORKSPACE properties.
- Workspace scope values are `Landscape` / `SoftwareSystem`; landscape scope
  rejects containers at the scope-validation stage (live V-10 evidence).
- Parent↔child relationships (`system -> its container`, `container -> its
  component`) throw at DSL parse: "Relationships cannot be added between
  parents and children".
- Style `jump` takes `true|false` (no `Round`/`Arc` in 6.2.4).
