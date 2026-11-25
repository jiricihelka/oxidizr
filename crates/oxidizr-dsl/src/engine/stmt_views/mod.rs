//! Chain section: the official parser arms at official lines
//! 1029–1196 — the nine view definitions (`custom`, `systemLandscape`,
//! `systemContext`, `container`, `component`, `dynamic`, `deployment`,
//! `filtered`, `image`), the `parallel` (`{`) arm inside dynamic views, the
//! custom/static/deployment view-content (`include`, `exclude`,
//! `animationStep`, `animation`) and their animation-context catch-alls,
//! `autoLayout`, `default`, `title`, `description`, the image-view content
//! (`plantuml`, `mermaid`, `kroki`, `image`) with `light`/`dark`, the
//! dynamic-view relationship catch-all and `url`, `theme`/`themes`, and
//! `terminology` with its seven term arms plus `metadata`.
//!
//! Faithful-port quirks: view definitions and
//! `animation`/`terminology` push their context UNCONDITIONALLY (the official parser
//! chain has no `shouldStartContext` guard there — a view statement without
//! `{` still swallows following lines until the next `}`), only `filtered`,
//! `light`/`dark` and the dynamic-relationship block guard on it; the
//! dynamic catch-all receives the RAW tokens (trailing `{` included, so
//! `a -> b {` stores `{` as the step description — verified);
//! sequence numbers are burned on every successful add even when an
//! explicit `[order:]` overrides them; `InstalledThemes` is empty in
//! oxidizr so a `theme` name that is neither a URL nor `default` takes the
//! file branch; URL/file `theme`, `plantuml`, `mermaid`, `kroki` and
//! raster/`plantuml <file>` content import are not supported yet —
//! those paths parse and fail with the deferred/import codes; `image
//! <file>` with an SVG inlines the plain data URI regardless of the color
//! scheme (the official parser `ImageImporter` ignores the scheme for files).

use std::borrow::Cow;

use oxidizr_core::url::{is_http_url, is_https_url, is_url};
use oxidizr_core::validate::ElementKind;
use oxidizr_core::views::{AutomaticLayout, MetadataSymbols, RankDirection, Terminology};
use oxidizr_core::{Id, ViewRef};

use crate::compat::{java_parse_int, java_trim};

use super::expr::{ExprScope, Mi, is_expression};
use super::idents::RegItem;
use super::stmt_line::{java_absolute, java_canonical};
use super::{ColorScheme, Ctx, DslError, Parser, Sec, Stmt, Tokens, tok};
mod content;
mod defs;

use defs::{
    component_view, container_view, context_view, custom_view, deployment_view, dynamic_view,
    filtered_view, image_view_def, landscape_view,
};

use content::{
    custom_exclude, custom_include, deployment_exclude, deployment_include, static_exclude,
    static_include,
};

/// The blank check (trims).
fn is_trim_empty(s: &str) -> bool {
    java_trim(s).is_empty()
}

fn too_many(grammar: &str) -> DslError {
    DslError::stmt(format!("Too many tokens, expected: {grammar}"))
}

fn expected(grammar: &str) -> DslError {
    DslError::stmt(format!("Expected: {grammar}"))
}

fn deferred(message: &str) -> DslError {
    DslError::coded(oxidizr_diag::codes::IMPORT_DEFERRED, message)
}

fn net_https() -> DslError {
    DslError::coded(
        oxidizr_diag::codes::NET_DISABLED,
        "Image views via HTTPS are not permitted",
    )
}

fn net_http() -> DslError {
    DslError::coded(
        oxidizr_diag::codes::NET_DISABLED,
        "Image views via HTTP are not permitted",
    )
}

/// the abstract-view parser's validate-view-key check.
fn validate_view_key(key: Cow<'_, str>) -> Result<String, DslError> {
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if valid {
        Ok(key.into_owned())
    } else {
        Err(DslError::stmt(
            "View keys can only contain the following characters: a-zA-Z0-9_-",
        ))
    }
}

/// `View.setDescription` / `setTitle` applied post-create: the official parser
/// parsers call the setter only when the token is present (empty quoted
/// tokens never reach it), and the writer's `NON_EMPTY` omission hides any
/// residual difference — so an empty value is not stored. `custom` views are
/// the exception (the `CustomView` constructor stores "" verbatim) and do
/// not use this helper.
fn apply_text(p: &mut Parser, view: ViewRef, text: String, title: bool) {
    if !text.is_empty()
        && let Some(base) = p.builder.view_base_mut(view)
    {
        if title {
            base.title = Some(text);
        } else {
            base.description = Some(text);
        }
    }
}

// ---------------------------------------------------------------------------
// animation steps
// ---------------------------------------------------------------------------

fn static_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut ids: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Static)?
        } else {
            p.parse_identifier(&token, ExprScope::Static)?
        };
        for item in items {
            if let Mi::Element(id) = item
                && matches!(
                    p.builder.element_kind(&id),
                    Some(
                        ElementKind::Person
                            | ElementKind::SoftwareSystem
                            | ElementKind::Container
                            | ElementKind::Component
                    )
                )
            {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return Err(DslError::stmt("No elements were found"));
    }
    p.builder
        .view_add_animation_step(view, &ids)
        .map_err(|e| DslError::core(&e))
}

fn deployment_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut instances: Vec<Id> = Vec::new();
    let mut infra: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Deployment)?
        } else {
            p.parse_identifier(&token, ExprScope::Deployment)?
        };
        for item in items {
            if let Mi::Element(id) = item {
                match p.builder.element_kind(&id) {
                    Some(ElementKind::SoftwareSystemInstance | ElementKind::ContainerInstance) => {
                        instances.push(id);
                    }
                    Some(ElementKind::InfrastructureNode) => infra.push(id),
                    _ => {}
                }
            }
        }
    }
    if instances.is_empty() && infra.is_empty() {
        return Err(DslError::stmt(
            "No software system instances, container instances, or infrastructure nodes were found",
        ));
    }
    // `DeploymentView.addAnimation(instances, infrastructureNodes)` —
    // The animation add takes
    instances.extend(infra);
    p.builder
        .view_add_animation_step(view, &instances)
        .map_err(|e| DslError::core(&e))
}

fn custom_animation_step(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
    start: usize,
) -> Result<(), DslError> {
    let mut ids: Vec<Id> = Vec::new();
    for i in start..tokens.size() {
        let token = tokens.get(i);
        let items = if is_expression(&token) {
            p.parse_expression(&token, ExprScope::Custom)?
        } else {
            p.parse_identifier(&token, ExprScope::Custom)?
        };
        for item in items {
            if let Mi::Element(id) = item
                && p.builder.element_kind(&id) == Some(ElementKind::CustomElement)
            {
                ids.push(id);
            }
        }
    }
    if ids.is_empty() {
        return Err(DslError::stmt("No custom elements were found"));
    }
    p.builder
        .view_add_animation_step(view, &ids)
        .map_err(|e| DslError::core(&e))
}

// ---------------------------------------------------------------------------
// autoLayout / default / title / description
// ---------------------------------------------------------------------------

/// the auto-layout parser over the RAW tokens (the trailing `{` is parsed as a
/// rank direction — official behavior), after the `model-view context`
/// cast the driver arm performs via the top-context fetch.
fn auto_layout(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str =
        "autoLayout [rankDirection] [rankSeparation] [nodeSeparation] [edgeSeparation] [vertices]";
    p.ctx_cast("ModelViewDslContext", Ctx::is_model_view)?;
    if tokens.has_more_than(5) {
        return Err(too_many(G));
    }
    let Some(view) = p.top().and_then(Ctx::view) else {
        return Ok(());
    };
    let mut rank_direction = RankDirection::LeftRight;
    let mut rank_separation = 100;
    let mut node_separation = 50;
    let mut edge_separation = 50;
    let mut vertices = true;
    if tokens.includes(1) {
        let rd = tokens.get(1);
        // `HashMap.containsKey` — case-SENSITIVE keys.
        rank_direction = match &*rd {
            "tb" => RankDirection::TopBottom,
            "bt" => RankDirection::BottomTop,
            "lr" => RankDirection::LeftRight,
            "rl" => RankDirection::RightLeft,
            _ => return Err(DslError::stmt("Valid rank directions are: tb|bt|lr|rl")),
        };
    }
    if tokens.includes(2) {
        rank_separation = java_parse_int(&tokens.get(2))
            .ok_or_else(|| DslError::stmt("Rank separation must be positive integer in pixels"))?;
    }
    if tokens.includes(3) {
        node_separation = java_parse_int(&tokens.get(3))
            .ok_or_else(|| DslError::stmt("Node separation must be positive integer in pixels"))?;
    }
    if tokens.includes(4) {
        edge_separation = java_parse_int(&tokens.get(4))
            .ok_or_else(|| DslError::stmt("Edge separation must be positive integer in pixels"))?;
    }
    if tokens.includes(5) {
        let v = tokens.get(5);
        vertices = if super::eqi(&v, "true") {
            true
        } else if super::eqi(&v, "false") {
            false
        } else {
            return Err(DslError::stmt("Vertices must be true or false"));
        };
    }
    if let Some(content) = p.builder.content_mut(view) {
        content.automatic_layout = Some(AutomaticLayout {
            rank_direction: Some(rank_direction),
            rank_separation,
            node_separation,
            edge_separation,
            vertices,
        });
    }
    Ok(())
}

