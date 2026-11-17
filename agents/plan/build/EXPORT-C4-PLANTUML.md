# EXPORT-C4-PLANTUML.md — C4-PlantUML dialect catalog (M3 deltas)

The P2-D6 contract, cataloged line-by-line from the pinned monorepo
(`ad34036`) 2026-11-17. Companion to `EXPORT-PLANTUML.md` (Structurizr
dialect): shared mechanics (`IndentingWriter` §1, `idOf`/`filter` §11,
sequence engine §8, iteration §14) live there — this file catalogs ONLY the
deltas of `plantuml/C4PlantUMLExporter.java` (645 lines) over
`AbstractPlantUMLExporter`. Witness corpus: oracle `plantuml/c4plantuml`
sweep of the 43-fixture set (`/tmp/nix-shell.uni100/opencode/m2d/sweep/*/\
ref-c4/`); the 10 exit-1 fixtures are the SAME vacuous/error-abort set as
the structurizr sweep. The ExportCommand registers plain
`C4PlantUMLExporter(ctor)` — no app-injected skinparams.

## §1 Property register (`getViewOrViewSetProperty` cascade, view → set → workspace)

| Property | Default | Gate style | Effect |
|---|---|---|---|
| `c4plantuml.legend` | `"true"` | `equalsIgnoreCase` | footer `SHOW_LEGEND(true\|false)` |
| `c4plantuml.stereotypes` | `"false"` | `equalsIgnoreCase` | footer `show\|hide stereotypes` |
| `c4plantuml.tags` | `"false"` | `equalsIgnoreCase` | tags section + `$tags=` values (§4, §6, §7) |
| `c4plantuml.stdlib` | `"true"` | `equalsIgnoreCase` | `!include <C4/C4_…>` vs raw-URL includes (§3) |
| `c4plantuml.elementProperties` | `"false"` | `equalsIgnoreCase` | `WithoutPropertyHeader()`+`AddProperty` before node/element macros (§8) |
| `c4plantuml.relationshipProperties` | `"false"` | `equalsIgnoreCase` | same before `Rel` (§7) |
| `c4plantuml.sprite` / `c4plantuml.shadow` | absent | per STYLE property | tags-section `$sprite`/`$shadowing` overrides (§4) |
| `plantuml.sequenceDiagram` | `"false"` | `equalsIgnoreCase`, `DynamicView` only | sequence path (§9) |
| `plantuml.title/skinparams/includes/animation/boundaries` | as §3 M2 catalog | shared | `plantuml.title` stays case-SENSITIVE (`"true".equals`) |

`isAnimationSupported` = `!(DynamicView) && plantuml.animation == "true"`
(ignoring case); `export(CustomView)` returns `null` → the diagram (and its
file) is SKIPPED (witness: `custom-view-animation` C4 sweep writes ZERO
files); base `createLegend` returns `null` → NO `-key.puml` legend files.

## §2 Header (exact line order; non-sequence view)

1. `super.writeHeader`: `@startuml` → title legs (M2 §3; the
   `Diagram:Title`/`Diagram:Description` font-size fallback `null`/24
   quirk included) → blank → `set separator none`. Then `groupId = 0`.
2. Direction line — ALWAYS, non-sequence only: autoLayout
   `rankDirection == LeftRight` → `left to right direction`, else (other
   rank directions AND no autoLayout) → `top to bottom direction`.
3. `writeSkinParams` (M2 §3; plain ctor → only `plantuml.skinparams`).
4. ALWAYS the style block (light scheme from the LIGHT constants even for
   `Default`): `<style>` / indent / `root {` / indent /
   `BackgroundColor: #ffffff` + `FontColor: #444444` (Dark: `#111111` /
   `#cccccc` — `Styles.DEFAULT_*`, P2-V5) / outdent `}` / outdent
   `</style>` / blank line.
5. Includes (§3) then a blank line.
6. `writeIncludes` (`plantuml.includes`, M2 §3).
7. Tags section when `c4plantuml.tags` (§4).

## §3 Include selection

