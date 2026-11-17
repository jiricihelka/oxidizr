//! Style resolution (`view/Styles`): the single effective-style computation
//! (P2-D2, `agents/plan/build/PHASE_2.md`) — exporters and the Phase 3 GUI
//! render through these functions, never through their own copies.
//!
//! Ported from `structurizr-core` `view/Styles.java` at the pin: the
//! `findElementStyle`/`findRelationshipStyle` pairs, the default styles, the
//! `ElementStyle`/`RelationshipStyle` `copyFrom` semantics, and
//! `addStylesFromTheme` (which appends to the NON-serialized `themes` list —
//! theme styles are consulted live at resolution time, never merged into
//! `elements`/`relationships`). Contract catalog: `EXPORT-PROPERTIES.md` §6–§7.

use crate::model::{Model, Relationship};
use crate::tags::{relationship_defaults, tag};
use crate::views::{ColorScheme, ElementStyle, RelationshipStyle, Styles, Theme};

/// `Styles.DEFAULT_BACKGROUND_LIGHT`.
pub const DEFAULT_BACKGROUND_LIGHT: &str = "#ffffff";
/// `Styles.DEFAULT_COLOR_LIGHT`.
pub const DEFAULT_COLOR_LIGHT: &str = "#444444";
/// `Styles.DEFAULT_BACKGROUND_DARK`.
pub const DEFAULT_BACKGROUND_DARK: &str = "#111111";
/// `Styles.DEFAULT_COLOR_DARK`.
pub const DEFAULT_COLOR_DARK: &str = "#cccccc";

impl Styles {
    /// `Styles.addStylesFromTheme` — appends; `find_*` then consults the
    /// theme's styles BEFORE the workspace styles. Never serialized.
    pub fn add_styles_from_theme(&mut self, theme: Theme) {
        self.themes.push(theme);
    }

    /// `Styles.findElementStyle(String, ColorScheme)`: an empty style for the
    /// (trimmed) tag, `copyFrom`-stacked over every style whose tag matches —
    /// theme styles (add order) first, workspace styles last — accepting a
    /// style when its `colorScheme` is null or equals `color_scheme`.
    /// `None` when nothing matched.
    #[must_use]
    pub fn find_element_style(&self, tag: &str, color_scheme: ColorScheme) -> Option<ElementStyle> {
        let tag = tag.trim();
        let mut style = ElementStyle {
            tag: tag.to_string(),
            ..ElementStyle::default()
        };
        let mut exists = false;
        for source in self
            .themes
            .iter()
            .flat_map(|t| &t.elements)
            .chain(&self.element_styles)
        {
            if source.tag == tag
                && source
                    .color_scheme
                    .is_none_or(|scheme| scheme == color_scheme)
            {
                exists = true;
                copy_from_element(&mut style, source);
            }
        }
        exists.then_some(style)
    }

    /// `Styles.findRelationshipStyle(String, ColorScheme)` —
    /// [`Styles::find_element_style`] for relationship styles.
    #[must_use]
    pub fn find_relationship_style(
        &self,
        tag: &str,
        color_scheme: ColorScheme,
    ) -> Option<RelationshipStyle> {
        let tag = tag.trim();
        let mut style = RelationshipStyle {
            tag: tag.to_string(),
            ..RelationshipStyle::default()
        };
        let mut exists = false;
        for source in self
            .themes
            .iter()
            .flat_map(|t| &t.relationships)
            .chain(&self.relationship_styles)
        {
            if source.tag == tag
                && source
                    .color_scheme
                    .is_none_or(|scheme| scheme == color_scheme)
            {
                exists = true;
                copy_from_relationship(&mut style, source);
            }
        }
        exists.then_some(style)
    }

