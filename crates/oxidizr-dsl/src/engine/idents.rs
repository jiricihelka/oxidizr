//! Port of the reference `IdentifiersRegister` (identifier → model item),
//! including its failure texts, case-insensitive lookups, hierarchical
//! identifier computation and UUID fallback.
//!
//! Identity model: Java compares registered items with `equals` —
//! `Element`/`Relationship`/`ElementGroup`/`DeploymentGroup` do NOT override
//! it (object identity, so `RegItem::Element` compares ids and each PSEUDO
//! registration carries a fresh uid), but the DSL's `DeploymentEnvironment`
//! DOES: it equals by NAME (`name.equals(that.name)` + matching class). A
//! fresh `new DeploymentEnvironment(name)` therefore FINDS the environment's
//! registered identifier (verified against the oracle: hierarchical node
//! identifiers are `<envIdentifier>.<nodeIdentifier>`), and two
//! `deploymentEnvironment` statements with the same name collide in the
//! register. `null.<name>` prefixes only survive where no environment of
//! that name was ever registered.

use oxidizr_core::Id;

use super::uuid::{looks_like_uuid, uuid_v4};

/// `IdentifierScope` (`!identifiers flat|hierarchical`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdentifierScope {
    /// Global namespace (default).
    #[default]
    Flat,
    /// Identifiers are prefixed by the enclosing element's identifier chain.
    Hierarchical,
}

/// Pseudo-elements the DSL registers but that never enter the workspace
/// model. Each registration gets a fresh `uid` for identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PseudoKind {
    /// `group "..." {` (the full, separator-joined name). `parent` mirrors
    /// `ElementGroup.setParent`, which the reference calls ONLY in the
    /// softwareSystem/container group arms — model-scope groups have no
    /// parent, so their hierarchical registration gets no prefix.
    Group {
        /// The full, separator-joined name.
        name: String,
        /// The system/container the group was declared in, if any.
        parent: Option<Id>,
    },
    /// `deploymentEnvironment "..."` — the `DeploymentEnvironment` the
    /// statement registers. Java `equals` is NAME-BASED for this class, so
    /// `findIdentifier(new DeploymentEnvironment(name))` resolves through it
    /// (the deployment-node / deployment-group hierarchical prefix).
    DeploymentEnvironment(String),
    /// `deploymentGroup "..."`. Java's parent is the context's
    /// `DeploymentEnvironment`, which `equals` resolves by name, so the
    /// hierarchical prefix is that environment's registered identifier
    /// (falling back to the literal `"null"` only when unregistered).
    DeploymentGroup {
        /// Enclosing environment name (the parent lookup key).
        environment: String,
        /// Group name.
        name: String,
    },
}

/// What the identifier register maps a name onto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegItem {
    /// A real model element.
    Element(Id),
    /// A relationship.
    Relationship(Id),
    /// A DSL pseudo-element.
    Pseudo {
        /// Fresh per registration (identity, like Java object identity).
        uid: u64,
        /// The pseudo kind/value.
        kind: PseudoKind,
    },
}

impl RegItem {
    /// Java `equals` semantics for registered objects — the comparison the
    /// reference's `findIdentifier` scan and `register` duplicate-check use.
    /// Everything is identity EXCEPT the DSL `DeploymentEnvironment`, which
    /// overrides `equals` by name.
    #[must_use]
    #[allow(clippy::match_same_arms)] // the fall-through arm repeats identity on purpose
    pub fn java_equals(&self, other: &RegItem) -> bool {
        match (self, other) {
            (RegItem::Element(a), RegItem::Element(b)) => a == b,
            (RegItem::Relationship(a), RegItem::Relationship(b)) => a == b,
            (
                RegItem::Pseudo {
                    kind: PseudoKind::DeploymentEnvironment(a),
                    ..
                },
                RegItem::Pseudo {
                    kind: PseudoKind::DeploymentEnvironment(b),
                    ..
                },
            ) => a == b,
            (RegItem::Pseudo { uid: a, kind: ka }, RegItem::Pseudo { uid: b, kind: kb }) => {
                a == b && ka == kb
            }
            _ => false,
        }
    }
}

/// One registered name and its item.
#[derive(Debug, Clone)]
struct Entry {
    identifier: String,
    item: RegItem,
}

