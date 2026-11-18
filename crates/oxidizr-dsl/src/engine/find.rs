//! Ports of `FindElementParser`, `FindRelationshipParser`,
//! `FindElementsParser` and `FindRelationshipsParser` (`ad34036`).

use oxidizr_core::Id;
use oxidizr_core::validate::ElementKind;

use super::ctx::Ctx;
use super::error::DslError;
use super::expr::Mi;
use super::idents::RegItem;
use super::{Parser, Tokens};

impl Parser<'_> {
    /// `FindElementParser.parse` — identifier OR canonical name; the
    /// groupable-context group side effect included.
    pub(crate) fn find_element(&mut self, tokens: &Tokens) -> Result<(RegItem, String), DslError> {
        const GRAMMAR: &str = "!element <identifier|canonical name>";
        if tokens.has_more_than(1) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {GRAMMAR}"
            )));
        }
        if !tokens.includes(1) {
            return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
        }
        let s = tokens.get(1);
        let item = if s.contains("://") {
            self.builder
                .element_with_canonical_name(&s)
                .map(RegItem::Element)
        } else {
            self.get_element(&s)
        };
        let Some(item) = item else {
            return Err(DslError::stmt(format!(
                "An element identified by \"{s}\" could not be found"
            )));
        };
        // `context instanceof GroupableDslContext && element instanceof
        // StaticStructureElement` — instances and custom elements are
        // excluded (their contexts/element classes are not groupable /
        // static-structure resp.).
        if let (true, RegItem::Element(id)) =
            (self.top().is_some_and(super::ctx::Ctx::is_groupable), &item)
            && matches!(
                self.builder.element_kind(id),
                Some(
                    ElementKind::Person
                        | ElementKind::SoftwareSystem
                        | ElementKind::Container
                        | ElementKind::Component
                )
            )
        {
            let group = self
                .top()
                .and_then(Ctx::group_payload)
                .map(|g| g.name.clone());
            if let Some(name) = group {
                let id = id.clone();
                // `setGroup` only — `FindElementParser` never calls
                // `group.addElement`, so the group's member set is
                // untouched (unlike the element-definition parsers).
                let _ = self.builder.set_group(&id, &name);
            }
        }
        let canonical = match &item {
            RegItem::Element(id) => self.builder.canonical_name_full(id).unwrap_or_default(),
            RegItem::Pseudo { .. } | RegItem::Relationship(_) => s.clone().into_owned(),
        };
        Ok((item, canonical))
    }

    /// `FindRelationshipParser.parse`.
    pub(crate) fn find_relationship(&mut self, tokens: &Tokens) -> Result<(Id, String), DslError> {
        const GRAMMAR: &str = "!relationship <identifier|canonical name>";
        if tokens.has_more_than(1) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {GRAMMAR}"
            )));
        }
        if !tokens.includes(1) {
            return Err(DslError::stmt(format!("Expected: {GRAMMAR}")));
        }
        let s = tokens.get(1);
        let id = if s.starts_with("Relationship://") {
            self.builder.relationship_with_canonical_name(&s)
        } else {
            self.get_relationship(&s)
        };
        let Some(id) = id else {
            return Err(DslError::stmt(format!(
                "A relationship identified by \"{s}\" could not be found"
            )));
        };
        let canonical = self
            .builder
            .relationship_canonical_name(&id)
            .unwrap_or_else(|| s.clone().into_owned());
        Ok((id, canonical))
    }

    /// `FindElementsParser.parse` — the element half of an expression.
    pub(crate) fn find_elements(&mut self, tokens: &Tokens) -> Result<Vec<Id>, DslError> {
        let expression = self.finder_expression(tokens, "!elements <expression>")?;
        let items = self.parse_expression(&expression, super::expr::ExprScope::Model)?;
        let elements: Vec<Id> = items
            .into_iter()
            .filter_map(|mi| match mi {
                Mi::Element(id) => Some(id),
                Mi::Rel(_) => None,
            })
            .collect();
        if elements.is_empty() {
            return Err(DslError::stmt(format!(
                "No elements found for expression \"{expression}\""
            )));
        }
        Ok(elements)
    }

    /// `FindRelationshipsParser.parse` — the relationship half.
    pub(crate) fn find_relationships(&mut self, tokens: &Tokens) -> Result<Vec<Id>, DslError> {
        let expression = self.finder_expression(tokens, "!relationships <expression>")?;
        let items = self.parse_expression(&expression, super::expr::ExprScope::Model)?;
        let relationships: Vec<Id> = items
            .into_iter()
            .filter_map(|mi| match mi {
                Mi::Rel(id) => Some(id),
                Mi::Element(_) => None,
            })
            .collect();
        if relationships.is_empty() {
            return Err(DslError::stmt(format!(
                "No relationships found for expression \"{expression}\""
            )));
        }
        Ok(relationships)
    }

    #[allow(clippy::unused_self)] // method symmetry with the other guards
    fn finder_expression(&self, tokens: &Tokens, grammar: &str) -> Result<String, DslError> {
        if tokens.has_more_than(1) {
            return Err(DslError::stmt(format!(
                "Too many tokens, expected: {grammar}"
            )));
        }
        // `FindElementsParser`/`FindRelationshipsParser` do NOT check
        // `includes(1)` — a bare `!elements` reaches `tokens.get(1)` and
        // the JDK throws; the message is reproduced.
        if !tokens.includes(1) {
            return Err(DslError::stmt(format!(
                "Index 1 out of bounds for length {}",
                tokens.size()
            )));
        }
        Ok(tokens.get(1).into_owned())
    }
}