    /// `Styles.findElementStyle(Element, ColorScheme)` — the default style
    /// (`Box`, fontSize 24, `Solid`, opacity 100, metadata+description)
    /// stacked over each tag of the element's FULL tag chain, then the
    /// scheme's background/stroke/color defaults. `tags` is
    /// `element.getTags()` as CSV (`Tags::serialized_for`); for
    /// `SoftwareSystemInstance`/`ContainerInstance` the reference prepends the
    /// parent element's tags — build the chain with [`instance_tag_chain`].
    ///
    /// The result's `tag` is the comma-joined chain of the tags that actually
    /// matched (`tagsUsedToComposeStyle`), starting at `Element`.
    #[must_use]
    pub fn element_style(&self, tags: &str, color_scheme: ColorScheme) -> ElementStyle {
        let mut style = ElementStyle {
            tag: tag::ELEMENT.to_string(),
            shape: Some(crate::views::Shape::Box),
            font_size: Some(24),
            border: Some(crate::views::LineStyle::Solid),
            opacity: Some(100),
            metadata: Some(true),
            description: Some(true),
            ..ElementStyle::default()
        };
        let mut used = vec![tag::ELEMENT.to_string()];
        for t in tags.split(',') {
            // `StringUtils.isNullOrEmpty` trims: whitespace-only parts skip.
            if t.trim().is_empty() {
                continue;
            }
            if let Some(matched) = self.find_element_style(t, color_scheme) {
                copy_from_element(&mut style, &matched);
                // LinkedHashSet: first position kept, re-adds are no-ops.
                if !used.contains(&matched.tag) {
                    used.push(matched.tag.clone());
                }
            }
        }
        style.tag = used.join(","); // TagUtils.toString (LinkedHashSet order)

        if let Some(background) = style.background.clone() {
            if style.stroke.is_none() {
                // `Color.decode(background).darker()` formatted `#%06X`.
                // A decode miss throws in the reference; oxidizr leaves the
                // stroke unset (deviation note in PROGRESS, pathological
                // malformed-color input).
                style.stroke = java_darker_hex(&background);
            }
        } else if color_scheme == ColorScheme::Dark {
            style.background = Some(DEFAULT_BACKGROUND_DARK.to_string());
            if style.stroke.is_none() {
                style.stroke = Some(DEFAULT_COLOR_DARK.to_string());
            }
        } else {
            style.background = Some(DEFAULT_BACKGROUND_LIGHT.to_string());
            if style.stroke.is_none() {
                style.stroke = Some(DEFAULT_COLOR_LIGHT.to_string());
            }
        }

        if style.color.is_none() {
            style.color = Some(
                (if color_scheme == ColorScheme::Dark {
                    DEFAULT_COLOR_DARK
                } else {
                    DEFAULT_COLOR_LIGHT
                })
                .to_string(),
            );
        }
        style
    }

    /// `Styles.findRelationshipStyle(Relationship, ColorScheme)` — the
    /// default style (thickness 2, `Dashed`+dashed, `Direct`, fontSize 24,
    /// width 200, position 50, opacity 100) stacked over the relationship's
    /// tag chain — linked relationships PREPEND the tags of every linked
    /// relationship, walking `linkedRelationshipId` transitively — then the
    /// scheme's color default. `tag` is the matched-tag chain, starting at
    /// `Relationship`.
    ///
    /// A dangling `linked_relationship_id` breaks the chain (the reference
    /// NPEs; oxidizr never panics on workspace data).
    #[must_use]
    pub fn relationship_style(
        &self,
        relationship: &Relationship,
        model: &Model,
        color_scheme: ColorScheme,
    ) -> RelationshipStyle {
        let mut style = RelationshipStyle {
            tag: tag::RELATIONSHIP.to_string(),
            thickness: Some(2),
            style: Some(crate::views::LineStyle::Dashed),
            dashed: Some(true),
            routing: Some(crate::views::Routing::Direct),
            font_size: Some(24),
            width: Some(200),
            position: Some(50),
            opacity: Some(100),
            ..RelationshipStyle::default()
        };
        let mut tags = relationship.tags.serialized(&relationship_defaults(
            relationship.linked_relationship_id.is_some(),
            relationship.interaction_style,
        ));
        let mut linked = relationship.linked_relationship_id.clone();
        while let Some(id) = linked.take() {
            let Some(linked_rel) = model.relationships.iter().find(|r| r.id == id) else {
                break;
            };
            let linked_tags = linked_rel.tags.serialized(&relationship_defaults(
                linked_rel.linked_relationship_id.is_some(),
                linked_rel.interaction_style,
            ));
            tags = format!("{linked_tags},{tags}");
            linked.clone_from(&linked_rel.linked_relationship_id);
        }

        let mut used = vec![tag::RELATIONSHIP.to_string()];
        for t in tags.split(',') {
            if t.trim().is_empty() {
                continue;
            }
            if let Some(matched) = self.find_relationship_style(t, color_scheme) {
                copy_from_relationship(&mut style, &matched);
                // LinkedHashSet: first position kept, re-adds are no-ops.
                if !used.contains(&matched.tag) {
                    used.push(matched.tag.clone());
                }
            }
        }
        style.tag = used.join(",");

        if style.color.is_none() {
            style.color = Some(
                (if color_scheme == ColorScheme::Dark {
                    DEFAULT_COLOR_DARK
                } else {
                    DEFAULT_COLOR_LIGHT
                })
                .to_string(),
            );
        }
        style
    }
}