/// The identifier register. Element and relationship NAMES share one
/// namespace (collision-checked both ways) but lookups are per-kind, like the
/// reference's two maps.
#[derive(Debug, Default)]
pub struct IdentifiersRegister {
    scope: IdentifierScope,
    elements: Vec<Entry>,
    relationships: Vec<Entry>,
}

impl IdentifiersRegister {
    /// Empty flat-scope register.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Current scope.
    #[must_use]
    pub fn scope(&self) -> IdentifierScope {
        self.scope
    }

    /// `setIdentifierScope` (`!identifiers …`).
    pub fn set_scope(&mut self, scope: IdentifierScope) {
        self.scope = scope;
    }

    /// Merge another register's entries into this one (`workspace extends`:
    /// reference `DslParserContext.copyFrom`, which replays the child's
    /// entries through `register()`). A verbatim append is equivalent for
    /// every REACHABLE extends case: the target register is always empty at
    /// copy time (extends is the first statement, the register is created
    /// with the parser), so no collision guard can fire; re-running
    /// hierarchical prefix computation on already-composite keys would
    /// double-prefix them, and the reference only avoids that by `HashMap`
    /// iteration luck — so keys are kept verbatim (deterministic choice,
    /// matches every reference test).
    pub(crate) fn adopt_entries(&mut self, other: &mut IdentifiersRegister) {
        self.elements.append(&mut other.elements);
        self.relationships.append(&mut other.relationships);
    }

    /// Case-insensitive element lookup (first registered key wins).
    #[must_use]
    pub fn element(&self, identifier: &str) -> Option<&RegItem> {
        self.elements
            .iter()
            .find(|e| crate::compat::java_eq_ignore_case(&e.identifier, identifier))
            .map(|e| &e.item)
    }

    /// Case-insensitive relationship lookup.
    #[must_use]
    pub fn relationship(&self, identifier: &str) -> Option<&RegItem> {
        self.relationships
            .iter()
            .find(|e| crate::compat::java_eq_ignore_case(&e.identifier, identifier))
            .map(|e| &e.item)
    }

    /// The identifier under which this item is registered (Java `equals`),
    /// if any.
    #[must_use]
    pub fn find_element(&self, item: &RegItem) -> Option<&str> {
        self.elements
            .iter()
            .find(|e| e.item.java_equals(item))
            .map(|e| e.identifier.as_str())
    }

    /// `findIdentifier(new DeploymentEnvironment(name))` — the identifier of
    /// the environment registered under this name (case-SENSITIVE, like the
    /// Java `equals`), or `None` (`null` in Java) when no such environment
    /// was registered.
    #[must_use]
    pub fn find_env(&self, name: &str) -> Option<&str> {
        self.find_element(&RegItem::Pseudo {
            uid: 0,
            kind: PseudoKind::DeploymentEnvironment(name.to_string()),
        })
    }

    /// The identifier of an element id (identity equality).
    #[must_use]
    pub fn find_element_id(&self, id: &Id) -> Option<&str> {
        self.find_element(&RegItem::Element(id.clone()))
    }

    /// The identifier of a relationship id.
    #[allow(dead_code)] // Java parity surface: `Model.getRelationship` lookup.
    #[must_use]
    pub fn find_relationship_id(&self, id: &Id) -> Option<&str> {
        self.relationships
            .iter()
            .find(|e| e.item == RegItem::Relationship(id.clone()))
            .map(|e| e.identifier.as_str())
    }