Sequence (`renderAsSequenceDiagram`): only `C4_Sequence`.
Otherwise: `C4` + `C4_Context` always; `C4_Container` iff any view element
(resolved, instances count) is a `Container`/`ContainerInstance`;
`C4_Component` iff any `Component`; `C4_Deployment` iff `DeploymentView`
(unconditional for deployment views). stdlib form `!include <C4/NAME>`;
non-stdlib form `!include https://raw.githubusercontent.com/plantuml-stdlib/\
C4-PlantUML/master/NAME.puml`.

## §4 Tags section (`c4plantuml.tags`, after includes)

Three `HashMap<String,Style>` maps keyed by the RESOLVED style's full tag
list (iteration = Java `HashMap` order — `oxidizr_json::javahash::
entry_order`, portable String-hashCode, unlike D8/D13). Collection: every
view element → `findElementStyle(element)` by TAG; every view relationship
→ `findRelationshipStyle(relationship)`; boundaries (non-sequence only):
ContainerView → `getBoundarySoftwareSystems`, ComponentView →
`getBoundaryContainers`, DynamicView scoped to SoftwareSystem/Container →
same (Landscape/Deployment: none). Blocks, in this order, each followed by
one blank line, and ONLY if non-empty:

* `AddElementTag("<tag minus leading `Element,`>", $bgColor="<background>",
  $borderColor="<stroke>", $fontColor="<color>", $sprite="<s>",
  $shadowing="<c4plantuml.shadow or empty>", $borderStyle="<border
  lower-case>", $borderThickness="<strokeWidth ?: 1>")`. `$sprite` =
  `img:<icon>{scale=<s>}` iff `isSupportedIcon` (http-prefixed), scale via
  D12 (network fetch pinned 0.5); a style `c4plantuml.sprite` property
  overrides even an empty computed value (`getOrDefault`).
* `AddRelTag("<tag minus `Relationship,`>", $textColor="<color>",
  $lineColor="<color>", $lineStyle = ""|DashedLine()|DottedLine())` —
  note the SPACES around `$lineStyle =` and that Solid renders the literal
  `""`. Same color both legs (upstream as-is).
* `AddBoundaryTag("<tag minus `Element,`>", $bgColor="#ffffff",
  $borderColor="<stroke>", $fontColor="<stroke>", $shadowing=…,
  $borderStyle=…, $borderThickness=…)` — background is the constant
  `#ffffff` and font = stroke (quirk, upstream as-is).

All THREE element/boundary lines post-process
`line.replace(", $borderThickness=\"1\")", ")")` — thickness 1 drops the
leg (it also hits a literal thickness-1 INSIDE the line: harmless, the
format puts it last only).

## §5 Group / boundary macros (non-sequence)

* Group: `groupId++` (reset in header); display name = group CUT after the
  last `model.groupSeparator` occurrence (Java `substring`, M2 §6) iff that
  property is set, else FULL group; color starts `#cccccc`, then Group
  `Group:<full group>` → `Group` style cascade (color, then stroke
  overrides it, then strokeWidth for thickness). Lines:
  `AddBoundaryTag("<FULL group>", $borderColor=<color>, $fontColor=<color>,
  $borderStyle="dashed", $borderThickness=<n>)` (thickness-1 replace above)
  then `Boundary(group_<n>, "<display>", $tags="<FULL group>") {` + indent;
  end: outdent, `}`, blank.
* `System_Boundary("<idOf>_boundary", "<name>", $tags="<tagsOf>") {` and
  `Container_Boundary(…same shape…)` — the id IS QUOTED (unlike elements);
  end: outdent `}` blank.
* Deployment node (before indent): elementProperties leg; `Deployment_Node(\
<idOf>, "<name> (x<instances>)", $type="<technology>", $descr="<descr>",
  $tags=…, $link="<url>") {` — `(xN)` ONLY iff `instances != "1"`; after
  indent, `hide <idOf>` iff not visible. End: outdent `}` blank.
* Sequence boundary (`startBoundary`): `box "<name>\n<size:12><typeOf with
  metadata symbols></size>"` + indent; end: `end box` (base writer, M2 §8).

## §6 Element macros (`writeElement`)

