//! Theme and terminology arms of the view sections: the `theme`/`themes`
//! parsers with the official add-theme branch order, the `term`
//! terminology setter and the `metadataSymbols` arm. The definition,
//! content and step arms live in the sibling modules; the shared free
//! helpers stay in the parent module.

use oxidizr_core::url::is_url;
use oxidizr_core::views::{MetadataSymbols, Terminology};

use crate::compat::java_trim;

use super::super::stmt_line::java_absolute;
use super::{DslError, Parser, Tokens, deferred, expected, is_trim_empty, too_many};

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
    if super::super::eqi(&theme, "default") {
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

pub(super) fn theme(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
    const G: &str = "theme <name|url|file>";
    if tokens.has_more_than(1) {
        return Err(too_many(G));
    }
    if !tokens.includes(1) {
        return Err(expected(G));
    }
    add_theme(p, tokens.get(1).into_owned())
}

pub(super) fn themes(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
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
pub(super) fn term(
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
pub(super) fn metadata_symbols(p: &mut Parser, tokens: &Tokens) -> Result<(), DslError> {
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
