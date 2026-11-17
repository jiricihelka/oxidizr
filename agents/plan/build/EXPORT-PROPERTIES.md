# EXPORT-PROPERTIES.md — exporter property catalog (P2-D8 deliverable)

Pinned against the Structurizr monorepo at `ad34036` (= library 6.2.4 = the oracle
CLI 2026.10.26). Java paths below are relative to
`structurizr-export/src/main/java/com/structurizr/export/` unless stated; line
numbers are at the pin. Exhaustive grep of property-name string literals in the
four diagram exporters + `AbstractDiagramExporter` + `AbstractExporter`. Nothing
here is ported from memory — re-grep the pin if in doubt. Companion decisions:
`PHASE_2.md` P2-D8/D9/D10.

## 1. The cascade — `AbstractDiagramExporter.getViewOrViewSetProperty` (766–772)

```java
view.getProperties().getOrDefault(name,
        views.getConfiguration().getProperties().getOrDefault(name, defaultValue))
```

**Exactly TWO levels + default**: view → `views.configuration` → default. The
MODEL's properties are NOT consulted (the early P2-D8 wording assumed three
levels — corrected; `useAlphanumericShortIds`-style and "enterprise name"
property reads do not exist at this pin). Match semantics live at each call
site; the `match` column below is part of the contract: `eq` =
`"true".equals(v)` (case-SENSITIVE — `TRUE` reads as false!), `eqi` =
`equalsIgnoreCase`, `raw` = value used verbatim.

oxidizr implements this as ONE helper `export_property(view, views, name,
default) -> &str` in `oxidizr-export` — no scattered `properties.get("…")`.

## 2. View/ViewSet properties (the cascade)

| property | dialect | read site(s) | default | match | effect |
|---|---|---|---|---|---|
| `plantuml.title` | both PlantUML | AbstractPlantUMLExporter 180 | `true` | **eq** | `title …` block in header |
| `plantuml.includes` | both PlantUML | 256 | *(empty)* | raw | `!include <x>` per comma item, trimmed, empties skipped, order kept, one blank line after |
| `plantuml.skinparams` | both PlantUML | 228 | *(empty)* | raw | `skinparam { name value }`; comma items need `=` (else skipped), `split("=")` keeps only parts 0–1 (values truncated at 2nd `=`), items trimmed |
| `plantuml.animation` | both PlantUML | 185 | `false` | eqi | animation frame diagrams emitted |
| `plantuml.sequenceDiagram` | both PlantUML | 271 | `false` | eqi | dynamic views render as sequence diagram; **gated on `view instanceof DynamicView`** |
| `plantuml.boundaries` | both PlantUML | 286 (Abstract), 41 (StructurizrPlantUMLExporter) | `true` | eqi | group/software-system/container boundaries rendered |
| `c4plantuml.legend` | C4-PlantUML | C4PlantUMLExporter 631 | `true` | eqi | separate legend diagram exported |
| `c4plantuml.stereotypes` | C4-PlantUML | 635 | `false` | eqi | `<<stereotypes>>` on macro calls |
| `c4plantuml.tags` | C4-PlantUML | 639 | `false` | eqi | `$tags=` on macro calls (`Element,`-prefix stripped) |
| `c4plantuml.stdlib` | C4-PlantUML | 643 | `true` | eqi | `!include C4.puml` stdlib vs inlined includes |
| `c4plantuml.elementProperties` | C4-PlantUML | 384, 451 | `false` | eqi | per-element property blocks (see §4) |
| `c4plantuml.relationshipProperties` | C4-PlantUML | 574 | `false` | eqi | per-relationship property block (see §4) |
| `mermaid.title` | Mermaid | MermaidDiagramExporter 392 | `true` | **eq** | `---` front-matter title |
| `mermaid.sequenceDiagram` | Mermaid | 396 | `false` | eqi | dynamic views as `sequenceDiagram` |
| `mermaid.icons` | Mermaid | 292 | `false` | **eq** | icon `<img>` in node label; also requires style `icon` non-empty AND `startsWith("http")` (399–401) |