`CustomElement` → RETURN (skipped). Then: style = `findElementStyle(\
element)`; `id = idOf(element)` (M2 §11); url `""` fallback; if
`c4plantuml.elementProperties` → properties block FIRST (§8); if a
`StaticStructureElementInstance` → the MACRO describes the TARGET element
(name/description/technology/url fall back to the target's url), but
`$tags` comes from the INSTANCE's resolved style (`elementToWrite`).
Empty descr/technology → `""`. Verbs: `Person(id, "name", $descr=…,
$tags=…, $link=…)`; `System(…)`; `Container[Db|Queue](id, "name", $techn=…,
$descr=…, $tags=…, $link=…)`; `Component[Db|Queue](…)` — `Db` iff style
shape `Cylinder`, `Queue` iff `Pipe`; `Deployment_Node(idOf, "name",
$type="<technology>", $descr=…, $tags=…, $link=…)` for InfrastructureNode
(uses the node's OWN idOf, not the instance-unwrapped `id`). No macro for
kinds outside this chain (DeploymentNode bodies ride §5; a bare
SoftwareSystemInstance/ContainerInstance kind falls through ONLY after
unwrap). `hide <id>` (the element id, not `_boundary`) iff `!isVisible`.
`$tags` = resolved element style tag, `replaceFirst("Element,", "")`, or
`""` when tags off.

## §7 Relationship macro (`writeRelationship`)

CustomElement on EITHER endpoint → RETURN. `c4plantuml.relationshipProperties`
→ properties block FIRST. Response (`isResponse == true`) SWAPS the
endpoints. Description = (`"<order>: "` iff the view relationshipView has a
non-empty order) + relationshipView.description ?:
relationship.description ?: `""`. Line: `Rel(<idOf src>, <idOf dst>,
"<description>", $techn="<technology>", $tags="<rel tag minus
Relationship, or empty>", $link="<url>")`. NOTE: unlike the structurizr
dialect there is NO `<direction>`/`[[url]]` decoration and NO
relationship-style line styles outside the tags section.

## §8 Properties block (`addProperties`, gated by §1)

Filter out keys starting `structurizr.`; if any remain:
`WithoutPropertyHeader()` then per key in SORTED (byte) order:
`AddProperty("<key>","<value>")`. The Java `HashMap` here is invisible —
the `stream().sorted()` pins the order (no javahash needed).

## §9 Footer + lifecycle deltas

`writeFooter`: `SHOW_LEGEND(<true|false>)`, `show|hide stereotypes`, then
base `@enduml`. Sequence path: shared M2 §8 engine body, with `startBoundary`
per §5 (the `box` form) — boundaries gated by `plantuml.boundaries`
(default true, `equalsIgnoreCase`). Frames: animation supported per §1
(dynamic views NEVER); legend never (base null).

## §10 Quirk register (C4-only)

1. `, $borderThickness="1")` → `)` string replace (§4, §5).
2. Boundary tags-section: `$bgColor="#ffffff"` constant, `$fontColor` =
   `$borderColor` = stroke (§4).
3. `AddRelTag` renders `$lineStyle = X` WITH spaces; solid = `""`.
4. `System_Boundary`/`Container_Boundary` alias is `"<id>_boundary"` quoted;
   group alias is the counter `group_<n>` (per-diagram, reset in header);
   element macros use the BARE id.
5. `(x<instances>)` on deployment nodes iff instances ≠ `"1"` (an explicit
   `"1"` renders nothing).
6. InfrastructureNode re-computes `idOf` (identical to `id` unless the
   element was an instance — no instance wraps a node, but the Java re-reads).
7. `replaceFirst("Element,"|"Relationship,", "")` removes only the FIRST
   occurrence (a tag literally containing `Element,` mid-list survives).
8. Tags-section HashMap iteration order = javahash entry_order (portable —
   unlike D8 workspace-properties and D13 bulk fan-out).
9. `title <size:24>` uses the shared header; the `Diagram:Title` style
   font-size `null` → literal `null` quirk applies (M2 §3/§13).
10. Response relationships swap endpoints for `Rel` (dynamic views).