/// the view parser's parse-title check / `parseDescription` over RAW tokens.
fn view_text(p: &mut Parser, tokens: &Tokens, title: bool) -> Result<(), DslError> {
    let grammar = if title {
        "title <title>"
    } else {
        "description <description>"
    };
    if tokens.has_more_than(1) {
        return Err(too_many(grammar));
    }
    let Some(view) = p.top().and_then(Ctx::view) else {
        return Ok(());
    };
    if tokens.size() == 2 {
        let value = tokens.get(1);
        // `setTitle`/`setDescription` store verbatim; empty text is
        // omitted by the writer, so an explicit `""` stores nothing.
        apply_text(p, view, value.into_owned(), title);
        Ok(())
    } else {
        Err(expected(grammar))
    }
}

// ---------------------------------------------------------------------------
// image view content
// ---------------------------------------------------------------------------

/// Shared tail of `plantuml`/`mermaid`: URL branches are refused, a
/// file that exists inlines via a official importer — deferred to later.
fn diagram_file_or_url(p: &mut Parser, source: &str, keyword: &str) -> Result<(), DslError> {
    if is_https_url(source) {
        return Err(net_https());
    }
    if is_http_url(source) {
        return Err(net_http());
    }
    let Some(dir) = p.current_dir() else {
        return Err(DslError::stmt("NullPointerException"));
    };
    let file = std::path::Path::new(&dir).join(source);
    if !file.exists() {
        return Err(DslError::stmt(format!(
            "The file at {} does not exist",
            java_absolute(&file)
        )));
    }
    // `context.portability off` — after the existence check.
    if let Some(frame) = p.frame_portable.last_mut() {
        *frame = false;
    }
    Err(deferred(&format!(
        "{keyword} <file> import is not supported yet"
    )))
}

fn diagram_view_key(p: &Parser, source: &str) -> bool {
    // the view-by-key model-view check — filtered and image
    // views fall through to the URL/file branch.
    p.builder
        .view_ref_of_key(source)
        .is_some_and(|v| !matches!(v, ViewRef::Filtered(_) | ViewRef::Image(_)))
}

/// the image-view-content parser's parse-plant-u-m-l check / `parseMermaid`.
fn plantuml_or_mermaid(p: &mut Parser, tokens: &Tokens, keyword: &str) -> Result<(), DslError> {
    let grammar = format!("{keyword} <source|file|url|viewKey>");
    if !tokens.includes(1) {
        return Err(expected(&grammar));
    }
    if tokens.has_more_than(1) {
        return Err(too_many(&grammar));
    }
    let source = tokens.get(1);
    if source.contains('\n') {
        return Err(deferred(&format!(
            "{keyword} <source> import is not supported yet"
        )));
    }
    if diagram_view_key(p, &source) {
        return Err(deferred(&format!(
            "{keyword} <viewKey> import is not supported yet"
        )));
    }
    diagram_file_or_url(p, &source, keyword)
}

/// the image-view-content parser's parse-kroki check (no viewKey branch).
fn kroki(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str = "kroki <format> <source|file|url>";
    if !tokens.includes(2) {
        return Err(expected(G));
    }
    if tokens.has_more_than(2) {
        return Err(too_many(G));
    }
    let source = tokens.get(2);
    if source.contains('\n') {
        return Err(deferred("kroki <source> import is not supported yet"));
    }
    diagram_file_or_url(p, &source, "kroki")
}