`WebSequenceDiagramsExporter` reads **no** view/view-set properties.

## 3. Style-level properties (on the RESOLVED element style — never the cascade)

| property | dialect | read site(s) | default | effect |
|---|---|---|---|---|
| `plantuml.shadow` | Structurizr-PlantUML | 127, 173, 211, 247, 301, 345 | `false` (eqi) | per-element tag `Shadowing` |
| `c4plantuml.sprite` | C4-PlantUML | 197 | computed `img:<icon>{scale=n}` when icon supported, else `""` | `$sprite=` on `AddElementTag` |
| `c4plantuml.shadow` | C4-PlantUML | 210, 260 | `""` | `$shadowing=` on `AddElementTag`/`AddRelTag` |

These come from `ElementStyle.getProperties()`/`RelationshipStyle` — i.e. the
theme/workspace style `properties` map after resolution (P2-D2), NOT view or
configuration properties.

## 4. Model and element properties (not the cascade)

- `structurizr.groupSeparator` (`Model.GROUP_SEPARATOR_PROPERTY_NAME`,
  structurizr-core model/Model.java:18) — read DIRECTLY from
  `view.getModel().getProperties()`, no default, unset/empty = flat groups:
  AbstractDiagramExporter 516 (`findGroups`: nested ancestor groups added), 544
  (`writeElements`: nesting depth via `split(quote(sep))`), StructurizrPlantUMLExporter
  113 (group rename `a/b` → `a / b` style). Group order: `HashSet` then
  `Collections.sort` → lexicographic; elements sorted by `getId()` first.
- C4 property blocks (C4PlantUMLExporter `addProperties` 607–620): keys
  starting `structurizr.` are FILTERED OUT (internal props like
  `structurizr.dsl.identifier`), remaining keys **sorted**, wrapped in
  `WithoutPropertyHeader()` / `WithPropertyHeader(...)`, rendered
  `ShowProperty()`-style per macro. Gated by §2 `…Properties` toggles.
- `breakText(maxWidth, fontSize, s)` (AbstractExporter 8–44) is pure
  geometry (`characterWidth = fontSize * 0.6`), reads no property.

## 5. View iteration + support matrix (P2-V2 evidence)

`AbstractDiagramExporter.export(Workspace)` (32–89) is `final` and shared by
every `DiagramExporter`: **custom → landscape → context → container →
component → dynamic → deployment**, `null` diagrams skipped. **`FilteredView`
is never iterated** — filtered views are simply not exported. Per-kind output
runs through the dialect's template methods (`writeHeader`/`writeElement`/
`createDiagram`/boundaries).

Overrides at the pin: C4PlantUMLExporter overrides `export(CustomView)` (423,
C4 macros for custom elements); AbstractPlantUMLExporter overrides
`export(DynamicView)` (275, sequence path); MermaidDiagramExporter overrides
`export(DynamicView)` (173). WebSequenceDiagramsExporter overrides all six
sequence-oriented kinds and `export(DeploymentView)` **returns null** (72–74 —
deployment silently skipped); its CustomView support rides the base template.

`export(ModelView)` (91–110) is a dispatch used elsewhere: the seven known
kinds, anything else `throw new RuntimeException(<class FQN> + " is not
supported")` — unreachable via the CLI loop (it never hands a FilteredView over).

## 6. Theme loading chain (P2-V4 evidence)

### 6.1 Reference behavior (all at the pin)

1. **Install** (name → file registry, process-global
   `InstalledThemes.THEMES: Map<String, File>` — structurizr-client
   view/InstalledThemes.java): `AbstractCommand.installThemes`
   (structurizr-application command/AbstractCommand.java 118–134) probes
   `<workspace-parent>/themes` (or `./themes` for the no-file flows); if absent
   falls back to env `STRUCTURIZR_THEMES`. The dir is scanned one level: every
   subdirectory containing `theme.json` registers as name = subdirectory name
   (`ThemeUtils.installThemes`, structurizr-client view/ThemeUtils.java 138–158).