    /// `register(identifier, element)`.
    ///
    /// * `prefix` — `Some(p)` applies the hierarchical-scope prefix when the
    ///   register is in `Hierarchical` scope: the parent's found identifier,
    ///   or the literal `"null"` reproducing the reference's `null + "." +
    ///   name` string concatenation (unregistered parent / fresh
    ///   `DeploymentEnvironment` pseudo). `None` keeps the name (an element
    ///   with no parent).
    /// * `canonical_name` — only used in the "please assign an identifier"
    ///   error text.
    ///
    /// Returns the FINAL identifier the item was stored under.
    ///
    /// # Errors
    ///
    /// Returns the reference message when the identifier is invalid, is
    /// already registered, or an automatic name would be needed without
    /// `Auto` scope.
    pub fn register_element(
        &mut self,
        identifier: Option<&str>,
        item: RegItem,
        prefix: Option<String>,
        canonical_name: &str,
    ) -> Result<String, String> {
        let mut identifier = match identifier {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => uuid_v4(),
        };
        if self.scope == IdentifierScope::Hierarchical
            && let Some(p) = prefix
        {
            identifier = format!("{p}.{identifier}");
        }

        // Re-registration of the same item (Java `equals`) under another
        // name.
        for entry in &self.elements {
            if entry.item.java_equals(&item)
                && !crate::compat::java_eq_ignore_case(&entry.identifier, &identifier)
            {
                return Err(if looks_like_uuid(&entry.identifier) {
                    format!("Please assign an identifier to \"{canonical_name}\" before using it")
                } else {
                    format!(
                        "The element is already registered with an identifier of \"{}\"",
                        entry.identifier
                    )
                });
            }
        }

        if self.element(&identifier).is_some() || self.relationship(&identifier).is_some() {
            return Err(format!(
                "The identifier \"{}\" is already in use",
                identifier.to_lowercase()
            ));
        }
        self.elements.push(Entry {
            identifier: identifier.clone(),
            item,
        });
        Ok(identifier)
    }

    /// `register(identifier, relationship)`. Relationships never get a
    /// hierarchical prefix. Returns the final identifier.
    ///
    /// # Errors
    ///
    /// Returns the reference message when the identifier is invalid or
    /// already registered.
    pub fn register_relationship(
        &mut self,
        identifier: Option<&str>,
        id: &Id,
        canonical_name: &str,
    ) -> Result<String, String> {
        let item = RegItem::Relationship(id.clone());
        let identifier = match identifier {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => uuid_v4(),
        };

        for entry in &self.relationships {
            if entry.item == item
                && !crate::compat::java_eq_ignore_case(&entry.identifier, &identifier)
            {
                return Err(if looks_like_uuid(&entry.identifier) {
                    format!("Please assign an identifier to \"{canonical_name}\" before using it")
                } else {
                    format!(
                        "The relationship is already registered with an identifier of \"{}\"",
                        entry.identifier
                    )
                });
            }
        }

        if self.element(&identifier).is_some() || self.relationship(&identifier).is_some() {
            return Err(format!(
                "The identifier \"{}\" is already in use",
                identifier.to_lowercase()
            ));
        }
        self.relationships.push(Entry {
            identifier: identifier.clone(),
            item,
        });
        Ok(identifier)
    }

    /// `validateIdentifierName` — checked on every `ident = ` prefix.
    ///
    /// # Errors
    ///
    /// Returns the reference message for `-`-prefixed, empty or
    /// non-`\w[a-zA-Z0-9_-]*` names.
    pub fn validate_identifier_name(identifier: &str) -> Result<(), String> {
        if identifier.starts_with('-') {
            return Err("Identifiers cannot start with a - character".to_string());
        }
        // Java \w[a-zA-Z0-9_-]* : first char [a-zA-Z0-9_], rest also allow -.
        let valid = !identifier.is_empty()
            && identifier
                .chars()
                .enumerate()
                .all(|(i, c)| c.is_ascii_alphanumeric() || c == '_' || (c == '-' && i > 0));
        if !valid {
            return Err(
                "Identifiers can only contain the following characters: a-zA-Z0-9_-".to_string(),
            );
        }
        Ok(())
    }

    /// `toIdentifier` — strip invalid characters, then a leading `-`.
    #[must_use]
    #[allow(dead_code)] // Java parity surface: `DslIdentifiers.toIdentifier` (test-pinned).
    pub fn to_identifier(s: &str) -> String {
        let mut out: String = s
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if let Some(stripped) = out.strip_prefix('-') {
            out = stripped.to_string();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::many_single_char_names)]

    use super::*;

    fn id(n: &str) -> Id {
        Id::new(n)
    }

    #[test]
    fn names_are_one_case_insensitive_namespace() {
        let mut r = IdentifiersRegister::new();
        r.register_element(Some("ci"), RegItem::Element(id("1")), None, "CI")
            .unwrap();
        assert!(r.element("CI").is_some());
        assert_eq!(
            r.register_relationship(Some("CI"), &id("9"), "rel"),
            Err("The identifier \"ci\" is already in use".to_string())
        );
    }

