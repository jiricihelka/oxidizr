//! Dynamic-view content arms of the view sections: the `dynamic_step`
//! relationship/element-pair parser and its identifier resolution. The
//! definition, content, animation and configuration arms live in the
//! sibling modules; the shared free helpers stay in the parent module.

use oxidizr_core::validate::ElementKind;
use oxidizr_core::{Id, ViewRef};

use super::super::idents::RegItem;
use super::{Ctx, DslError, Parser, Tokens, expected, is_trim_empty, tok, too_many};

// ---------------------------------------------------------------------------
// dynamic view content
// ---------------------------------------------------------------------------

pub(super) fn dynamic_step(
    p: &mut Parser,
    raw: &Tokens,
    view: ViewRef,
    open: bool,
) -> Result<(), DslError> {
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