/// the image-view-content parser's parse-image check. An SVG inlines as the PLAIN
/// content data URI regardless of the color scheme (the official parser
/// `ImageImporter.importDiagram(view, file,...)` ignores the scheme);
/// raster formats need the official image codec (the same deviation family); other files get
/// the `ImageUtils` texts.
// False positive: the name is explicitly lower-cased below (mirrors the
// the official lowercase suffix check).
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn image_content(p: &mut Parser, view: ViewRef, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str = "image <file|url>";
    if !tokens.includes(1) {
        return Err(expected(G));
    }
    if tokens.has_more_than(1) {
        return Err(too_many(G));
    }
    let source = tokens.get(1);
    if is_https_url(&source) {
        return Err(net_https());
    }
    if is_http_url(&source) {
        return Err(net_http());
    }
    let Some(dir) = p.current_dir() else {
        return Err(DslError::stmt("NullPointerException"));
    };
    let file = std::path::Path::new(&dir).join(&*source);
    if !file.exists() {
        return Err(DslError::stmt(format!(
            "The file at {} does not exist",
            java_absolute(&file)
        )));
    }
    // portability off — before the ImageUtils checks.
    if let Some(frame) = p.frame_portable.last_mut() {
        *frame = false;
    }
    let canonical = java_canonical(&file);
    if !file.is_file() {
        // `ImageUtils.getContentType` — trailing period.
        return Err(DslError::stmt(format!("{canonical} is not a file.")));
    }
    let name = file
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let lower = name.to_lowercase();
    if !lower.ends_with(".svg") {
        if [".png", ".jpg", ".jpeg", ".gif"]
            .iter()
            .any(|e| lower.ends_with(e))
        {
            return Err(DslError::coded(
                oxidizr_diag::codes::JVM_ONLY,
                format!(
                    "image {source} is not supported by oxidizr (JVM image codec): use an SVG file"
                ),
            ));
        }
        return Err(DslError::stmt(format!(
            "{canonical} is not a supported image file."
        )));
    }
    let bytes = std::fs::read(&file)
        .map_err(|e| DslError::stmt(format!("Error reading {}: {e}", java_absolute(&file))))?;
    let data_uri = format!("data:image/svg+xml;base64,{}", super::base64_std(&bytes));
    let ViewRef::Image(i) = view else {
        return Ok(());
    };
    if let Some(iv) = p.builder.views_mut().image_views.get_mut(i) {
        // `ImageImporter` sets the PLAIN content (color scheme ignored),
        // the content type and the title.
        iv.content = Some(data_uri);
        iv.content_type = Some("image/svg+xml".to_string());
        iv.base.title = Some(name);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// dynamic view content
// ---------------------------------------------------------------------------

fn dynamic_step(p: &mut Parser, raw: &Tokens, view: ViewRef, open: bool) -> Result<(), DslError> {
    const G1: &str = "[order:] <identifier> -> <identifier> [description] [technology]";
    const G2: &str = "[order:] <identifier> [description]";
    // the official parser mutates the driver's token list; the port's answer is a borrowed
    // view with a drop-prefix offset instead of the old deep clone (2R
    // the trailing `{` stays, and `open` was computed from it
    // before).
    let mut rest = raw.raw_tokens();
    let mut order = String::new();
    if let Some(first) = rest.first() {
        let first = Tokens::unescape(first);
        if let Some(stripped) = first.strip_suffix(':') {
            order = stripped.to_string();
            rest = &rest[1..];
        }
    }
    let tokens = Tokens::borrowed(rest);
    let rel_id = if tokens.size() > 1 && tokens.eq_at(1, tok::RELATIONSHIP) {
        // Pair form. `RELATIONSHIP_TOKEN.equals` — case-sensitive `->`.
        if tokens.has_more_than(4) {
            return Err(too_many(G1));
        }
        if !tokens.includes(2) {
            return Err(expected(G1));
        }
        let source_id = tokens.get(0);
        let destination_id = tokens.get(2);
        let source = dynamic_element(p, &source_id, true)?;
        let destination = dynamic_element(p, &destination_id, false)?;
        let description = if tokens.includes(3) {
            tokens.get(3).into_owned()
        } else {
            String::new()
        };
        let technology = if tokens.includes(4) {
            tokens.get(4).into_owned()
        } else {
            String::new()
        };
        let sequence = p.seq_peek(view);
        match p.builder.dynamic_add_element_pair(
            view,
            &source,
            &description,
            &technology,
            &destination,
            &sequence,
        ) {
            Ok(id) => {
                p.seq_commit(view);
                id
            }
            Err(e) => return Err(DslError::core(&e)),
        }
    } else {
        // Relationship-identifier form. token `0` on the stripped
        // list — the official index-out-of-bounds text for a line that is only `<order>:`.
        if tokens.is_empty() {
            return Err(DslError::stmt("Index 0 out of bounds for length 0"));
        }
        let relationship_id = tokens.get(0);
        let relationship = p.get_relationship(&relationship_id);
        if tokens.has_more_than(1) {
            return Err(too_many(G2));
        }
        let Some(rel) = relationship else {
            return Err(DslError::stmt(format!(
                "The relationship \"{relationship_id}\" does not exist"
            )));
        };
        let description = if tokens.includes(1) {
            tokens.get(1).into_owned()
        } else {
            String::new()
        };
        let sequence = p.seq_peek(view);
        match p
            .builder
            .dynamic_add_relationship(view, &rel, &description, &sequence)
        {
            Ok(id) => {
                p.seq_commit(view);
                id
            }
            Err(e) => return Err(DslError::core(&e)),
        }
    };
    // The order is set only when non-blank — the
    // sequence number was burned either way.
    if !is_trim_empty(&order)
        && let Some(rv) = p.relationship_view_mut(view, &rel_id)
    {
        rv.order = Some(order);
    }
    // `hasRelationships(true)` on the enclosing parallel sequence (if any).
    if let Some(Ctx::DynamicParallel {
        has_relationships, ..
    }) = p.stack.last_mut()
    {
        *has_relationships = true;
    }
    if open {
        p.start_context(Ctx::DynamicRel { view, rel: rel_id });
    }
    Ok(())
}

/// `getElement` + the static-structure-or-custom gate. A pseudo-element
/// resolves (non-null) but fails the gate — the `should be` text.
fn dynamic_element(p: &Parser, identifier: &str, source: bool) -> Result<Id, DslError> {
    let which = if source { "source" } else { "destination" };
    match p.get_element(identifier) {
        None => Err(DslError::stmt(format!(
            "The {which} element \"{identifier}\" does not exist"
        ))),
        Some(RegItem::Element(id))
            if matches!(
                p.builder.element_kind(&id),
                Some(
                    ElementKind::Person
                        | ElementKind::SoftwareSystem
                        | ElementKind::Container
                        | ElementKind::Component
                        | ElementKind::CustomElement
                )
            ) =>
        {
            Ok(id)
        }
        Some(_) => Err(DslError::stmt(format!(
            "The {which} element \"{identifier}\" should be a static structure or custom element"
        ))),
    }
}

// ---------------------------------------------------------------------------
// theme / themes
// ---------------------------------------------------------------------------

const DEFAULT_THEME_URL: &str = "https://static.structurizr.com/themes/default/theme.json";

/// the theme parser's add-theme check — branch order: `default` → URL mapping, then the
/// installed-name branch (`InstalledThemes`, registry injected by the
/// pipeline pre-parse, verified), then URL, then the FILE form. The file branch
/// inlines styles via `ThemeUtils` — later. Without an injected
/// registry the installed branch is inert (fragment parses, library use).
fn add_theme(p: &mut Parser, theme_token: String) -> Result<(), DslError> {
    let mut theme = theme_token;
    if super::eqi(&theme, "default") {
        theme = DEFAULT_THEME_URL.to_string();
    }
    if let Some(registry) = &p.opts.installed_themes
        && registry.is_installed(&theme)
    {
        // `Configuration.addTheme`: trim, then `themes.add` — the installed
        // branch appends WITHOUT dedup (only the URL branch deduplicates).
        p.builder
            .views_mut()
            .configuration
            .themes
            .push(java_trim(&theme).to_string());
        return Ok(());
    }
    if is_url(&theme) {
        // `Configuration.addTheme`: trim, then dedup on the trimmed value.
        let theme = java_trim(&theme).to_string();
        if is_trim_empty(&theme) {
            return Ok(());
        }
        let themes = &mut p.builder.views_mut().configuration.themes;
        if !themes.contains(&theme) {
            themes.push(theme);
        }
        return Ok(());
    }
    // FILE_SYSTEM branch — portability off BEFORE the exists check.
    if let Some(frame) = p.frame_portable.last_mut() {
        *frame = false;
    }
    // the file parent - a fragment parse crashes in the official parser
    // (resolve_icon precedent).
    let Some(dir) = p.current_dir() else {
        return Err(DslError::stmt("NullPointerException"));
    };
    let file = std::path::Path::new(&dir).join(&theme);
    if !file.exists() {
        return Err(DslError::stmt(format!(
            "Theme {theme} does not exist at {}",
            java_absolute(&file)
        )));
    }
    if !file.is_file() {
        return Err(DslError::stmt(format!(
            "{} is not a file",
            java_absolute(&file)
        )));
    }
    Err(deferred("theme <file> inlining is not supported yet"))
}

fn theme(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str = "theme <name|url|file>";
    if tokens.has_more_than(1) {
        return Err(too_many(G));
    }
    if !tokens.includes(1) {
        return Err(expected(G));
    }
    add_theme(p, tokens.get(1).into_owned())
}

fn themes(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str = "themes <name|url|file> [name|url|file] ... [name|url|file]";
    if !tokens.includes(1) {
        return Err(expected(G));
    }
    for i in 1..tokens.size() {
        add_theme(p, tokens.get(i).into_owned())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// terminology
// ---------------------------------------------------------------------------

/// the terminology parser term arms — Expected check, then set the field.
fn term(
    p: &mut Parser,
    tokens: &Tokens,
    keyword: &str,
    set: impl FnOnce(&mut Terminology, String),
) -> Result<(), DslError> {
    let grammar = format!("{keyword} <term>");
    if !tokens.includes(1) {
        return Err(expected(&grammar));
    }
    set(
        &mut p.builder.views_mut().configuration.terminology,
        tokens.get(1).into_owned(),
    );
    Ok(())
}

/// the terminology parser's parse-metadata-symbols check — insertion order in the
/// error grammar; the value is lowercased BEFORE the lookup.
fn metadata_symbols(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const MAP: [(&str, MetadataSymbols); 6] = [
        ("square", MetadataSymbols::SquareBrackets),
        ("round", MetadataSymbols::RoundBrackets),
        ("curly", MetadataSymbols::CurlyBrackets),
        ("angle", MetadataSymbols::AngleBrackets),
        ("double-angle", MetadataSymbols::DoubleAngleBrackets),
        ("none", MetadataSymbols::None),
    ];
    let grammar = format!(
        "metadata <{}>",
        MAP.iter().map(|(k, _)| *k).collect::<Vec<_>>().join("|")
    );
    if !tokens.includes(1) {
        return Err(expected(&grammar));
    }
    let value = tokens.get(1).to_lowercase();
    let symbol = MAP
        .iter()
        .find(|(k, _)| *k == value)
        .map(|(_, v)| *v)
        .ok_or_else(|| DslError::stmt(format!("The symbol type \"{value}\" is not valid")))?;
    p.builder.views_mut().configuration.metadata_symbols = Some(symbol);
    Ok(())
}

// ---------------------------------------------------------------------------
// the section
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_lines)] // the chain arm order IS the contract
pub(crate) fn section(p: &mut Parser, stmt: &Stmt) -> Sec {
    let first = stmt.first();
    let raw = &stmt.tokens;
    let t = raw.without_context_start_token();
    let open = Parser::should_start_context(raw);
    let in_views = matches!(p.top(), Some(Ctx::Views));
    let views_or_styles = matches!(p.top(), Some(Ctx::Views | Ctx::Styles { .. }));

    // -- view definitions (startContext is UNCONDITIONAL except filtered) --
    macro_rules! view_def {
        ($tok:expr, $parse:expr, $ctx:expr, $push:expr) => {
            if super::eqi(&first, $tok) && in_views {
                let result = $parse;
                return view_arm(p, result, $ctx, $push);
            }
        };
    }
    view_def!(
        tok::VIEW_CUSTOM,
        custom_view(p, &t),
        |v| Ctx::CustomView { view: v },
        true
    );
    view_def!(
        tok::VIEW_LANDSCAPE,
        landscape_view(p, &t),
        |v| Ctx::LandscapeView { view: v },
        true
    );
    view_def!(
        tok::VIEW_CONTEXT,
        context_view(p, &t),
        |v| Ctx::ContextView { view: v },
        true
    );
    view_def!(
        tok::VIEW_CONTAINER,
        container_view(p, &t),
        |v| Ctx::ContainerView { view: v },
        true
    );
    view_def!(
        tok::VIEW_COMPONENT,
        component_view(p, &t),
        |v| Ctx::ComponentView { view: v },
        true
    );
    view_def!(
        tok::VIEW_DYNAMIC,
        dynamic_view(p, &t),
        |v| Ctx::DynamicView { view: v },
        true
    );
    view_def!(
        tok::VIEW_DEPLOYMENT,
        deployment_view(p, &t),
        |v| Ctx::DeploymentView { view: v },
        true
    );
    view_def!(
        tok::VIEW_FILTERED,
        filtered_view(p, &t),
        |v| Ctx::FilteredView { view: v },
        open
    );
    view_def!(
        tok::VIEW_IMAGE,
        image_view_def(p, &t),
        |v| Ctx::ImageView {
            view: v,
            color_scheme: None,
        },
        true
    );

    // -- `{` opening a parallel sequence inside a dynamic view -----------
    if super::eqi(&first, tok::CONTEXT_START)
        && matches!(
            p.top(),
            Some(Ctx::DynamicView { .. } | Ctx::DynamicParallel { .. })
        )
    {
        let view = p.top().and_then(Ctx::view).unwrap_or(ViewRef::Dynamic(0));
        p.seq_start_parallel(view);
        p.start_context(Ctx::DynamicParallel {
            view,
            has_relationships: false,
        });
        return Sec::Handled;
    }

    // -- custom view content ---------------------------------------------
    if super::eqi(&first, tok::VIEW_INCLUDE) {
        if let Some(Ctx::CustomView { view }) = p.top() {
            let view = *view;
            return Sec::from(custom_include(p, view, raw));
        }
        if let Some(Ctx::DeploymentView { view }) = p.top() {
            let view = *view;
            return Sec::from(deployment_include(p, view, raw));
        }
        if p.top().is_some_and(super::ctx::Ctx::is_static_view) {
            let view = p.top().and_then(Ctx::view).expect("static view");
            return Sec::from(static_include(p, view, raw));
        }
    }
    if super::eqi(&first, tok::VIEW_EXCLUDE) {
        if let Some(Ctx::CustomView { view }) = p.top() {
            let view = *view;
            return Sec::from(custom_exclude(p, view, raw));
        }
        if let Some(Ctx::DeploymentView { view }) = p.top() {
            let view = *view;
            return Sec::from(deployment_exclude(p, view, raw));
        }
        if p.top().is_some_and(super::ctx::Ctx::is_static_view) {
            let view = p.top().and_then(Ctx::view).expect("static view");
            return Sec::from(static_exclude(p, view, raw));
        }
    }
    if super::eqi(&first, tok::VIEW_ANIMATION_STEP) {
        if let Some(Ctx::CustomView { view }) = p.top() {
            let view = *view;
            if !raw.includes(1) {
                return Sec::Err(expected(
                    "animationStep <identifier|element expression> [identifier|element expression...]",
                ));
            }
            return Sec::from(custom_animation_step(p, view, raw, 1));
        }
        if p.top().is_some_and(super::ctx::Ctx::is_static_view) {
            let view = p.top().and_then(Ctx::view).expect("static view");
            if !raw.includes(1) {
                return Sec::Err(expected(
                    "animationStep <identifier|element expression> [identifier|element expression...]",
                ));
            }
            return Sec::from(static_animation_step(p, view, raw, 1));
        }
        if let Some(Ctx::DeploymentView { view }) = p.top() {
            let view = *view;
            if !raw.includes(1) {
                return Sec::Err(expected(
                    "animationStep <identifier|element expression> [identifier|element expression...]",
                ));
            }
            return Sec::from(deployment_animation_step(p, view, raw, 1));
        }
    }
    if super::eqi(&first, tok::VIEW_ANIMATION) {
        if let Some(Ctx::CustomView { view }) = p.top() {
            let view = *view;
            p.start_context(Ctx::CustomViewAnimation { view });
            return Sec::Handled;
        }
        if p.top().is_some_and(super::ctx::Ctx::is_static_view) {
            let view = p.top().and_then(Ctx::view).expect("static view");
            p.start_context(Ctx::StaticViewAnimation { view });
            return Sec::Handled;
        }
        if let Some(Ctx::DeploymentView { view }) = p.top() {
            let view = *view;
            p.start_context(Ctx::DeploymentViewAnimation { view });
            return Sec::Handled;
        }
    }
    // animation-context catch-alls (match ANY token; the chain places each
    // right after its view family's arms, before the next family).
    if let Some(Ctx::CustomViewAnimation { view }) = p.top() {
        let view = *view;
        if !raw.includes(0) {
            return Sec::Err(expected(
                "<identifier|element expression> [identifier|element expression...]",
            ));
        }
        return Sec::from(custom_animation_step(p, view, raw, 0));
    }
    if let Some(Ctx::StaticViewAnimation { view }) = p.top() {
        let view = *view;
        if !raw.includes(0) {
            return Sec::Err(expected(
                "<identifier|element expression> [identifier|element expression...]",
            ));
        }
        return Sec::from(static_animation_step(p, view, raw, 0));
    }
    if let Some(Ctx::DeploymentViewAnimation { view }) = p.top() {
        let view = *view;
        if !raw.includes(0) {
            return Sec::Err(expected(
                "<identifier|element expression> [identifier|element expression...]",
            ));
        }
        return Sec::from(deployment_animation_step(p, view, raw, 0));
    }

    // -- autoLayout / default / title / description ----------------------
    if super::eqi(&first, tok::VIEW_AUTOLAYOUT) && p.top().is_some_and(Ctx::is_view) {
        return Sec::from(auto_layout(p, raw));
    }
    if super::eqi(&first, tok::VIEW_DEFAULT) && p.top().is_some_and(Ctx::is_view) {
        if let Some(view) = p.top().and_then(Ctx::view) {
            let key = p.builder.view_key(view);
            p.builder.views_mut().configuration.default_view = Some(key);
        }
        return Sec::Handled;
    }
    if super::eqi(&first, tok::VIEW_TITLE) && p.top().is_some_and(Ctx::is_view) {
        return Sec::from(view_text(p, raw, true));
    }
    if super::eqi(&first, tok::VIEW_DESCRIPTION) && p.top().is_some_and(Ctx::is_view) {
        return Sec::from(view_text(p, raw, false));
    }

    // -- image view content ----------------------------------------------
    if super::eqi(&first, tok::PLANTUML) && matches!(p.top(), Some(Ctx::ImageView { .. })) {
        return Sec::from(plantuml_or_mermaid(p, raw, "plantuml"));
    }
    if super::eqi(&first, tok::MERMAID) && matches!(p.top(), Some(Ctx::ImageView { .. })) {
        return Sec::from(plantuml_or_mermaid(p, raw, "mermaid"));
    }
    if super::eqi(&first, tok::KROKI) && matches!(p.top(), Some(Ctx::ImageView { .. })) {
        return Sec::from(kroki(p, raw));
    }
    if super::eqi(&first, tok::IMAGE)
        && let Some(Ctx::ImageView { view, .. }) = p.top()
    {
        let view = *view;
        return Sec::from(image_content(p, view, raw));
    }
    if super::eqi(&first, tok::LIGHT)
        && let Some(Ctx::ImageView { view, .. }) = p.top()
    {
        let view = *view;
        if open {
            retheme(p, view, Some(ColorScheme::Light));
            return Sec::Handled;
        }
        return Sec::No;
    }
    if super::eqi(&first, tok::DARK)
        && let Some(Ctx::ImageView { view, .. }) = p.top()
    {
        let view = *view;
        if open {
            retheme(p, view, Some(ColorScheme::Dark));
            return Sec::Handled;
        }
        return Sec::No;
    }

    // -- dynamic relationship statements ----------------------------------
    if let Some(Ctx::DynamicView { view } | Ctx::DynamicParallel { view, .. }) = p.top() {
        let view = *view;
        return Sec::from(dynamic_step(p, raw, view, open));
    }
    if super::eqi(&first, tok::URL)
        && let Some(Ctx::DynamicRel { view, rel }) = p.top()
    {
        const G: &str = "url <url>";
        let view = *view;
        let rel = rel.clone();
        if t.has_more_than(1) {
            return Sec::Err(too_many(G));
        }
        if !t.includes(1) {
            return Sec::Err(expected(G));
        }
        let url = t.get(1);
        if let Some(rv) = p.relationship_view_mut(view, &rel) {
            rv.url = Some(url.into_owned());
        }
        return Sec::Handled;
    }

    // -- theme / themes ----------------------------------------------------
    if super::eqi(&first, tok::THEME) && views_or_styles {
        return Sec::from(theme(p, raw));
    }
    if super::eqi(&first, tok::THEMES) && views_or_styles {
        return Sec::from(themes(p, raw));
    }

    // -- terminology -------------------------------------------------------
    if super::eqi(&first, tok::TERMINOLOGY) && in_views {
        p.start_context(Ctx::Terminology);
        return Sec::Handled;
    }
    if matches!(p.top(), Some(Ctx::Terminology)) {
        if super::eqi(&first, tok::PERSON) {
            return Sec::from(term(p, raw, "person", |t, v| t.person = Some(v)));
        }
        if super::eqi(&first, tok::SOFTWARE_SYSTEM) {
            return Sec::from(term(p, raw, "softwareSystem", |t, v| {
                t.software_system = Some(v);
            }));
        }
        if super::eqi(&first, tok::CONTAINER) {
            return Sec::from(term(p, raw, "container", |t, v| {
                t.container = Some(v);
            }));
        }
        if super::eqi(&first, tok::COMPONENT) {
            return Sec::from(term(p, raw, "component", |t, v| t.component = Some(v)));
        }
        if super::eqi(&first, tok::DEPLOYMENT_NODE) {
            return Sec::from(term(p, raw, "deploymentNode", |t, v| {
                t.deployment_node = Some(v);
            }));
        }
        if super::eqi(&first, tok::INFRASTRUCTURE_NODE) {
            return Sec::from(term(p, raw, "infrastructureNode", |t, v| {
                t.infrastructure_node = Some(v);
            }));
        }
        if super::eqi(&first, tok::TERMINOLOGY_RELATIONSHIP) {
            return Sec::from(term(p, raw, "relationship", |t, v| {
                t.relationship = Some(v);
            }));
        }
        if super::eqi(&first, tok::METADATA_SYMBOLS) {
            return Sec::from(metadata_symbols(p, raw));
        }
    }

    Sec::No
}

/// Success pushes the view context (always, or only when the statement
/// opened a block — `filtered`); failure returns the diagnostic.
fn view_arm(
    p: &mut Parser,
    result: Result<ViewRef, DslError>,
    ctx: impl FnOnce(ViewRef) -> Ctx,
    push: bool,
) -> Sec {
    match result {
        Ok(view) => {
            if push {
                p.start_context(ctx(view));
            }
            Sec::Handled
        }
        Err(e) => Sec::Err(e),
    }
}

/// The `light`/`dark` arms: the official parser mutates the SAME context object
/// and pushes it — mutate the top frame's scheme, then push a clone (a pop
/// of the clone leaves the mutated scheme on the outer frame, as the official parser does).
fn retheme(p: &mut Parser, view: ViewRef, scheme: Option<ColorScheme>) {
    if let Some(Ctx::ImageView { color_scheme, .. }) = p.stack.last_mut() {
        *color_scheme = scheme;
    }
    let ctx = p.stack.last().cloned().unwrap_or(Ctx::ImageView {
        view,
        color_scheme: scheme,
    });
    p.start_context(ctx);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use oxidizr_core::views::{MetadataSymbols, RankDirection};

    use super::super::{InstalledThemes, ParseOptions, ParseResult, parse_dsl};

    fn full(dsl: &str) -> ParseResult {
        parse_dsl(dsl, ParseOptions::fragment()).expect("parse")
    }

    fn errors(r: &ParseResult) -> Vec<String> {
        r.errors.iter().map(|e| e.message.clone()).collect()
    }

    fn assert_clean(r: &ParseResult) {
        assert!(r.errors.is_empty(), "unexpected errors: {:?}", errors(r));
    }

    fn first_error(r: &ParseResult) -> String {
        errors(r).into_iter().next().unwrap_or_default()
    }

    fn views(body: &str) -> ParseResult {
        full(&format!(
            r#"workspace {{
  model {{
    user = person "User"
    sys = softwareSystem "System" {{
      web = container "Web" {{
        c1 = component "C1"
      }}
      db = container "DB"
    }}
    other = softwareSystem "Other"
    user -> web "Uses"
    rel1 = web -> db "Reads from"
  }}
  views {{
{body}
  }}
}}
"#
        ))
    }

    fn views_model(body: &str, model: &str) -> ParseResult {
        full(&format!(
            r"workspace {{
  model {{
{model}  }}
  views {{
{body}
  }}
}}
"
        ))
    }

    #[test]
    fn landscape_default_key_and_description() {
        let r = views("    systemLandscape slkey \"A description\" {\n    }");
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(w.views.system_landscape_views.len(), 1);
        assert_eq!(w.views.system_landscape_views[0].base.key, "slkey");
        assert!(!w.views.system_landscape_views[0].base.generated_key);
        assert_eq!(
            w.views.system_landscape_views[0]
                .base
                .description
                .as_deref(),
            Some("A description")
        );

        // Generated key + order.
        let r = views("    systemLandscape {\n    }");
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.system_landscape_views[0].base.key,
            "SystemLandscape-001"
        );
        assert!(
            r.workspace.views.system_landscape_views[0]
                .base
                .generated_key
        );
    }

    #[test]
    fn landscape_too_many_tokens() {
        let r = views("    systemLandscape k d x");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: systemLandscape [key] [description] {"
        );
    }

    #[test]
    fn context_view_resolution_and_key_rules() {
        let r = views("    systemContext sys ctxkey \"ctx desc\" {\n    }");
        assert_clean(&r);
        assert_eq!(r.workspace.views.system_context_views[0].base.key, "ctxkey");

        let r = views("    systemContext nope");
        assert_eq!(
            first_error(&r),
            "The software system \"nope\" does not exist"
        );

        let r = views("    systemContext user");
        assert_eq!(
            first_error(&r),
            "The element \"user\" is not a software system"
        );

        let r = views("    systemContext sys \"bad.key\"");
        assert_eq!(
            first_error(&r),
            "View keys can only contain the following characters: a-zA-Z0-9_-"
        );

        let r = views("    systemContext");
        assert_eq!(
            first_error(&r),
            "Expected: systemContext <software system identifier> [key] [description] {"
        );
    }

    #[test]
    fn container_and_component_views() {
        let r = views("    container sys {\n    }\n    component web {\n    }");
        assert_clean(&r);
        assert_eq!(r.workspace.views.container_views.len(), 1);
        assert_eq!(r.workspace.views.component_views.len(), 1);

        let r = views("    component sys");
        assert_eq!(first_error(&r), "The element \"sys\" is not a container");

        let r = views("    container");
        assert_eq!(
            first_error(&r),
            "Expected: container <software system identifier> [key] [description] {"
        );
    }

    #[test]
    fn dynamic_views() {
        let r = views("    dynamic * {\n    }\n    dynamic sys dynkey \"desc\" {\n    }");
        assert_clean(&r);
        assert_eq!(r.workspace.views.dynamic_views.len(), 2);
        assert_eq!(r.workspace.views.dynamic_views[0].base.key, "Dynamic-001");
        assert_eq!(r.workspace.views.dynamic_views[1].base.key, "dynkey");
        assert_eq!(
            r.workspace.views.dynamic_views[1]
                .base
                .description
                .as_deref(),
            Some("desc")
        );

        let r = views("    dynamic user");
        assert_eq!(
            first_error(&r),
            "The element \"user\" is not a software system or container"
        );

        let r = views("    dynamic nope");
        assert_eq!(
            first_error(&r),
            "The software system or container \"nope\" does not exist"
        );
    }

    #[test]
    fn deployment_view_env_and_scope() {
        let model = "    s = softwareSystem \"S\"\n    env = deploymentEnvironment \"Live\" {\n      n = deploymentNode \"N\" {\n        si = softwareSystemInstance s\n      }\n    }\n";
        // wildcard + environment by name
        let r = views_model("    deployment * Live dv1 \"dv desc\" {\n    }", model);
        assert_clean(&r);
        assert_eq!(r.workspace.views.deployment_views[0].environment, "Live");
        assert_eq!(r.workspace.views.deployment_views[0].base.key, "dv1");
        // scope + environment via the registered pseudo identifier
        let r = views_model("    deployment s env {\n    }", model);
        assert_clean(&r);
        let dv = &r.workspace.views.deployment_views[0];
        assert_eq!(dv.environment, "Live");
        // `s = softwareSystem "S"` is the first model element → id 1.
        assert_eq!(
            dv.software_system_id.as_ref().map(oxidizr_core::Id::as_str),
            Some("1")
        );
        assert_eq!(dv.base.key, "Deployment-001");

        let r = views_model("    deployment * Nope", model);
        assert_eq!(first_error(&r), "The environment \"Nope\" does not exist");

        let r = views_model("    deployment user Live", model);
        assert_eq!(
            first_error(&r),
            "The software system \"user\" does not exist"
        );
    }

    #[test]
    fn filtered_views() {
        let r = views(
            "    systemLandscape {\n    }\n    filtered SystemLandscape-001 include \"Person,Software System\" fkey",
        );
        assert_clean(&r);
        let fv = &r.workspace.views.filtered_views[0];
        assert_eq!(fv.base.key, "fkey");
        assert_eq!(fv.base_view_key, "SystemLandscape-001");
        assert_eq!(
            fv.tags,
            ["Person".to_string(), "Software System".to_string()]
        );

        let r = views("    systemLandscape base {\n    }\n    filtered base maybe t");
        assert_eq!(first_error(&r), "Filter mode should be include or exclude");

        let r = views("    filtered nope include t");
        assert_eq!(first_error(&r), "The view \"nope\" does not exist");

        let r = views("    dynamic * dyn {\n    }\n    filtered dyn include t");
        assert_eq!(
            first_error(&r),
            "The view \"dyn\" must be a System Landscape, System Context, Container, Component, or Deployment view"
        );

        let r = views_model(
            "    systemLandscape base {\n    }\n    filtered base include t f2\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_clean(&r);
        // tags: split keeps leading empties, trim-based blank check, dedup
        let r = views("    systemLandscape base {\n    }\n    filtered base include \", ,A,A,B\"");
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.filtered_views[0].tags,
            ["A".to_string(), "B".to_string()]
        );
    }

    #[test]
    fn filtered_autolayout_base_fails() {
        let r = views_model(
            "    systemLandscape base {\n        autoLayout\n    }\n    filtered base include t\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_eq!(
            first_error(&r),
            "The view \"base\" has automatic layout enabled - this is not supported for filtered views"
        );
    }

    #[test]
    fn image_view_def_and_no_content_hook() {
        let r = views("    image * img1");
        // An image view without any content fails at close.
        assert_eq!(first_error(&r), "The image view \"img1\" has no content");

        let r = views("    image nope");
        assert_eq!(first_error(&r), "The element \"nope\" does not exist");

        let r = views("    image");
        assert_eq!(
            first_error(&r),
            "Expected: image <*|element identifier> [key] {"
        );
    }

    #[test]
    fn image_view_pseudo_scope_does_not_exist() {
        let model = "    s = softwareSystem \"S\"\n    env = deploymentEnvironment \"Live\" {\n      n = deploymentNode \"N\" {\n        si = softwareSystemInstance s\n      }\n    }\n";
        let r = views_model(
            "    image env {\n        plantuml \"x\\ny\"\n    }\n",
            model,
        );
        assert_eq!(first_error(&r), "The element \"env\" does not exist");
    }

    #[test]
    fn static_content_include_and_exclude() {
        let r = views("    container sys {\n        include *\n        exclude db\n    }");
        assert_clean(&r);
        let content = &r.workspace.views.container_views[0].content;
        let ids: Vec<&str> = content.elements.iter().map(|e| e.id.as_str()).collect();
        // sys + user + web + db + other (nearest neighbours of systems),
        // then db removed (with its `web → db` relationship).
        assert!(!ids.contains(&"5"), "db removed: {ids:?}");
        // `user → web` survives.
        assert_eq!(
            content
                .relationships
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["7"]
        );

        let r = views("    container sys {\n        include user\n    }");
        assert_clean(&r);

        let r = views("    systemLandscape {\n        include web\n    }");
        assert_eq!(
            first_error(&r),
            "The element \"web\" can not be added to this type of view"
        );

        let r = views("    systemLandscape {\n        include\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: include <*|identifier|expression> [*|identifier|expression...]"
        );

        let r = views("    container sys {\n        include\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: include <*|*?|identifier|expression> [*|identifier|expression...]"
        );

        let r = views("    systemLandscape {\n        exclude\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: exclude <identifier|expression> [identifier|expression...]"
        );
    }

    #[test]
    fn static_include_relationship_between_views() {
        // `web -> db` gets a relationship view once both ends are in view.
        let r = views("    container sys {\n        include web\n        include db\n    }");
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.container_views[0]
                .content
                .relationships
                .len(),
            1
        );
    }

    #[test]
    fn animation_steps_static() {
        let r = views(
            "    systemLandscape {\n        include user\n        include sys\n        animationStep user\n        animation {\n            include nothing\n            animationStep sys\n        }\n    }",
        );
        // Inside an `animation { }` block every statement is a step:
        // `include` is looked up as an element identifier (catch-all).
        assert_eq!(
            first_error(&r),
            "The element/relationship \"include\" does not exist"
        );

        let r = views(
            "    systemLandscape {\n        include user\n        include sys\n        animationStep nope\n    }",
        );
        assert_eq!(
            first_error(&r),
            "The element/relationship \"nope\" does not exist"
        );

        let r =
            views("    systemLandscape {\n        include user\n        animationStep web\n    }");
        // web is a container, not in this view → core NoAnimatableElements.
        assert_eq!(
            first_error(&r),
            "None of the specified elements exist in this view."
        );

        let r = views("    systemLandscape {\n        animationStep\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: animationStep <identifier|element expression> [identifier|element expression...]"
        );
    }

    #[test]
    fn animation_no_elements() {
        let r = views_model(
            "    custom {\n        animationStep nope2\n    }\n",
            "    ce = element \"CE\"\n    s = softwareSystem \"S\"\n",
        );
        assert_eq!(
            first_error(&r),
            "The element/relationship \"nope2\" does not exist"
        );
    }

    #[test]
    fn custom_view_content() {
        let r = views_model(
            "    custom cv \"title\" \"desc\" {\n        include ce\n        animationStep ce\n        exclude ce\n    }\n",
            "    ce = element \"CE\"\n    s = softwareSystem \"S\"\n",
        );
        assert_clean(&r);
        let cv = &r.workspace.views.custom_views[0];
        assert_eq!(cv.base.key, "cv");
        assert_eq!(cv.base.title.as_deref(), Some("title"));
        assert_eq!(cv.base.description.as_deref(), Some("desc"));

        let r = views_model(
            "    custom {\n        include s\n    }\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_eq!(
            first_error(&r),
            "The element \"s\" can not be added to this type of view"
        );

        let r = views_model(
            "    custom {\n        include\n    }\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_eq!(
            first_error(&r),
            "Expected: include <*|identifier> [*|identifier...]"
        );
    }

    #[test]
    fn deployment_content_instances() {
        let model = "    s = softwareSystem \"S\" {\n        c = container \"C\"\n    }\n    deploymentEnvironment \"Live\" {\n        n = deploymentNode \"N\" {\n            si = softwareSystemInstance s\n            ci = containerInstance c\n            in1 = infrastructureNode \"I\"\n        }\n    }\n";
        let r = views_model(
            "    deployment * Live {\n        include s\n        include n\n        include in1\n    }\n",
            model,
        );
        assert_clean(&r);
        let content = &r.workspace.views.deployment_views[0].content;
        // `include s` adds the SSI instance; `include n` the node (its
        // subtree via the core cascade) and infrastructure node.
        assert!(
            content.elements.iter().any(|e| e.id.as_str() == "6"),
            "instance added: {:?}",
            content
                .elements
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>()
        );

        let r = views_model(
            "    deployment * Live {\n        include s\n        exclude s\n    }\n",
            model,
        );
        assert_clean(&r);
        // `exclude s` removes only the instance - the official
        // instance-removal path has no ancestor
        // leg, so the parent node `include` added via the ancestor walk
        // STAYS (verified: the official view keeps the node id).
        let ids: Vec<&str> = r.workspace.views.deployment_views[0]
            .content
            .elements
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["3"]);
    }

    #[test]
    fn dynamic_pair_and_rel_forms() {
        // Pair form needs an EXISTING model relationship (user → web
        // "Uses" in the test model) and a scope that permits containers.
        let r = views("    dynamic sys {\n        user -> web \"Uses it\"\n    }");
        assert_clean(&r);
        let rv = &r.workspace.views.dynamic_views[0].content.relationships[0];
        assert_eq!(rv.description.as_deref(), Some("Uses it"));
        assert_eq!(rv.order.as_deref(), Some("1"));

        // rel-id form with an explicit description
        let r = views_model(
            "    dynamic * {\n        rel1 \"Changed\"\n    }\n",
            "    s = softwareSystem \"S\"\n    t = softwareSystem \"T\"\n    rel1 = s -> t \"Orig\"\n",
        );
        assert_clean(&r);
        let rv = &r.workspace.views.dynamic_views[0].content.relationships[0];
        assert_eq!(rv.description.as_deref(), Some("Changed"));

        // explicit [order:] overrides — the sequence number is still burned
        let r = views("    dynamic sys {\n        5: user -> web\n        user -> web\n    }");
        assert_clean(&r);
        let rels = &r.workspace.views.dynamic_views[0].content.relationships;
        assert_eq!(rels[0].order.as_deref(), Some("5"));
        assert_eq!(rels[1].order.as_deref(), Some("2"));

        // `{` is consumed as the description (official quirk) — verified:
        // description "{"
        let r = views(
            "    dynamic sys {\n        user -> web {\n            url \"https://x\"\n        }\n    }",
        );
        assert_clean(&r);
        let rels = &r.workspace.views.dynamic_views[0].content.relationships;
        assert_eq!(rels[0].description.as_deref(), Some("{"));
        assert_eq!(rels[0].url.as_deref(), Some("https://x"));
    }

    #[test]
    fn dynamic_errors() {
        let r = views("    dynamic * {\n        nope -> web\n    }");
        assert_eq!(
            first_error(&r),
            "The source element \"nope\" does not exist"
        );

        let r = views("    dynamic sys {\n        web -> other\n    }");
        // Pair form without a model relationship → core lookup failure
        // (the scope system itself can not be added at all).
        assert_eq!(
            first_error(&r),
            "A relationship between Web and Other does not exist in model."
        );

        let r = views("    dynamic * {\n        user -> web \"a\" \"b\" \"c\"\n    }");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: [order:] <identifier> -> <identifier> [description] [technology]"
        );

        let r = views("    dynamic * {\n        nosuchrel \"x\" \"y\"\n    }");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: [order:] <identifier> [description]"
        );

        let r = views("    dynamic * {\n        nosuchrel\n    }");
        assert_eq!(
            first_error(&r),
            "The relationship \"nosuchrel\" does not exist"
        );

        let r = views("    dynamic * {\n        5:\n    }");
        assert_eq!(first_error(&r), "Index 0 out of bounds for length 0");
    }

    #[test]
    fn dynamic_parallel_sequences() {
        let r = views(
            "    dynamic sys {\n        user -> web\n        {\n            user -> web\n        }\n        user -> web\n    }",
        );
        assert_clean(&r);
        let rels = &r.workspace.views.dynamic_views[0].content.relationships;
        // Official quirk: ending the parallel sequence DISCARDS
        // the child counter when the block added steps, so the next
        // top-level step reuses the number after the pre-block counter.
        assert_eq!(
            rels.iter()
                .map(|r| r.order.as_deref().unwrap_or("?"))
                .collect::<Vec<_>>(),
            ["1", "2", "2"]
        );
    }

    #[test]
    fn dynamic_parallel_empty_burns_number() {
        let r = views(
            "    dynamic sys {\n        user -> web\n        {\n        }\n        user -> web\n    }",
        );
        assert_clean(&r);
        let rels = &r.workspace.views.dynamic_views[0].content.relationships;
        // The empty parallel block still burns 2 (`endParallelSequence`
        // semantics), so the final step is 3.
        assert_eq!(rels[1].order.as_deref(), Some("3"));
    }

    #[test]
    fn url_arm_guards() {
        let r = views_model(
            "    dynamic * {\n        ab {\n            url\n        }\n    }\n",
            "    a = softwareSystem \"A\"\n    b = softwareSystem \"B\"\n    ab = a -> b \"x\"\n",
        );
        assert_eq!(first_error(&r), "Expected: url <url>");

        let r = views_model(
            "    dynamic * {\n        ab {\n            url a b\n        }\n    }\n",
            "    a = softwareSystem \"A\"\n    b = softwareSystem \"B\"\n    ab = a -> b \"x\"\n",
        );
        assert_eq!(first_error(&r), "Too many tokens, expected: url <url>");
    }

    #[test]
    fn autolayout() {
        let r = views("    systemLandscape sl {\n        autoLayout\n    }");
        assert_clean(&r);
        let al = r.workspace.views.system_landscape_views[0]
            .content
            .automatic_layout
            .expect("autoLayout");
        assert_eq!(al.rank_direction, Some(RankDirection::LeftRight));
        assert_eq!(al.rank_separation, 100);
        assert_eq!(al.node_separation, 50);
        assert_eq!(al.edge_separation, 50);
        assert!(al.vertices);

        let r = views("    systemLandscape sl {\n        autoLayout tb 200 30 40 false\n    }");
        assert_clean(&r);
        let al = r.workspace.views.system_landscape_views[0]
            .content
            .automatic_layout
            .expect("autoLayout");
        assert_eq!(al.rank_direction, Some(RankDirection::TopBottom));
        assert_eq!(al.rank_separation, 200);
        assert!(!al.vertices);

        let r = views("    systemLandscape sl {\n        autoLayout TB\n    }");
        assert_eq!(first_error(&r), "Valid rank directions are: tb|bt|lr|rl");

        let r = views("    systemLandscape sl {\n        autoLayout lr x\n    }");
        assert_eq!(
            first_error(&r),
            "Rank separation must be positive integer in pixels"
        );

        let r = views("    systemLandscape sl {\n        autoLayout lr 1 2 3 maybe\n    }");
        assert_eq!(first_error(&r), "Vertices must be true or false");

        let r = views("    systemLandscape sl {\n        autoLayout lr 1 2 3 false 9\n    }");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: autoLayout [rankDirection] [rankSeparation] [nodeSeparation] [edgeSeparation] [vertices]"
        );

        // filtered context → model-view context cast failure
        let r = views(
            "    systemLandscape base {\n    }\n    filtered base include t f {\n        autoLayout\n    }",
        );
        assert_eq!(
            first_error(&r),
            "Expected com.structurizr.dsl.ModelViewDslContext but got com.structurizr.dsl.FilteredViewDslContext"
        );
    }

    #[test]
    fn default_title_description() {
        let r = views(
            "    systemLandscape sl {\n    }\n    systemContext sys sc {\n        default\n        title \"My Title\"\n        description \"My Desc\"\n    }",
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.configuration.default_view.as_deref(),
            Some("sc")
        );
        assert_eq!(
            r.workspace.views.system_context_views[0]
                .base
                .title
                .as_deref(),
            Some("My Title")
        );
        assert_eq!(
            r.workspace.views.system_context_views[0]
                .base
                .description
                .as_deref(),
            Some("My Desc")
        );

        let r = views("    systemLandscape sl {\n        title a b\n    }");
        assert_eq!(first_error(&r), "Too many tokens, expected: title <title>");

        let r = views("    systemLandscape sl {\n        description\n    }");
        assert_eq!(first_error(&r), "Expected: description <description>");
    }

    #[test]
    fn theme_arm() {
        let r = views_model(
            "    theme https://example.com/theme.json\n    theme https://example.com/theme.json\n    theme default\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.configuration.themes,
            [
                "https://example.com/theme.json".to_string(),
                "https://static.structurizr.com/themes/default/theme.json".to_string()
            ]
        );

        let r = views("    theme");
        assert_eq!(first_error(&r), "Expected: theme <name|url|file>");

        let r = views("    theme a b");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: theme <name|url|file>"
        );
    }

    #[test]
    fn theme_installed_name_branch() {
        // verified : with an injected registry, an installed name takes
        // the `InstalledThemes.isInstalled` branch — stored verbatim (trim,
        // NO dedup: `Configuration.addTheme`'s installed branch just adds),
        // never reaching the FILE branch.
        let opts = ParseOptions {
            installed_themes: Some(InstalledThemes::new(|name| name == "mine")),
            ..ParseOptions::fragment()
        };
        let r = parse_dsl(
            "workspace {\n  views {\n    theme mine\n    theme mine\n  }\n}\n",
            opts,
        )
        .expect("parse");
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.configuration.themes,
            ["mine".to_string(), "mine".to_string()]
        );

        // Not in the registry → FILE branch (fragment ⇒ official crash, the
        // `themes_arm_and_missing_file` test path).
        let opts = ParseOptions {
            installed_themes: Some(InstalledThemes::new(|_| false)),
            ..ParseOptions::fragment()
        };
        let r = parse_dsl("workspace {\n  views {\n    theme mine\n  }\n}\n", opts).expect("parse");
        assert_eq!(errors(&r).len(), 1);
        assert_eq!(r.workspace.views.configuration.themes, Vec::<String>::new());
    }

    #[test]
    fn themes_arm_and_missing_file() {
        let r = views("    themes");
        assert_eq!(
            first_error(&r),
            "Expected: themes <name|url|file> [name|url|file] ... [name|url|file]"
        );

        // File branch needs a real DSL file for the base directory.
        let dir = std::env::temp_dir().join(format!("oxidizr-themes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        let dsl_path = dir.join("oxz-themes.dsl");
        let dsl_path = dsl_path.to_string_lossy().into_owned();
        let base = dir.to_string_lossy().into_owned();
        let r = parse_dsl(
            "workspace {\n  views {\n        themes https://x.example/t.json blankish\n  }\n}\n",
            ParseOptions::file_with_base(&dsl_path, &base),
        )
        .expect("parse");
        // "blankish" is not a URL → file branch → not found
        let msgs = errors(&r);
        assert_eq!(msgs.len(), 1);
        assert!(
            msgs[0].starts_with("Theme blankish does not exist at ")
                && msgs[0].ends_with("blankish"),
            "{:?}",
            msgs[0]
        );
        // The URL theme was added before the error.
        assert_eq!(
            r.workspace.views.configuration.themes,
            ["https://x.example/t.json".to_string()]
        );
    }

    #[test]
    fn theme_file_deferred_and_svg() {
        let dir = std::env::temp_dir().join(format!("oxidizr-views-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        std::fs::write(dir.join("oxz-theme.json"), b"{}").expect("theme");
        std::fs::write(dir.join("oxz-logo.svg"), b"<svg/>").expect("svg");
        std::fs::write(dir.join("oxz-logo.png"), b"not a png").expect("png");
        std::fs::write(dir.join("oxz-notes.txt"), b"hello").expect("txt");
        let dsl_path = dir.join("oxz-views.dsl");
        let dsl_path = dsl_path.to_string_lossy().into_owned();
        let base = dir.to_string_lossy().into_owned();

        let parse = |body: &str| {
            parse_dsl(
                &format!("workspace {{\n  views {{\n{body}\n  }}\n}}\n"),
                ParseOptions::file_with_base(&dsl_path, &base),
            )
            .expect("parse")
        };

        let r = parse("        theme oxz-theme.json");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::IMPORT_DEFERRED);

        let r = parse("        image * iv {\n            image oxz-logo.svg\n        }");
        // an image view with plain content closes cleanly
        assert_clean(&r);
        let iv = &r.workspace.views.image_views[0];
        assert_eq!(
            iv.content.as_deref(),
            Some("data:image/svg+xml;base64,PHN2Zy8+")
        );
        assert_eq!(iv.content_type.as_deref(), Some("image/svg+xml"));
        assert_eq!(iv.base.title.as_deref(), Some("oxz-logo.svg"));

        // svg content inside a light block → STILL plain content (erratum)
        let r = parse(
            "        image * iv {\n            light {\n                image oxz-logo.svg\n            }\n        }",
        );
        assert_clean(&r);
        assert_eq!(r.workspace.views.image_views[0].content_light, None);
        assert!(r.workspace.views.image_views[0].content.is_some());

        let r = parse("        image * iv {\n            image oxz-logo.png\n        }");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::JVM_ONLY);

        let r = parse("        image * iv {\n            image oxz-notes.txt\n        }");
        assert!(
            first_error(&r).ends_with("oxz-notes.txt is not a supported image file."),
            "{}",
            first_error(&r)
        );

        let r = parse("        image * iv {\n            image oxz-missing.svg\n        }");
        assert!(
            first_error(&r).starts_with("The file at ")
                && first_error(&r).ends_with("does not exist"),
            "{}",
            first_error(&r)
        );

        let r =
            parse("        image * iv {\n            image https://example.com/a.png\n        }");
        assert_eq!(first_error(&r), "Image views via HTTPS are not permitted");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::NET_DISABLED);
    }

    #[test]
    fn diagram_content_arms() {
        let r = views("    image * ix {\n        plantuml \"a\\nb\"\n    }");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::IMPORT_DEFERRED);
        assert_eq!(
            first_error(&r),
            "plantuml <source> import is not supported yet"
        );

        let r = views("    image * ix {\n        plantuml\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: plantuml <source|file|url|viewKey>"
        );

        let r = views("    image * ix {\n        mermaid a b\n    }");
        assert_eq!(
            first_error(&r),
            "Too many tokens, expected: mermaid <source|file|url|viewKey>"
        );

        // viewKey branch: the key of a model view → deferred
        let r = views_model(
            "    systemLandscape sl {\n    }\n    image * ix {\n        plantuml sl\n    }\n",
            "    s = softwareSystem \"S\"\n",
        );
        assert_eq!(
            first_error(&r),
            "plantuml <viewKey> import is not supported yet"
        );

        let r = views("    image * ix {\n        kroki plantuml \"a\\nb\"\n    }");
        assert_eq!(r.errors[0].code, oxidizr_diag::codes::IMPORT_DEFERRED);

        let r = views("    image * ix {\n        kroki plantuml\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: kroki <format> <source|file|url>"
        );

        let r = views("    image * ix {\n        mermaid http://example.com/d.mmd\n    }");
        assert_eq!(first_error(&r), "Image views via HTTP are not permitted");
    }

    #[test]
    fn light_dark_repush() {
        let r = views(
            "    image * x {\n        light {\n            image oxz-none.svg\n        }\n    }",
        );
        // the inner content fails (file missing relative to CWD-less
        // fragment parse → crash per resolve_icon precedent)
        assert_eq!(first_error(&r), "NullPointerException");
    }

    #[test]
    fn terminology_arms() {
        let r = views(
            "    terminology {\n        person Actor\n        softwareSystem Platform\n        container Subsystem\n        component Module\n        deploymentNode Server\n        infrastructureNode Service\n        relationship Connects to\n        metadata round\n    }",
        );
        assert_clean(&r);
        let t = &r.workspace.views.configuration.terminology;
        assert_eq!(t.person.as_deref(), Some("Actor"));
        assert_eq!(t.software_system.as_deref(), Some("Platform"));
        assert_eq!(t.container.as_deref(), Some("Subsystem"));
        assert_eq!(t.component.as_deref(), Some("Module"));
        assert_eq!(t.deployment_node.as_deref(), Some("Server"));
        assert_eq!(t.infrastructure_node.as_deref(), Some("Service"));
        // the official parser stores token `1` verbatim — no too-many check, so the
        // unquoted second word is dropped (`Connects to` → `Connects`).
        assert_eq!(t.relationship.as_deref(), Some("Connects"));
        assert_eq!(
            r.workspace.views.configuration.metadata_symbols,
            Some(MetadataSymbols::RoundBrackets)
        );

        let r = views("    terminology {\n        person\n    }");
        assert_eq!(first_error(&r), "Expected: person <term>");

        let r = views("    terminology {\n        metadata wibble\n    }");
        assert_eq!(first_error(&r), "The symbol type \"wibble\" is not valid");

        let r = views("    terminology {\n        metadata DOUBLE-ANGLE\n    }");
        assert_clean(&r);
        assert_eq!(
            r.workspace.views.configuration.metadata_symbols,
            Some(MetadataSymbols::DoubleAngleBrackets)
        );

        let r = views("    terminology {\n        metadata\n    }");
        assert_eq!(
            first_error(&r),
            "Expected: metadata <square|round|curly|angle|double-angle|none>"
        );
    }

    #[test]
    fn view_without_braces_swallows_until_brace() {
        // The unconditional startContext: `systemLandscape` (no `{`) keeps
        // the view context open; `include *` lands in the view, and the
        // single `}` closes BOTH view and workspace — leaving the stack
        // non-empty → the end-of-content error.
        let r = full(
            "workspace {\n  model {\n    s = softwareSystem \"S\"\n  }\n  views {\n    systemLandscape sl\n    include *\n  }\n",
        );
        let msgs = errors(&r);
        assert!(msgs.is_empty() || msgs.len() == 1);
        // The view still got its content.
        assert_eq!(
            r.workspace.views.system_landscape_views[0]
                .content
                .elements
                .len(),
            1
        );
    }

    #[test]
    fn fixture_dynamic_like_big_bank() {
        let model = "    internetBankingServices = softwareSystem \"Internet Banking Services\" {\n        webapp = container \"Web Application\"\n        set = container \"Settlement\"\n    }\n    atm = softwareSystem \"ATM\"\n    atm -> webapp \"Uses\" \"HTTPS\"\n    webapp -> set \"Clears\"\n";
        let r = views_model(
            "    dynamic internetBankingServices dbank {\n        atm -> webapp \"Withdraw cash\" \"HTTPS\"\n        webapp -> set\n    }\n",
            model,
        );
        assert_clean(&r);
        let rels = &r.workspace.views.dynamic_views[0].content.relationships;
        assert_eq!(rels.len(), 2);
        assert_eq!(rels[0].order.as_deref(), Some("1"));
        assert_eq!(rels[1].order.as_deref(), Some("2"));
    }

    #[test]
    fn filtered_no_brace_does_not_open_context() {
        let r = views(
            "    systemLandscape base {\n    }\n    filtered base include t fkey\n    systemContext sys sc {\n    }",
        );
        assert_clean(&r);
        assert_eq!(r.workspace.views.filtered_views.len(), 1);
        assert_eq!(r.workspace.views.system_context_views.len(), 1);
    }

    #[test]
    fn custom_view_and_animation_step_ids() {
        let r = views_model(
            "    custom cv {\n        include *\n        animation {\n            ce1 ce2\n        }\n    }\n",
            "    ce1 = element \"CE1\"\n    ce2 = element \"CE2\"\n",
        );
        assert_clean(&r);
        let cv = &r.workspace.views.custom_views[0];
        assert_eq!(cv.content.elements.len(), 2);
        assert_eq!(cv.animations.len(), 1);
        assert_eq!(cv.animations[0].element_ids.len(), 2);
    }
}
