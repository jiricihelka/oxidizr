//! View-configuration arms of the view sections: the `autoLayout` parser,
//! the title/description text legs and the image-view content arms
//! (plantuml/mermaid/kroki/diagram-file with the official `ImageImporter`
//! inlining). The definition, content and step arms live in the sibling
//! modules; the shared free helpers stay in the parent module.

use oxidizr_core::ViewRef;
use oxidizr_core::url::{is_http_url, is_https_url};
use oxidizr_core::views::{AutomaticLayout, RankDirection};

use crate::compat::java_parse_int;

use super::super::stmt_line::{java_absolute, java_canonical};
use super::{
    Ctx, DslError, Parser, Tokens, apply_text, deferred, expected, net_http, net_https, too_many,
};

// ---------------------------------------------------------------------------
// autoLayout / default / title / description
// ---------------------------------------------------------------------------

/// the auto-layout parser over the RAW tokens (the trailing `{` is parsed as a
/// rank direction — official behavior), after the `model-view context`
/// cast the driver arm performs via the top-context fetch.
pub(super) fn auto_layout(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
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
        vertices = if super::super::eqi(&v, "true") {
            true
        } else if super::super::eqi(&v, "false") {
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
pub(super) fn view_text(p: &mut Parser, tokens: &Tokens, title: bool) -> Result<(), DslError> {
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
pub(super) fn plantuml_or_mermaid(
    p: &mut Parser,
    tokens: &Tokens,
    keyword: &str,
) -> Result<(), DslError> {
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
pub(super) fn kroki(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
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
pub(super) fn image_content(
    p: &mut Parser,
    view: ViewRef,
    tokens: &Tokens,
) -> Result<(), DslError> {
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
    let data_uri = format!(
        "data:image/svg+xml;base64,{}",
        super::super::base64_std(&bytes)
    );
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
