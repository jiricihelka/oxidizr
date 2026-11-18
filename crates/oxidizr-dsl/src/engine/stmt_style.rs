//! Chain section: reference `StructurizrDslParser.java` arms at Java lines
//! 742–948 — the `archetypes` scope (the nine archetype-definition arms and
//! the archetype property statements `metadata`/`description`/`technology`/
//! `tag`/`tags` plus the archetype `properties`/`perspectives` pushes), the
//! `views` scope with its duplicate guard, the `styles`/`light`/`dark`
//! scopes, and the `element <tag>` / `relationship <tag>` style blocks with
//! all their property statements (`ElementStyleParser` /
//! `RelationshipStyleParser`). `theme`/`themes` sit at 1164/1167 and belong
//! to `stmt_views`.
//!
//! Arm order mirrors the reference `else if` chain exactly. The definition
//! statements (`element <tag>`, `relationship <tag>`) read
//! `withoutContextStartToken()`; every property statement inside a block
//! reads the RAW tokens.

use oxidizr_core::views::{ColorScheme, ElementStyle, IconPosition, LineStyle, Routing, Shape};

use super::ctx::Ctx;
use super::error::DslError;
use super::{Archetype, Parser, Sec, Stmt, Tokens, eqi, tok};

#[allow(clippy::too_many_lines)] // the chain arm order IS the contract
pub(crate) fn section(p: &mut Parser, stmt: &Stmt) -> Sec {
    let first = stmt.first();
    let raw = &stmt.tokens;
    let top = p.top().cloned();

    // ------------------------------------------------------------------
    // archetypes scope (742–845)
    // ------------------------------------------------------------------

    // 742: `archetypes { … }` inside `model`.
    if eqi(&first, tok::ARCHETYPES) && matches!(top, Some(Ctx::Model { .. })) {
        p.start_context(Ctx::Archetypes);
        return Sec::Handled;
    }

    // 745–814: the nine archetype-definition arms, in chain order. The
    // archetype NAME comes from the `id = ` prefix; `firstToken` is both the
    // parent lookup (extend) and, for the keyword spelling, a miss (empty
    // throwaway parent). GROUP has NO context form — a `{` after a group
    // archetype opens nothing.
    if matches!(top, Some(Ctx::Archetypes)) {
        for kind in [
            tok::GROUP,
            tok::CUSTOM_ELEMENT,
            tok::PERSON,
            tok::SOFTWARE_SYSTEM,
            tok::CONTAINER,
            tok::COMPONENT,
            tok::DEPLOYMENT_NODE,
            tok::INFRASTRUCTURE_NODE,
        ] {
            if !p.is_element_keyword_or_archetype(&first, kind) {
                continue;
            }
            let idx = define_archetype(p, stmt.identifier.as_deref(), kind, &first);
            let open = Parser::should_start_context(raw) && kind != tok::GROUP;
            if open {
                p.start_context(Ctx::ElementArchetype {
                    kind: kind.to_string(),
                    idx,
                });
            }
            return Sec::Handled;
        }
        if p.is_relationship_keyword_or_archetype(&first) {
            let idx = define_archetype(p, stmt.identifier.as_deref(), tok::RELATIONSHIP, &first);
            if Parser::should_start_context(raw) {
                p.start_context(Ctx::RelationshipArchetype { idx });
            }
            return Sec::Handled;
        }
    }

    // 816–845: archetype property statements. `metadata` is
    // CustomElementArchetypeDslContext-only; `technology` covers the kinds
    // that HAVE one (plus relationship archetypes); `tag`/`tags` hit the
    // archetype LIST (the element `tags` arms in `stmt_props` exclude
    // archetype contexts, mirroring the reference class guards).
    if let Some(idx) = top.as_ref().and_then(archetype_idx) {
        if eqi(&first, tok::METADATA)
            && matches!(
                top,
                Some(Ctx::ElementArchetype { ref kind, .. }) if kind == tok::CUSTOM_ELEMENT
            )
        {
            return (|| {
                let value = one_token(p, raw, "metadata <metadata>")?;
                p.archetype_list[idx].metadata = value;
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::DESCRIPTION) {
            return (|| {
                let value = one_token(p, raw, "description <description>")?;
                p.archetype_list[idx].description = value;
                Ok(())
            })()
            .into();
        }
        let technology_kind = match top {
            Some(Ctx::ElementArchetype { ref kind, .. }) => matches!(
                kind.as_str(),
                tok::CONTAINER | tok::COMPONENT | tok::DEPLOYMENT_NODE | tok::INFRASTRUCTURE_NODE
            ),
            Some(Ctx::RelationshipArchetype { .. }) => true,
            _ => false,
        };
        if eqi(&first, tok::TECHNOLOGY) && technology_kind {
            return (|| {
                let value = one_token(p, raw, "technology <technology>")?;
                p.archetype_list[idx].technology = value;
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::TAG) {
            return (|| {
                let value = one_token(p, raw, "tag <tag>")?;
                // `Archetype.addTags(String...)` with ONE vararg — the value
                // is TRIMMED but NOT split on commas, and an empty tag lands
                // in the LinkedHashSet (`tag.trim()` is added unconditionally).
                Parser::add_archetype_tag(&mut p.archetype_list[idx], &value);
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::TAGS) {
            return (|| {
                if !raw.includes(1) {
                    return Err(DslError::stmt("Expected: tags <tags> [tags]"));
                }
                // `ArchetypeParser.parseTags`: each token is
                // `addTags(tags.split(","))` — comma-split (trailing empties
                // dropped), each part trimmed, empties kept.
                for i in 1..raw.size() {
                    Parser::add_archetype_tags(&mut p.archetype_list[idx], &raw.get(i));
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::PROPERTIES) {
            p.start_context(Ctx::Properties {
                holders: vec![super::ctx::Holder::Archetype { idx }],
            });
            return Sec::Handled;
        }
        if eqi(&first, tok::PERSPECTIVES) {
            p.start_context(Ctx::Perspectives {
                holders: vec![super::ctx::Holder::Archetype { idx }],
            });
            return Sec::Handled;
        }
    }

    // ------------------------------------------------------------------
    // views / styles scopes (847–861)
    // ------------------------------------------------------------------

    // 847: `views { … }` — once only.
    if eqi(&first, tok::VIEWS) && matches!(top, Some(Ctx::Workspace)) {
        return (|| {
            if p.parsed_tokens.iter().any(|t| t == tok::VIEWS) {
                return Err(DslError::stmt(
                    "Multiple view sets are not permitted in a DSL definition",
                ));
            }
            p.parsed_tokens.push(tok::VIEWS.to_string());
            p.start_context(Ctx::Views);
            Ok(())
        })()
        .into();
    }

    if eqi(&first, tok::STYLES) && matches!(top, Some(Ctx::Views)) {
        p.start_context(Ctx::Styles { color_scheme: None });
        return Sec::Handled;
    }
    if let Some(Ctx::Styles { .. }) = top {
        if eqi(&first, tok::LIGHT) {
            p.start_context(Ctx::Styles {
                color_scheme: Some(ColorScheme::Light),
            });
            return Sec::Handled;
        }
        if eqi(&first, tok::DARK) {
            p.start_context(Ctx::Styles {
                color_scheme: Some(ColorScheme::Dark),
            });
            return Sec::Handled;
        }
    }

    // ------------------------------------------------------------------
    // element styles (862–907)
    // ------------------------------------------------------------------

    // 862: `element <tag> {` — get-or-add per (tag, scheme); the context push
    // is UNCONDITIONAL (no `{` required).
    if eqi(&first, tok::ELEMENT_STYLE)
        && let Some(Ctx::Styles { color_scheme }) = top
    {
        return (|| {
            let t = raw.without_context_start_token();
            let tag = style_tag(&t, "element")?;
            let idx = get_or_add_element_style(p, &tag, color_scheme);
            p.start_context(Ctx::ElementStyle { idx });
            Ok(())
        })()
        .into();
    }

    if let Some(Ctx::ElementStyle { idx }) = top {
        // background/colour/stroke go through the Java setters' colour
        // normalization (`Color`): `#rrggbb` lowercased, HTML names mapped to
        // hex, anything else a statement error (T4 oracle-verified 2026-11-17
        // — the DSL importer calls the same validation JSON hydration does).
        // The `colour` and `color` chain arms share `parseColour`, so the
        // grammar text always says `colour`.
        if eqi(&first, tok::ELEMENT_STYLE_BACKGROUND) {
            return (|| {
                let value = one_token(p, raw, "background <#rrggbb|color name>")?;
                let value = style_color(&value)?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.background = Some(value);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_COLOUR) || eqi(&first, tok::ELEMENT_STYLE_COLOR) {
            return (|| {
                let value = one_token(p, raw, "colour <#rrggbb|color name>")?;
                let value = style_color(&value)?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.color = Some(value);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_STROKE) {
            return (|| {
                let value = one_token(p, raw, "stroke <#rrggbb|color name>")?;
                let value = style_color(&value)?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.stroke = Some(value);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_STROKE_WIDTH) {
            return (|| {
                let value = one_token(p, raw, "strokeWidth <1-10>")?;
                let n = java_int(&value).ok_or_else(|| {
                    DslError::stmt("Stroke width must be an integer between 1 and 10")
                })?;
                // `ElementStyle.setStrokeWidth`: clamp to [1, 10].
                let clamped = n.clamp(1, 10);
                if let Some(s) = element_style_mut(p, idx) {
                    s.stroke_width = Some(clamped);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_SHAPE) {
            return (|| {
                one_grammar(p, raw, "shape", SHAPE_GRAMMAR)?;
                let name = raw.get(1).to_lowercase();
                let shape = SHAPE_NAMES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| DslError::stmt(format!("The shape \"{name}\" is not valid")))?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.shape = Some(shape);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_BORDER) {
            return (|| {
                one_grammar(p, raw, "border", "solid|dashed|dotted")?;
                let name = raw.get(1).to_lowercase();
                let border = BORDER_NAMES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| DslError::stmt(format!("The border \"{name}\" is not valid")))?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.border = Some(border);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_OPACITY) {
            return (|| {
                let value = one_token(p, raw, "opacity <0-100>")?;
                let n = java_int(&value).ok_or_else(|| {
                    DslError::stmt("Opacity must be an integer between 0 and 100")
                })?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.opacity = Some(n.clamp(0, 100));
                }
                Ok(())
            })()
            .into();
        }
        for (token, grammar, message) in [
            (
                tok::ELEMENT_STYLE_WIDTH,
                "width <number>",
                "Width must be a positive integer",
            ),
            (
                tok::ELEMENT_STYLE_HEIGHT,
                "height <number>",
                "Height must be a positive integer",
            ),
            (
                tok::ELEMENT_STYLE_FONT_SIZE,
                "fontSize <number>",
                "Font size must be a positive integer",
            ),
        ] {
            if eqi(&first, token) {
                return (|| {
                    let value = one_token(p, raw, grammar)?;
                    let n = java_int(&value).ok_or_else(|| DslError::stmt(message))?;
                    if let Some(s) = element_style_mut(p, idx) {
                        match token {
                            tok::ELEMENT_STYLE_HEIGHT => s.height = Some(n),
                            tok::ELEMENT_STYLE_FONT_SIZE => s.font_size = Some(n),
                            _ => s.width = Some(n),
                        }
                    }
                    Ok(())
                })()
                .into();
            }
        }
        if eqi(&first, tok::ELEMENT_STYLE_METADATA) {
            return (|| {
                let value = one_token(p, raw, "metadata <true|false>")?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.metadata = Some(tri_bool(&value, "Metadata")?);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_DESCRIPTION) {
            return (|| {
                // The reference grammar text literally says `<true|false>`.
                let value = one_token(p, raw, "description <true|false>")?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.description = Some(tri_bool(&value, "Description")?);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_ICON) {
            return (|| {
                let value = one_token(p, raw, "icon <file|url>")?;
                let data_uri = resolve_icon(p, &value)?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.icon = Some(data_uri);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_ICON_POSITION) {
            return (|| {
                one_grammar(p, raw, "iconPosition", "Top|Bottom|Left")?;
                let name = raw.get(1).to_lowercase();
                let pos = ICON_POSITION_NAMES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| {
                        DslError::stmt(format!("The icon position \"{name}\" is not valid"))
                    })?;
                if let Some(s) = element_style_mut(p, idx) {
                    s.icon_position = Some(pos);
                }
                Ok(())
            })()
            .into();
        }
    }

    // ------------------------------------------------------------------
    // relationship styles (908–947)
    // ------------------------------------------------------------------

    if eqi(&first, tok::RELATIONSHIP_STYLE)
        && let Some(Ctx::Styles { color_scheme }) = top
    {
        return (|| {
            let t = raw.without_context_start_token();
            let tag = style_tag(&t, "relationship")?;
            let idx = get_or_add_relationship_style(p, &tag, color_scheme);
            p.start_context(Ctx::RelationshipStyle { idx });
            Ok(())
        })()
        .into();
    }

    if let Some(Ctx::RelationshipStyle { idx }) = top {
        if eqi(&first, tok::RELATIONSHIP_STYLE_THICKNESS) {
            return (|| {
                let value = one_token(p, raw, "thickness <number>")?;
                let n = java_int(&value)
                    .ok_or_else(|| DslError::stmt("Thickness must be a positive integer"))?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.thickness = Some(n);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_COLOUR) || eqi(&first, tok::RELATIONSHIP_STYLE_COLOR)
        {
            return (|| {
                let value = one_token(p, raw, "colour <#rrggbb|color name>")?;
                let value = style_color(&value)?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.color = Some(value);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_DASHED) {
            return (|| {
                let value = one_token(p, raw, "dashed <true|false>")?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.dashed = Some(tri_bool(&value, "Dashed")?);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_OPACITY) {
            return (|| {
                let value = one_token(p, raw, "opacity <0-100>")?;
                let n = java_int(&value).ok_or_else(|| {
                    DslError::stmt("Opacity must be an integer between 0 and 100")
                })?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.opacity = Some(n.clamp(0, 100));
                }
                Ok(())
            })()
            .into();
        }
        for (token, grammar, message) in [
            (
                tok::ELEMENT_STYLE_WIDTH,
                "width <number>",
                "Width must be a positive integer",
            ),
            (
                tok::ELEMENT_STYLE_FONT_SIZE,
                "fontSize <number>",
                "Font size must be a positive integer",
            ),
        ] {
            if eqi(&first, token) {
                return (|| {
                    let value = one_token(p, raw, grammar)?;
                    let n = java_int(&value).ok_or_else(|| DslError::stmt(message))?;
                    if let Some(s) = relationship_style_mut(p, idx) {
                        if token == tok::ELEMENT_STYLE_FONT_SIZE {
                            s.font_size = Some(n);
                        } else {
                            s.width = Some(n);
                        }
                    }
                    Ok(())
                })()
                .into();
            }
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_POSITION) {
            return (|| {
                let value = one_token(p, raw, "position <0-100>")?;
                let n = java_int(&value).ok_or_else(|| {
                    DslError::stmt("Position must be an integer between 0 and 100")
                })?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.position = Some(n.clamp(0, 100));
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_LINE_STYLE) {
            return (|| {
                one_grammar(p, raw, "style", "solid|dashed|dotted")?;
                let name = raw.get(1).to_lowercase();
                let ls = LINE_STYLE_NAMES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| {
                        DslError::stmt(format!("The line style \"{name}\" is not valid"))
                    })?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.style = Some(ls);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_ROUTING) {
            return (|| {
                one_grammar(p, raw, "routing", "direct|orthogonal|curved")?;
                let name = raw.get(1).to_lowercase();
                let routing = ROUTING_NAMES
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| *v)
                    .ok_or_else(|| {
                        DslError::stmt(format!("The routing \"{name}\" is not valid"))
                    })?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.routing = Some(routing);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::RELATIONSHIP_STYLE_JUMP) {
            return (|| {
                let value = one_token(p, raw, "jump <true|false>")?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.jump = Some(tri_bool(&value, "Jump")?);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_METADATA) {
            return (|| {
                let value = one_token(p, raw, "metadata <true|false>")?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.metadata = Some(tri_bool(&value, "Metadata")?);
                }
                Ok(())
            })()
            .into();
        }
        if eqi(&first, tok::ELEMENT_STYLE_DESCRIPTION) {
            return (|| {
                let value = one_token(p, raw, "description <true|false>")?;
                if let Some(s) = relationship_style_mut(p, idx) {
                    s.description = Some(tri_bool(&value, "Description")?);
                }
                Ok(())
            })()
            .into();
        }
    }

    Sec::No
}

// ----------------------------------------------------------------------
// helpers
// ----------------------------------------------------------------------

/// `new Archetype(identifier, kind)` + `extendArchetype` + `addArchetype`.
fn define_archetype(
    p: &mut Parser<'_>,
    identifier: Option<&str>,
    kind: &str,
    parent_token: &str,
) -> usize {
    p.archetype_list.push(Archetype::new(identifier, kind));
    let idx = p.archetype_list.len() - 1;
    p.extend_archetype(idx, parent_token);
    p.add_archetype(idx);
    idx
}

/// The `ArchetypeDslContext` payload index (element and relationship
/// archetype contexts alike).
fn archetype_idx(ctx: &Ctx) -> Option<usize> {
    match ctx {
        Ctx::ElementArchetype { idx, .. } | Ctx::RelationshipArchetype { idx } => Some(*idx),
        _ => None,
    }
}

/// The `parseXxx(context, tokens)` single-value guard over the RAW tokens:
/// `hasMoreThan(1)` then `includes(1)`, returning token 1.
fn one_token(_p: &Parser<'_>, raw: &Tokens, grammar: &str) -> Result<String, DslError> {
    if raw.has_more_than(1) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {grammar}"
        )));
    }
    if !raw.includes(1) {
        return Err(DslError::stmt(format!("Expected: {grammar}")));
    }
    Ok(raw.get(1))
}

/// The enum-valued statements: same two guards with the enum list already
/// substituted into the grammar (`tokens.get(1)` is read by the caller).
fn one_grammar(_p: &Parser<'_>, raw: &Tokens, keyword: &str, list: &str) -> Result<(), DslError> {
    if raw.has_more_than(1) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {keyword} <{list}>"
        )));
    }
    if !raw.includes(1) {
        return Err(DslError::stmt(format!("Expected: {keyword} <{list}>")));
    }
    Ok(())
}

/// `"true"`/`"false"` case-insensitively; the reference message prefixes
/// with the field name (`Metadata must be true or false`).
fn tri_bool(value: &str, field: &str) -> Result<bool, DslError> {
    if eqi(value, "true") {
        Ok(true)
    } else if eqi(value, "false") {
        Ok(false)
    } else {
        Err(DslError::stmt(format!("{field} must be true or false")))
    }
}

/// `Integer.parseInt`.
fn java_int(value: &str) -> Option<i32> {
    crate::compat::java_parse_int(value)
}

/// Style colour statements run the `Color` setter normalization the Java DSL
/// importer also applies (T4 oracle-verified 2026-11-17 — the same validation
/// JSON hydration does): `#rrggbb` lowercased, HTML name mapped to hex, else
/// the reference `IllegalArgumentException` text as a statement error.
fn style_color(value: &str) -> Result<String, DslError> {
    oxidizr_core::colors::normalize(value)
        .map_err(|msg| DslError::stmt(msg.trim_end_matches('.').to_owned()))
}

/// `ElementStyleParser.parseElementStyle` / `RelationshipStyleParser`
/// front matter: too-many / blank-tag / expected guards, in Java order.
fn style_tag(t: &Tokens, keyword: &str) -> Result<String, DslError> {
    let grammar = format!("{keyword} <tag> {{");
    if t.has_more_than(1) {
        return Err(DslError::stmt(format!(
            "Too many tokens, expected: {grammar}"
        )));
    }
    if t.includes(1) {
        let tag = t.get(1);
        if tag.trim().is_empty() {
            // `StringUtils.isNullOrEmpty` trims.
            return Err(DslError::stmt("A tag must be specified"));
        }
        Ok(tag)
    } else {
        Err(DslError::stmt(format!("Expected: {grammar}")))
    }
}

fn element_style_mut<'a>(
    p: &'a mut Parser<'_>,
    idx: usize,
) -> Option<&'a mut oxidizr_core::views::ElementStyle> {
    p.builder
        .views_mut()
        .configuration
        .styles
        .element_styles
        .get_mut(idx)
}

fn relationship_style_mut<'a>(
    p: &'a mut Parser<'_>,
    idx: usize,
) -> Option<&'a mut oxidizr_core::views::RelationshipStyle> {
    p.builder
        .views_mut()
        .configuration
        .styles
        .relationship_styles
        .get_mut(idx)
}

/// `Styles.getElementStyle(tag, scheme)` … `addElementStyle(tag, scheme)` —
/// reuse the style with the SAME tag and SAME scheme (a scheme-less `styles`
/// block and a `light`/`dark` one are different objects). New styles enter at
/// their `compareTo` position (the reference `TreeSet`, §5).
fn get_or_add_element_style(p: &mut Parser<'_>, tag: &str, scheme: Option<ColorScheme>) -> usize {
    let styles = &mut p.builder.views_mut().configuration.styles;
    if let Some(i) = styles
        .element_styles
        .iter()
        .position(|s| s.tag == tag && s.color_scheme == scheme)
    {
        return i;
    }
    styles.insert_element_style(ElementStyle {
        tag: tag.to_string(),
        color_scheme: scheme,
        ..Default::default()
    })
}

fn get_or_add_relationship_style(
    p: &mut Parser<'_>,
    tag: &str,
    scheme: Option<ColorScheme>,
) -> usize {
    let styles = &mut p.builder.views_mut().configuration.styles;
    if let Some(i) = styles
        .relationship_styles
        .iter()
        .position(|s| s.tag == tag && s.color_scheme == scheme)
    {
        return i;
    }
    styles.insert_relationship_style(oxidizr_core::views::RelationshipStyle {
        tag: tag.to_string(),
        color_scheme: scheme,
        ..Default::default()
    })
}

/// `ElementStyleParser.parseIcon` (see D10 for the raster-file rejection):
/// data URIs and URLs are stored verbatim after validation; SVG files are
/// embedded as raw-bytes base64 (`dslPortable=false`); raster files are
/// rejected because the reference embeds its `ImageIO` re-encode, not the
/// file bytes.
// False positive: the name is explicitly lower-cased below (mirrors the
// reference `name.toLowerCase().endsWith(...)`).
#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn resolve_icon(p: &mut Parser<'_>, path: &str) -> Result<String, DslError> {
    if path.starts_with("data:image/") {
        // `ImageUtils.validateImage` trims before checking; the message
        // carries the TRIMMED descriptor.
        let trimmed = crate::compat::java_trim(path);
        if !(trimmed.starts_with("data:image/png;base64,")
            || trimmed.starts_with("data:image/jpeg;base64,")
            || trimmed.starts_with("data:image/svg+xml;"))
        {
            return Err(DslError::stmt(format!(
                "Only PNG, JPG, and SVG data URIs are supported: {trimmed}"
            )));
        }
        return Ok(path.to_string());
    }
    // `Url.isHttpsUrl`/`isHttpUrl` — the core `Url` port (`URI.create(..)`
    // + JDK protocol handlers, never fetching); the reference stores the
    // URL verbatim, so no `--allow-net` is involved.
    if oxidizr_core::url::is_https_url(path) || oxidizr_core::url::is_http_url(path) {
        return Ok(path.to_string());
    }
    // FILE_SYSTEM branch: relative to the DSL file's parent. A fragment
    // parse (no file) NPEs in the reference — its `getSimpleName()` text.
    let Some(dir) = p.current_dir() else {
        return Err(DslError::stmt("NullPointerException"));
    };
    let file = std::path::Path::new(&dir).join(path);
    let Ok(meta) = file.metadata() else {
        return Err(DslError::stmt(format!("{path} does not exist")));
    };
    if !meta.is_file() {
        // Reference quirk: a DIRECTORY also reports `does not exist`.
        return Err(DslError::stmt(format!("{path} does not exist")));
    }
    let name = file
        .file_name()
        .map_or(String::new(), |n| n.to_string_lossy().into_owned());
    let lower_name = name.to_lowercase();
    if lower_name.ends_with(".svg") {
        let bytes =
            std::fs::read(&file).map_err(|_| DslError::stmt(format!("{path} does not exist")))?;
        // `context.setDslPortable(false)` — file-system content.
        if let Some(frame) = p.frame_portable.last_mut() {
            *frame = false;
        }
        let encoded = super::base64_std(&bytes);
        return Ok(format!("data:image/svg+xml;base64,{encoded}"));
    }
    let raster = [".png", ".jpg", ".jpeg", ".gif"];
    if raster.iter().any(|ext| lower_name.ends_with(ext)) {
        return Err(DslError::coded(
            oxidizr_diag::codes::JVM_ONLY,
            format!(
                "icon {path} is not supported by oxidizr (JVM image codec, D10): embed the image as a data: URI or use SVG"
            ),
        ));
    }
    // `URLConnection.guessContentTypeFromName` misses / non-image.
    let canonical = std::fs::canonicalize(&file).map_or_else(
        |_| file.to_string_lossy().into_owned(),
        |p| p.to_string_lossy().into_owned(),
    );
    Err(DslError::stmt(format!(
        "{canonical} is not a supported image file."
    )))
}

/// `Shape.values()` order — the grammar string and the lowercase lookups.
const SHAPE_NAMES: &[(&str, Shape)] = &[
    ("box", Shape::Box),
    ("roundedbox", Shape::RoundedBox),
    ("circle", Shape::Circle),
    ("ellipse", Shape::Ellipse),
    ("hexagon", Shape::Hexagon),
    ("diamond", Shape::Diamond),
    ("cylinder", Shape::Cylinder),
    ("bucket", Shape::Bucket),
    ("pipe", Shape::Pipe),
    ("person", Shape::Person),
    ("robot", Shape::Robot),
    ("folder", Shape::Folder),
    ("webbrowser", Shape::WebBrowser),
    ("window", Shape::Window),
    ("terminal", Shape::Terminal),
    ("shell", Shape::Shell),
    ("mobiledeviceportrait", Shape::MobileDevicePortrait),
    ("mobiledevicelandscape", Shape::MobileDeviceLandscape),
    ("component", Shape::Component),
];

/// The enum `toString()` concatenation in the shape error text.
const SHAPE_GRAMMAR: &str = "Box|RoundedBox|Circle|Ellipse|Hexagon|Diamond|Cylinder|Bucket|Pipe|Person|Robot|Folder|WebBrowser|Window|Terminal|Shell|MobileDevicePortrait|MobileDeviceLandscape|Component";

/// `Border.values()` (Java order: Solid, Dashed, Dotted).
const BORDER_NAMES: &[(&str, LineStyle)] = &[
    ("solid", LineStyle::Solid),
    ("dashed", LineStyle::Dashed),
    ("dotted", LineStyle::Dotted),
];

/// `LineStyle.values()` (Java order: Dashed, Dotted, Solid — the error-list
/// text is hardcoded `<solid|dashed|dotted>`).
const LINE_STYLE_NAMES: &[(&str, LineStyle)] = &[
    ("dashed", LineStyle::Dashed),
    ("dotted", LineStyle::Dotted),
    ("solid", LineStyle::Solid),
];

/// `Routing.values()` (Java order: Direct, Curved, Orthogonal — the error
/// text lists `<direct|orthogonal|curved>`).
const ROUTING_NAMES: &[(&str, Routing)] = &[
    ("direct", Routing::Direct),
    ("curved", Routing::Curved),
    ("orthogonal", Routing::Orthogonal),
];

/// `IconPosition.values()` (Java order: Top, Bottom, Left).
const ICON_POSITION_NAMES: &[(&str, IconPosition)] = &[
    ("top", IconPosition::Top),
    ("bottom", IconPosition::Bottom),
    ("left", IconPosition::Left),
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::super::{Ctx, ParseOptions, ParseResult, Parser, parse_dsl};
    use oxidizr_core::views::{ColorScheme, IconPosition, LineStyle, Routing, Shape};

    fn full(dsl: &str) -> ParseResult {
        parse_dsl(dsl, ParseOptions::fragment()).expect("parse")
    }

    /// Model-scope harness (mirrors the `stmt_props` tests).
    fn in_model(dsl: &str) -> ParseResult {
        let mut p = Parser::new(ParseOptions::fragment());
        p.start_context(Ctx::Model { group: None });
        let logical = crate::preprocess::preprocess(dsl);
        p.parse_lines(&logical, dsl, false);
        assert_eq!(p.stack.len(), 1, "unbalanced contexts: {:?}", p.errors);
        p.stack.pop().expect("seeded context");
        p.finish()
    }

    /// Wrap a body inside `workspace { views { styles { … } } }`.
    fn styles(body: &str) -> ParseResult {
        full(&format!(
            "workspace {{\n  views {{\n    styles {{\n{body}\n    }}\n  }}\n}}\n"
        ))
    }

    fn errors(r: &ParseResult) -> Vec<String> {
        r.errors.iter().map(|e| e.message.clone()).collect()
    }

    fn assert_clean(r: &ParseResult) {
        assert!(r.errors.is_empty(), "unexpected errors: {:?}", errors(r));
    }

    #[test]
    fn archetype_definitions_apply_and_extend() {
        let r = full(
            r#"
workspace {
  model {
    archetypes {
      application = container {
        tag Application
      }
      springBootApplication = application {
        technology "Spring Boot"
        tags "Spring Boot"
      }
    }
    s = softwareSystem "S" {
      app = springBootApplication "App"
    }
  }
}
"#,
        );
        assert_clean(&r);
        let app = &r.workspace.model.software_systems[0].containers[0];
        assert_eq!(app.technology.as_deref(), Some("Spring Boot"));
        assert_eq!(
            app.item.tags.stored(),
            ["Application", "Spring Boot"],
            "extendArchetype copies the parent tags first"
        );
    }

    #[test]
    fn keyword_named_archetypes_act_as_defaults() {
        // `softwareSystem` / `->` inside `archetypes` register under the
        // keyword name itself, so plain element/relationship statements
        // pick them up (the `archetypes-for-defaults.dsl` fixture shape).
        let r = full(
            r#"
workspace {
  model {
    archetypes {
      softwareSystem {
        description "Default Description"
      }
      -> {
        technology "HTTP"
        tag DefaultRel
      }
    }
    p = person "P"
    s = softwareSystem "S"
    p -> s "Uses"
  }
}
"#,
        );
        assert_clean(&r);
        let w = &r.workspace;
        assert_eq!(
            w.model.software_systems[0].item.description.as_deref(),
            Some("Default Description")
        );
        let rel = &w.model.relationships[0];
        assert_eq!(rel.technology.as_deref(), Some("HTTP"));
        assert_eq!(rel.tags.stored(), ["DefaultRel"]);
    }

    #[test]
    fn group_archetype_brace_opens_nothing() {
        // `g = group {` never pushes a context, so the `}` closes
        // `archetypes` itself — the seeded `Model` frame is all that is
        // left (the harness asserts stack height 1).
        let r = in_model("archetypes {\n  g = group {\n}\n");
        assert_clean(&r);
    }

    #[test]
    fn archetype_tag_statement_quirks() {
        let r = full(
            r#"
workspace {
  model {
    archetypes {
      a1 = person {
        tag "a,b"
        tags "x,,y,"
      }
    }
    p = a1 "P"
  }
}
"#,
        );
        assert_clean(&r);
        assert_eq!(
            r.workspace.model.people[0].item.tags.stored(),
            ["a,b", "x", "", "y"],
            "`tag` is NOT comma-split; `tags` splits on commas, \
             keeping the embedded empty (Java split drops only trailing)"
        );
    }

    #[test]
    fn archetype_statement_errors() {
        let r = full(
            r#"
workspace {
  model {
    archetypes {
      a = person {
        technology "not permitted here"
        description a b
        tag
        tags
      }
    }
  }
}
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                // `technology` has no person-archetype arm — catch-all with
                // the PersonArchetypeDslContext list.
                "Unexpected tokens (expected: description, tag, tags, properties, perspectives)",
                "Too many tokens, expected: description <description>",
                "Expected: tag <tag>",
                "Expected: tags <tags> [tags]",
            ]
        );
    }

    #[test]
    fn views_scope_is_unique() {
        let r = full("workspace {\n  views {\n  }\n  views {\n  }\n}\n");
        assert_eq!(
            errors(&r),
            vec![
                "Multiple view sets are not permitted in a DSL definition",
                // The failed `views` pushed nothing, so the final `}`
                // lands on the empty stack.
                "Unexpected end of context"
            ]
        );
    }

    #[test]
    fn element_style_properties_and_clamps() {
        let r = styles(
            r##"
      element "Tag A" {
        background #112233
        stroke "#445566"
        strokeWidth 20
        opacity -5
        shape RoundedBox
        border DOTTED
        width -3
        height 40
        fontSize 0
        colour white
        metadata true
        description false
        icon "http://example.com/icon.png"
        iconPosition left
      }
"##,
        );
        assert_clean(&r);
        let s = &r.workspace.views.configuration.styles;
        assert_eq!(s.element_styles.len(), 1);
        let st = &s.element_styles[0];
        assert_eq!(st.tag, "Tag A");
        assert_eq!(st.color_scheme, None);
        assert_eq!(st.background.as_deref(), Some("#112233"));
        assert_eq!(st.stroke.as_deref(), Some("#445566"));
        assert_eq!(st.stroke_width, Some(10), "setStrokeWidth clamps to 10");
        assert_eq!(st.opacity, Some(0), "setOpacity clamps to 0");
        assert_eq!(st.shape, Some(Shape::RoundedBox));
        assert_eq!(st.border, Some(LineStyle::Dotted));
        assert_eq!(st.width, Some(-3), "width is NOT clamped");
        assert_eq!(st.height, Some(40));
        assert_eq!(st.font_size, Some(0), "fontSize is NOT clamped");
        // `Color` setter normalization (T4 oracle-verified): names map to hex.
        assert_eq!(st.color.as_deref(), Some("#ffffff"));
        assert_eq!(st.metadata, Some(true));
        assert_eq!(st.description, Some(false));
        assert_eq!(st.icon.as_deref(), Some("http://example.com/icon.png"));
        assert_eq!(st.icon_position, Some(IconPosition::Left));
    }

    #[test]
    fn relationship_style_properties_and_clamps() {
        let r = styles(
            r"
      relationship R {
        thickness 3
        colour green
        dashed true
        opacity 200
        width 5
        fontSize 9
        position -10
        style DASHED
        routing ORTHOGONAL
        jump false
        metadata false
        description true
      }
",
        );
        assert_clean(&r);
        let s = &r.workspace.views.configuration.styles;
        assert_eq!(s.relationship_styles.len(), 1);
        let st = &s.relationship_styles[0];
        assert_eq!(st.tag, "R");
        assert_eq!(st.color_scheme, None);
        assert_eq!(st.thickness, Some(3));
        assert_eq!(st.color.as_deref(), Some("#008000"), "name → hex");
        assert_eq!(st.dashed, Some(true));
        assert_eq!(st.opacity, Some(100), "clamps to 100");
        assert_eq!(st.width, Some(5));
        assert_eq!(st.font_size, Some(9));
        assert_eq!(st.position, Some(0), "clamps to 0");
        assert_eq!(st.style, Some(LineStyle::Dashed));
        assert_eq!(st.routing, Some(Routing::Orthogonal));
        assert_eq!(st.jump, Some(false));
        assert_eq!(st.metadata, Some(false));
        assert_eq!(st.description, Some(true));
    }

    #[test]
    fn styles_are_get_or_add_per_scheme() {
        let r = styles(
            r##"
      element T {
        background "#111111"
      }
      light {
        element T {
          background "#222222"
        }
      }
      dark {
        element T {
          stroke "#333333"
        }
      }
      element T {
        background "#444444"
      }
"##,
        );
        assert_clean(&r);
        let s = &r.workspace.views.configuration.styles;
        assert_eq!(s.element_styles.len(), 3);
        assert_eq!(s.element_styles[0].color_scheme, None);
        assert_eq!(
            s.element_styles[0].background.as_deref(),
            Some("#444444"),
            "the second scheme-less `element T` REUSES and overwrites"
        );
        // `TreeSet` order: null scheme first, then `"scheme/tag"` UTF-16 —
        // so `Dark/T` precedes `Light/T`.
        assert_eq!(s.element_styles[1].color_scheme, Some(ColorScheme::Dark));
        assert_eq!(s.element_styles[1].stroke.as_deref(), Some("#333333"));
        assert_eq!(s.element_styles[2].color_scheme, Some(ColorScheme::Light));
        assert_eq!(s.element_styles[2].background.as_deref(), Some("#222222"));
    }

    #[test]
    fn styles_storage_is_sorted_like_the_reference_treeset() {
        // Reference `Styles` fields are `TreeSet`s: DSL declaration order is
        // replaced by `AbstractStyle.compareTo` (null scheme first, then tag;
        // both-scheme compares `"scheme/tag"`).
        let r = styles(
            r##"
      element Zeta {
        background "#000000"
      }
      element Alpha {
        background "#111111"
      }
      element Mid {
      }
      light {
        element Aardvark {
        }
      }
    "##,
        );
        assert_clean(&r);
        let s = &r.workspace.views.configuration.styles;
        let keys: Vec<(&str, Option<ColorScheme>)> = s
            .element_styles
            .iter()
            .map(|x| (x.tag.as_str(), x.color_scheme))
            .collect();
        assert_eq!(
            keys,
            vec![
                ("Alpha", None),
                ("Mid", None),
                ("Zeta", None),
                ("Aardvark", Some(ColorScheme::Light)),
            ]
        );
    }

    #[test]
    fn style_definition_guards() {
        // No `{`: a failed definition pushes no context, so the braces
        // still balance.
        let r = styles("      element A B\n      relationship\n      element \" \"\n");
        assert_eq!(
            errors(&r),
            vec![
                "Too many tokens, expected: element <tag> {",
                "Expected: relationship <tag> {",
                "A tag must be specified",
            ]
        );
        assert!(
            r.workspace
                .views
                .configuration
                .styles
                .element_styles
                .is_empty()
                && r.workspace
                    .views
                    .configuration
                    .styles
                    .relationship_styles
                    .is_empty()
        );
    }

    #[test]
    fn element_style_property_errors() {
        let r = styles(
            r#"
      element T {
        strokeWidth x
        opacity y
        shape zigzag
        border zig
        width z
        fontSize z
        metadata maybe
        description maybe
        icon "data:image/gif;base64,AAA"
        iconPosition zig
      }
"#,
        );
        assert_eq!(
            errors(&r),
            vec![
                "Stroke width must be an integer between 1 and 10",
                "Opacity must be an integer between 0 and 100",
                "The shape \"zigzag\" is not valid",
                "The border \"zig\" is not valid",
                "Width must be a positive integer",
                "Font size must be a positive integer",
                "Metadata must be true or false",
                "Description must be true or false",
                "Only PNG, JPG, and SVG data URIs are supported: data:image/gif;base64,AAA",
                "The icon position \"zig\" is not valid",
            ]
        );
    }

    #[test]
    fn element_style_grammar_texts() {
        let r = styles(
            r"
      element T {
        shape
        border a b
        strokeWidth
        icon
      }
      relationship R {
        thickness
        position a b
      }
",
        );
        let shape_list = "Box|RoundedBox|Circle|Ellipse|Hexagon|Diamond|Cylinder|\
             Bucket|Pipe|Person|Robot|Folder|WebBrowser|Window|Terminal|Shell|\
             MobileDevicePortrait|MobileDeviceLandscape|Component";
        assert_eq!(
            errors(&r),
            vec![
                format!("Expected: shape <{shape_list}>"),
                "Too many tokens, expected: border <solid|dashed|dotted>".to_string(),
                "Expected: strokeWidth <1-10>".to_string(),
                "Expected: icon <file|url>".to_string(),
                "Expected: thickness <number>".to_string(),
                "Too many tokens, expected: position <0-100>".to_string(),
            ]
        );
    }

    #[test]
    fn relationship_style_property_errors() {
        let r = styles(
            r"
      relationship R {
        style zig
        routing zig
        position x
        thickness y
        dashed z
        jump q
        nonsense x
      }
",
        );
        assert_eq!(
            errors(&r),
            vec![
                "The line style \"zig\" is not valid",
                "The routing \"zig\" is not valid",
                "Position must be an integer between 0 and 100",
                "Thickness must be a positive integer",
                "Dashed must be true or false",
                "Jump must be true or false",
                // Java's RelationshipStyleDslContext token list (note: no
                // dashed/jump/metadata/description despite the chain arms).
                "Unexpected tokens (expected: thickness, color, colour, style, \
                 routing, fontSize, width, position, opacity, properties)",
            ]
        );
    }

    #[test]
    fn element_style_catch_all_list() {
        let r = styles("      element T {\n        nonsense x\n      }\n");
        assert_eq!(
            errors(&r),
            vec![
                "Unexpected tokens (expected: shape, icon, iconPosition, width, height, \
                 background, color, colour, stroke, strokeWidth, fontSize, border, \
                 opacity, metadata, description, properties)"
            ]
        );
    }

    #[test]
    fn style_properties_block_attaches_to_the_style() {
        let r =
            styles("      element T {\n        properties {\n          k v\n        }\n      }\n");
        assert_clean(&r);
        let st = &r.workspace.views.configuration.styles.element_styles[0];
        assert_eq!(st.properties.get("k"), Some("v"));
    }

    #[test]
    fn icon_data_uri_and_url_stored_verbatim() {
        let r = styles(
            r#"
      element T {
        icon "data:image/png;base64,AA=="
        icon "data:image/svg+xml;utf8,<svg/>"
        icon "https://example.com/x.svg"
      }
"#,
        );
        assert_clean(&r);
        // Last write wins.
        assert_eq!(
            r.workspace.views.configuration.styles.element_styles[0]
                .icon
                .as_deref(),
            Some("https://example.com/x.svg")
        );
    }

    #[test]
    fn icon_fragment_file_reference_npes() {
        // A fragment has no `dslFile` — Java NPEs in
        // `context.getFile().getParent()` and the chain's catch renders
        // `getSimpleName()`.
        let r = styles("      element T {\n        icon \"logo.svg\"\n      }\n");
        assert_eq!(errors(&r), vec!["NullPointerException"]);
    }

    #[test]
    fn icon_files_resolve_against_the_dsl_file() {
        let dir = std::env::temp_dir().join(format!("oxidizr-icon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tempdir");
        std::fs::write(dir.join("oxz-logo.svg"), b"<svg/>").expect("svg");
        std::fs::write(dir.join("oxz-logo.png"), b"not really a png").expect("png");
        std::fs::write(dir.join("oxz-notes.txt"), b"hello").expect("txt");
        let dsl = dir.join("oxz-icon.ws.dsl");

        let parse = |icon: &str| {
            let src = format!(
                "workspace {{\n  views {{\n    styles {{\n      element T {{\n        icon \"{icon}\"\n      }}\n    }}\n  }}\n}}\n"
            );
            let r = parse_dsl(&src, ParseOptions::file(dsl.to_str().unwrap())).expect("parse");
            let err = r.errors.first().map(|e| (e.message.clone(), e.code));
            (r, err)
        };

        let (r, err) = parse("oxz-logo.svg");
        assert!(err.is_none(), "unexpected: {err:?}");
        assert_eq!(
            r.workspace.views.configuration.styles.element_styles[0]
                .icon
                .as_deref(),
            Some("data:image/svg+xml;base64,PHN2Zy8+"),
            "SVG embeds as raw-bytes base64"
        );
        assert!(
            !r.workspace.properties.contains_key("structurizr.dsl"),
            "icon <file> clears dslPortable, dropping the source property"
        );

        // D10: raster files are rejected (the reference embeds its ImageIO
        // re-encode, which pure Rust std cannot reproduce).
        let (_, err) = parse("oxz-logo.png");
        let (message, code) = err.expect("png rejected");
        assert_eq!(code, oxidizr_diag::codes::JVM_ONLY);
        assert!(message.contains("D10"), "message names D10: {message}");

        // Non-image file: the `guessContentTypeFromName` miss, canonical
        // path, trailing period (the reference's IAE text).
        let (_, err) = parse("oxz-notes.txt");
        let (message, _) = err.expect("txt rejected");
        assert!(
            message.ends_with("oxz-notes.txt is not a supported image file."),
            "unexpected: {message}"
        );

        // Missing file (or directory): as-given path, no period.
        let (_, err) = parse("oxz-missing.png");
        let (message, _) = err.expect("missing rejected");
        assert_eq!(message, "oxz-missing.png does not exist");
    }
}