2. **Parse** (DSL `theme`/`themes`, structurizr-dsl ThemeParser.java):
   `default` (case-insensitive) → `https://static.structurizr.com/themes/default/theme.json`;
   installed NAME → stored verbatim in `configuration.themes`; URL → stored
   (dedup); anything else = FILE form: `FILE_SYSTEM` feature, `dslPortable=false`,
   resolved next to the DSL file, **inlined** via
   `ThemeUtils.inlineAllStylesFromTheme` (P4/D11 for oxidizr). JSON load path:
   `Configuration.addTheme` (structurizr-core view/Configuration.java 89–110)
   trims; installed → add; URL → dedup-add; otherwise **`log.warn "Unknown
   theme: X"` — a warning, not an error** (plus a Structurizr-Cloud EOL warn
   for themes.structurizr.com URLs).
3. **Load** (content → `Styles.themes`, only in the diagram branch of
   `ExportCommand` — see P2-D4): for each `configuration.themes` entry
   (ThemeUtils.loadThemes 101–134): URL → HTTP fetch (client allows all hosts);
   content-type must start with json or text/plain, else RuntimeException
   naming expected/actual; relative icons rewritten to `baseUrl + icon`
   (absolute http(s) and `data:` pass through); then
   `Styles.addStylesFromTheme(theme)` = append to a `List<Theme> themes` —
   **never serialized, never merged into `elements`/`relationships`**;
   `findElementStyle(tag, scheme)` consults theme styles first (145–175).
   Installed name → read `<dir>/<name>/theme.json` and append (icons NOT
   rewritten — asymmetry vs the URL branch). Anything else → **silently
   skipped**.

### 6.2 oxidizr `ThemeStore` mapping (M1-c/M1-d)

- Registry sources, first hit wins (reference probes one dir only): `--themes-dir`
  flag → `OXIDZR_THEMES_DIR` env → `<workspace-parent>/themes` (reference
  default) → `STRUCTURIZR_THEMES` env (drop-in fallback). Our-namespace sources
  are a documented D5-adjacent deviation; the reference chain is preserved.
- Scan rule identical to §6.1-1 (one level, `theme.json` presence).
- `Styles.themes` gets a `Vec<Theme>` in oxidizr-core (P2-D2 resolve reads it;
  NOT serialized — same non-contract as reference).
- URL branch honors D5: fetch **only** with `--allow-net`; without it, OXZ-5001
  naming the URL and the `--themes-dir` escape hatch. Transport = delegate to
  system `curl` via `std::process::Command` (std-only rule: no TLS in std;
  absence of `curl` = error, not panic). Local cache + `--themes-dir` remain
  the offline path (P2-D3 items 1–2; cache key hash hand-rolled FIPS 180-4).
- Installed-name matching also gates the DSL parser's `theme`/`themes`
  statements (`add_theme` currently treats every non-URL as FILE form —
  M1-d wires the registry check first, mirroring ThemeParser order).

## 7. Resolution engine constants (feeds P2-D2, part of P2-V5)

`Styles.java` (structurizr-core view/Styles.java 11–15): `DEFAULT_BACKGROUND_LIGHT
= #ffffff`, `DEFAULT_COLOR_LIGHT = #444444`, `DEFAULT_BACKGROUND_DARK =
#111111`, `DEFAULT_COLOR_DARK = #cccccc`. Element default (273): shape `Box`,
fontSize 24, border `Solid`, opacity 100, metadata+description `true`, tag
`Element`. Relationship default (351): thickness 2, style `Dashed`, dashed
`true`, routing `Direct`, fontSize 24, width 200, position 50, opacity 100, tag
`Relationship`. Unset stroke when background set = `Color.decode(bg).darker()`
formatted `#%06X` (317–321). Dialect-level color constants (skinparam/`$`
defaults per dialect) remain with the M2/M3 ports (P2-V5).