/// The instance prepending `findElementStyle` does: `ss.getTags() + "," +
/// tags` (`SoftwareSystemInstance`) / `c.getTags() + "," + tags`
/// (`ContainerInstance`) — the parent chain FIRST.
#[must_use]
pub fn instance_tag_chain(parent_tags: &str, instance_tags: &str) -> String {
    format!("{parent_tags},{instance_tags}")
}

/// `ElementStyle.copyFrom` (`ElementStyle.java` 391–448): numbers/booleans/
/// enums copy when non-null, colors and icon only when non-empty, properties
/// merged in iteration order (`addProperty` = `HashMap.put`). Field order is
/// the Java method's — it fixes the merged property-map order.
fn copy_from_element(dst: &mut ElementStyle, src: &ElementStyle) {
    if src.width.is_some() {
        dst.width = src.width;
    }
    if src.height.is_some() {
        dst.height = src.height;
    }
    if is_non_empty(src.background.as_deref()) {
        dst.background.clone_from(&src.background);
    }
    if is_non_empty(src.stroke.as_deref()) {
        dst.stroke.clone_from(&src.stroke);
    }
    if src.stroke_width.is_some() {
        dst.stroke_width = src.stroke_width;
    }
    if is_non_empty(src.color.as_deref()) {
        dst.color.clone_from(&src.color);
    }
    if src.font_size.is_some() {
        dst.font_size = src.font_size;
    }
    if src.shape.is_some() {
        dst.shape = src.shape;
    }
    if is_non_empty(src.icon.as_deref()) {
        dst.icon.clone_from(&src.icon);
    }
    if src.icon_position.is_some() {
        dst.icon_position = src.icon_position;
    }
    if src.border.is_some() {
        dst.border = src.border;
    }
    if src.opacity.is_some() {
        dst.opacity = src.opacity;
    }
    if src.metadata.is_some() {
        dst.metadata = src.metadata;
    }
    if src.description.is_some() {
        dst.description = src.description;
    }
    for (name, value) in src.properties.iter() {
        dst.properties.insert(name, value);
    }
}

/// `RelationshipStyle.copyFrom` (`RelationshipStyle.java` 275–327).
fn copy_from_relationship(dst: &mut RelationshipStyle, src: &RelationshipStyle) {
    if src.thickness.is_some() {
        dst.thickness = src.thickness;
    }
    if is_non_empty(src.color.as_deref()) {
        dst.color.clone_from(&src.color);
    }
    if src.dashed.is_some() {
        dst.dashed = src.dashed;
    }
    if src.style.is_some() {
        dst.style = src.style;
    }
    if src.routing.is_some() {
        dst.routing = src.routing;
    }
    if src.jump.is_some() {
        dst.jump = src.jump;
    }
    if src.font_size.is_some() {
        dst.font_size = src.font_size;
    }
    if src.width.is_some() {
        dst.width = src.width;
    }
    if src.position.is_some() {
        dst.position = src.position;
    }
    if src.opacity.is_some() {
        dst.opacity = src.opacity;
    }
    if src.metadata.is_some() {
        dst.metadata = src.metadata;
    }
    if src.description.is_some() {
        dst.description = src.description;
    }
    for (name, value) in src.properties.iter() {
        dst.properties.insert(name, value);
    }
}

/// `StringUtils.isNullOrEmpty` (structur-util: trims — whitespace-only IS
/// empty; `copyFrom`'s guard for the color/icon strings).
fn is_non_empty(value: Option<&str>) -> bool {
    value.is_some_and(|v| !v.trim().is_empty())
}