    #[test]
    fn same_element_second_name_reports_original_identifier() {
        let mut r = IdentifiersRegister::new();
        r.register_element(Some("one"), RegItem::Element(id("1")), None, "X")
            .unwrap();
        assert_eq!(
            r.register_element(Some("two"), RegItem::Element(id("1")), None, "X"),
            Err("The element is already registered with an identifier of \"one\"".to_string())
        );
    }

    #[test]
    fn anonymous_reuse_asks_for_an_identifier() {
        let mut r = IdentifiersRegister::new();
        let first = r
            .register_element(None, RegItem::Element(id("1")), None, "Sys")
            .unwrap();
        assert!(looks_like_uuid(&first));
        assert_eq!(
            r.register_element(Some("late"), RegItem::Element(id("1")), None, "Sys"),
            Err("Please assign an identifier to \"Sys\" before using it".to_string())
        );
    }

    #[test]
    fn hierarchical_prefix_is_applied() {
        let mut r = IdentifiersRegister::new();
        r.set_scope(IdentifierScope::Hierarchical);
        r.register_element(Some("s"), RegItem::Element(id("1")), None, "S")
            .unwrap();
        r.register_element(
            Some("c"),
            RegItem::Element(id("2")),
            Some(r.find_element_id(&id("1")).unwrap().to_string()),
            "S/C",
        )
        .unwrap();
        assert!(r.element("s.c").is_some());
        // unregistered parent → the literal "null." concatenation quirk
        r.register_element(
            Some("x"),
            RegItem::Element(id("3")),
            Some("null".into()),
            "X",
        )
        .unwrap();
        assert!(r.element("null.x").is_some());
    }

    #[test]
    fn env_pseudos_equal_by_name_others_by_identity() {
        let mut r = IdentifiersRegister::new();
        r.register_element(
            Some("live"),
            RegItem::Pseudo {
                uid: 1,
                kind: PseudoKind::DeploymentEnvironment("Live".into()),
            },
            None,
            "Live",
        )
        .unwrap();
        // Java `DeploymentEnvironment.equals` is name-based: a second
        // `deploymentEnvironment "Live"` collides.
        assert_eq!(
            r.register_element(
                Some("live2"),
                RegItem::Pseudo {
                    uid: 2,
                    kind: PseudoKind::DeploymentEnvironment("Live".into()),
                },
                None,
                "Live"
            ),
            Err("The element is already registered with an identifier of \"live\"".to_string())
        );
        // … and a fresh by-name lookup finds the registered identifier
        // (the `<envIdentifier>.<nodeId>` hierarchical prefix oracle check).
        assert_eq!(r.find_env("Live"), Some("live"));
        assert_eq!(r.find_env("Dev"), None);
        // DeploymentGroup has no equals override — identity (uid) only.
        r.register_element(
            Some("g1"),
            RegItem::Pseudo {
                uid: 3,
                kind: PseudoKind::DeploymentGroup {
                    environment: "Live".into(),
                    name: "G".into(),
                },
            },
            None,
            "G",
        )
        .unwrap();
        assert!(
            r.register_element(
                Some("g2"),
                RegItem::Pseudo {
                    uid: 4,
                    kind: PseudoKind::DeploymentGroup {
                        environment: "Live".into(),
                        name: "G".into(),
                    },
                },
                None,
                "G"
            )
            .is_ok(),
            "DeploymentGroup equals by identity, not value"
        );
    }

    #[test]
    fn identifier_name_validation() {
        assert!(IdentifiersRegister::validate_identifier_name("a_b-C9").is_ok());
        assert!(IdentifiersRegister::validate_identifier_name("_x").is_ok());
        assert_eq!(
            IdentifiersRegister::validate_identifier_name("-x"),
            Err("Identifiers cannot start with a - character".to_string())
        );
        assert_eq!(
            IdentifiersRegister::validate_identifier_name("a.b"),
            Err("Identifiers can only contain the following characters: a-zA-Z0-9_-".to_string())
        );
        assert!(IdentifiersRegister::validate_identifier_name("").is_err());
    }

    #[test]
    fn to_identifier_strips() {
        assert_eq!(IdentifiersRegister::to_identifier("My Group!"), "MyGroup");
        assert_eq!(IdentifiersRegister::to_identifier("-x-"), "x-");
    }
}