/// `String.format("#%06X", 0xFFFFFF & java.awt.Color.decode(s).darker()
/// .getRGB())` — the unset-stroke default (`Styles.java` 317–321). Decode
/// accepts Java's `Color.decode` forms (`#hex`, `0xhex`, decimal int); `None`
/// where the reference throws.
fn java_darker_hex(s: &str) -> Option<String> {
    let v = java_color_decode(s)?;
    let darker = |c: u32| darker_channel(c) & 0xFF;
    // The reference formats `#%06X` (upper) but assigns via
    // `ElementStyle.setStroke`, which lowercases every hex code — net:
    // lower-case storage (M2-d A/B: `#944a4a`).
    Some(format!(
        "#{:06x}",
        (darker((v >> 16) & 0xFF) << 16) | (darker((v >> 8) & 0xFF) << 8) | darker(v & 0xFF)
    ))
}

/// `java.awt.Color.darker()`: `(int)(channel * 0.7)` — Java multiplies in
/// IEEE-754 double, so the f64 op IS the contract; the truncation is intended.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn darker_channel(c: u32) -> u32 {
    (f64::from(c) * 0.7) as u32
}

/// `java.awt.Color.decode(String)` as a packed `0xRRGGBB`, restricted to the
/// forms styles can carry (`#rrggbb`, `0xhex`/`0Xhex`, decimal int). The
/// reference's legacy restart-on-bad-hex-digit and `.`-truncation paths are
/// unreachable from validated style colors; negatives and malformed values
/// throw there → `None` here.
fn java_color_decode(s: &str) -> Option<u32> {
    let (digits, radix) = match s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .or_else(|| s.strip_prefix('#'))
    {
        Some(hex) => (hex, 16),
        None => (s, 10),
    };
    if digits.is_empty() || s.starts_with('-') {
        return None;
    }
    let mut value: u64 = 0;
    for c in digits.chars() {
        let d = u64::from(c.to_digit(radix)?);
        value = value.checked_mul(u64::from(radix))?.checked_add(d)?;
        if value > 0xFFFF_FFFF {
            return None; // outside the RGB range the reference checks
        }
    }
    if value > 0xFF_FFFF {
        value >>= 24; // decode's legacy "first byte triple" shift
    }
    u32::try_from(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::Id;
    use crate::tags::Tags;
    use crate::views::{LineStyle, Shape};

    fn element_style(tag: &str, color_scheme: Option<ColorScheme>) -> ElementStyle {
        ElementStyle {
            tag: tag.into(),
            color_scheme,
            ..ElementStyle::default()
        }
    }

    fn rel_style(tag: &str) -> RelationshipStyle {
        RelationshipStyle {
            tag: tag.into(),
            ..RelationshipStyle::default()
        }
    }

    #[test]
    fn find_element_style_none_without_match_some_with() {
        let styles = Styles::default();
        assert!(
            styles
                .find_element_style("Person", ColorScheme::Light)
                .is_none()
        );
        let styles = Styles {
            element_styles: vec![ElementStyle {
                background: Some("#f0f0f0".into()),
                ..element_style("Person", None)
            }],
            ..Styles::default()
        };
        let found = styles
            .find_element_style(" Person ", ColorScheme::Dark)
            .expect("trimmed tag matches, null scheme accepts any");
        assert_eq!(found.tag, "Person");
        assert_eq!(found.background.as_deref(), Some("#f0f0f0"));
    }

    #[test]
    fn scheme_mismatch_rejects_and_later_wins() {
        let mut styles = Styles::default();
        styles.element_styles.push(ElementStyle {
            background: Some("#light".into()),
            color_scheme: Some(ColorScheme::Light),
            ..element_style("Person", Some(ColorScheme::Light))
        });
        styles.element_styles.push(ElementStyle {
            background: Some("#dark".into()),
            ..element_style("Person", Some(ColorScheme::Dark))
        });
        assert_eq!(
            styles
                .find_element_style("Person", ColorScheme::Dark)
                .expect("dark")
                .background
                .as_deref(),
            Some("#dark")
        );
        // Same-tag stacking: a second null-scheme rule overrides the first.
        let stacked = Styles {
            element_styles: vec![
                ElementStyle {
                    font_size: Some(20),
                    background: Some("#first".into()),
                    ..element_style("T", None)
                },
                ElementStyle {
                    background: Some("#second".into()),
                    ..element_style("T", None)
                },
            ],
            ..Styles::default()
        };
        let s = stacked
            .find_element_style("T", ColorScheme::Light)
            .expect("match");
        assert_eq!(s.background.as_deref(), Some("#second"));
        assert_eq!(s.font_size, Some(20)); // untouched field survives
    }

    #[test]
    fn theme_styles_come_first_workspace_wins() {
        let mut styles = Styles::default();
        styles.add_styles_from_theme(Theme {
            elements: vec![ElementStyle {
                background: Some("#theme".into()),
                color: Some("#theme-color".into()),
                ..element_style("Person", None)
            }],
            ..Theme::default()
        });
        styles.element_styles.push(ElementStyle {
            background: Some("#ws".into()),
            ..element_style("Person", None)
        });
        let s = styles
            .find_element_style("Person", ColorScheme::Light)
            .expect("match");
        assert_eq!(s.background.as_deref(), Some("#ws")); // workspace overrides
        assert_eq!(s.color.as_deref(), Some("#theme-color")); // theme fills gaps
    }

    #[test]
    fn element_style_defaults_light_and_dark() {
        let styles = Styles::default();
        let s = styles.element_style("Element,Person", ColorScheme::Light);
        assert_eq!(s.tag, "Element"); // nothing matched the chain
        assert_eq!(s.shape, Some(Shape::Box));
        assert_eq!(s.font_size, Some(24));
        assert_eq!(s.border, Some(LineStyle::Solid));
        assert_eq!(s.opacity, Some(100));
        assert_eq!(s.metadata, Some(true));
        assert_eq!(s.description, Some(true));
        assert_eq!(s.background.as_deref(), Some(DEFAULT_BACKGROUND_LIGHT));
        assert_eq!(s.stroke.as_deref(), Some(DEFAULT_COLOR_LIGHT));
        assert_eq!(s.color.as_deref(), Some(DEFAULT_COLOR_LIGHT));
        let d = styles.element_style("Element", ColorScheme::Dark);
        assert_eq!(d.background.as_deref(), Some(DEFAULT_BACKGROUND_DARK));
        assert_eq!(d.stroke.as_deref(), Some(DEFAULT_COLOR_DARK));
        assert_eq!(d.color.as_deref(), Some(DEFAULT_COLOR_DARK));
    }

    #[test]
    fn element_style_chain_order_and_matched_tag_chain() {
        let mut styles = Styles::default();
        styles.element_styles.push(ElementStyle {
            background: Some("#eeeeee".into()),
            font_size: Some(30),
            ..element_style("Element", None)
        });
        styles.element_styles.push(ElementStyle {
            background: Some("#ff0000".into()),
            ..element_style("Person", None)
        });
        styles.element_styles.push(ElementStyle {
            color: Some("#00ff00".into()),
            ..element_style("Mine", None)
        });
        let s = styles.element_style("Element,Person,Mine", ColorScheme::Light);
        assert_eq!(s.tag, "Element,Person,Mine");
        assert_eq!(s.background.as_deref(), Some("#ff0000")); // later tag wins
        assert_eq!(s.font_size, Some(30)); // Element rule survives
        assert_eq!(s.color.as_deref(), Some("#00ff00"));
        assert_eq!(s.stroke.as_deref(), Some("#b20000")); // darker(win bg)
        // whitespace-only chain parts skip (StringUtils.isNullOrEmpty trims)
        let s = styles.element_style("Element,  ,Person", ColorScheme::Light);
        assert_eq!(s.tag, "Element,Person");
    }

    #[test]
    fn unset_stroke_is_darker_of_background() {
        let styles = Styles {
            element_styles: vec![ElementStyle {
                background: Some("#ff0000".into()),
                ..element_style("Person", None)
            }],
            ..Styles::default()
        };
        let s = styles.element_style("Element,Person", ColorScheme::Light);
        assert_eq!(s.stroke.as_deref(), Some("#b20000")); // (255*0.7, 0, 0)
        // explicit stroke survives
        let styles = Styles {
            element_styles: vec![ElementStyle {
                background: Some("#ff0000".into()),
                stroke: Some("#123456".into()),
                ..element_style("Person", None)
            }],
            ..Styles::default()
        };
        assert_eq!(
            styles
                .element_style("Person", ColorScheme::Light)
                .stroke
                .as_deref(),
            Some("#123456")
        );
    }

    #[test]
    fn java_color_decode_edge_forms() {
        assert_eq!(java_color_decode("#ffffff"), Some(0xFF_FFFF));
        assert_eq!(java_color_decode("0xff0000"), Some(0xFF_0000));
        assert_eq!(java_color_decode("16711680"), Some(0xFF_0000)); // decimal
        assert_eq!(java_color_decode("#fff"), Some(0x00_0FFF)); // decode is hex, not 3-digit
        assert_eq!(java_color_decode("16777216"), Some(1)); // > 0xFFFFFF shifts >>24
        assert_eq!(java_color_decode("-0xff"), None); // negatives throw
        assert_eq!(java_color_decode("#f.g"), None);
        assert_eq!(java_color_decode(""), None);
        assert_eq!(java_darker_hex("#000000").as_deref(), Some("#000000"));
    }

    #[test]
    fn relationship_defaults_and_linked_chain() {
        let styles = Styles::default();
        let model = Model::default();
        let rel = Relationship {
            id: Id::new("1"),
            source: Id::new("2"),
            destination: Id::new("3"),
            description: None,
            technology: None,
            tags: Tags::new(),
            url: None,
            properties: crate::properties::Properties::new(),
            perspectives: Vec::new(),
            interaction_style: None,
            linked_relationship_id: None,
        };
        let s = styles.relationship_style(&rel, &model, ColorScheme::Light);
        assert_eq!(s.tag, "Relationship");
        assert_eq!(s.thickness, Some(2));
        assert_eq!(s.style, Some(LineStyle::Dashed));
        assert_eq!(s.dashed, Some(true));
        assert_eq!(s.routing, Some(crate::views::Routing::Direct));
        assert_eq!(s.font_size, Some(24));
        assert_eq!(s.width, Some(200));
        assert_eq!(s.position, Some(50));
        assert_eq!(s.opacity, Some(100));
        assert_eq!(s.color.as_deref(), Some(DEFAULT_COLOR_LIGHT));

        // Linked relationship: the linked tags are PREPENDED, walking the chain.
        let linked = Relationship {
            id: Id::new("10"),
            linked_relationship_id: Some(Id::new("11")),
            tags: Tags::set_from_csv("Linked"),
            ..rel.clone()
        };
        let base = Relationship {
            id: Id::new("11"),
            tags: Tags::set_from_csv("Base"),
            ..rel
        };
        let styles = Styles {
            relationship_styles: vec![
                RelationshipStyle {
                    color: Some("#base".into()),
                    ..rel_style("Base")
                },
                RelationshipStyle {
                    color: Some("#linked".into()),
                    thickness: Some(4),
                    ..rel_style("Linked")
                },
            ],
            ..Styles::default()
        };
        let model = Model {
            relationships: vec![linked.clone(), base],
            ..Model::default()
        };
        let s = styles.relationship_style(&linked, &model, ColorScheme::Light);
        assert_eq!(s.color.as_deref(), Some("#linked")); // OWN tags compose last
        assert_eq!(s.thickness, Some(4));
        assert_eq!(s.tag, "Relationship,Base,Linked"); // linked chain PREPENDED

        // Dangling linked id: chain breaks, no panic (reference NPEs).
        let dangling = Model::default();
        let s = styles.relationship_style(&linked, &dangling, ColorScheme::Light);
        assert_eq!(s.color.as_deref(), Some("#linked"));
        assert_eq!(s.tag, "Relationship,Linked");
    }

    #[test]
    fn copy_from_skips_null_and_blank_colors() {
        let mut dst = ElementStyle {
            tag: "T".into(),
            ..ElementStyle::default()
        };
        let src = ElementStyle {
            background: Some("   ".into()), // whitespace-only: isNullOrEmpty TRIMS → skipped
            color: Some(String::new()),     // empty: skipped
            stroke_width: Some(3),          // numbers copy whatever they are
            opacity: Some(0),
            ..element_style("T", None)
        };
        copy_from_element(&mut dst, &src);
        assert_eq!(dst.background, None);
        assert_eq!(dst.color, None);
        assert_eq!(dst.stroke_width, Some(3));
        assert_eq!(dst.opacity, Some(0));
    }

    #[test]
    fn properties_merge_in_iteration_order() {
        let mut dst = ElementStyle {
            tag: "T".into(),
            ..ElementStyle::default()
        };
        dst.properties.insert("a", "1");
        let mut src = ElementStyle {
            tag: "T".into(),
            ..ElementStyle::default()
        };
        src.properties.insert("a", "2");
        src.properties.insert("b", "3");
        copy_from_element(&mut dst, &src);
        assert_eq!(
            dst.properties.iter().collect::<Vec<_>>(),
            vec![("a", "2"), ("b", "3")]
        );
    }
}
