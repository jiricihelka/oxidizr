//! Identity newtypes and the shared id counter.
//!
//! ONE sequential integer counter is shared by elements and relationships in
//! definition order, starting at 1 (`SequentialIntegerIdGeneratorStrategy`); JSON
//! renders ids as strings everywhere except `workspace.id` (a number) — the
//! string rendering happens in `oxidizr-json`, the model stores [`Id`] values.
//!
//! [`Id`] is string-typed (like the official string ids) so legacy
//! workspaces with non-numeric ids round-trip byte-losslessly; numeric ids keep a
//! cached integer view because the official ORDERING of model items
//! (numeric when both parse, else string) is an output
//! concern, while id LOOKUP is a Java `HashMap` lookup:
//! `HashMap<String,…>`) is TEXT equality — which is what [`Id`]'s `Eq`/`Ord`
//! implement.

use std::sync::Arc;

/// A model id (element or relationship). Strings in the JSON contract; the
/// decimal-integer form is the norm, but any string round-trips.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct Id {
    text: Arc<str>,
    /// `Some` iff `text` parses as a Java `int` (leading `+`/`-` accepted,
    /// 32-bit range) — mirrors the official int-parse attempts
    /// (ordering and id registration).
    num: Option<i32>,
}

impl Id {
    /// Wrap an id string, detecting its numeric form.
    #[must_use]
    pub fn new(text: impl Into<Arc<str>>) -> Id {
        let text = text.into();
        let num = text.parse::<i32>().ok();
        Id { text, num }
    }

    /// The numeric id form (counter allocation).
    #[must_use]
    pub fn from_num(n: u32) -> Id {
        Id::new(n.to_string())
    }

    /// The id text (what JSON serializes).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// The Java-`int` value, if this id parses as one.
    #[must_use]
    pub const fn as_num(&self) -> Option<i32> {
        self.num
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

/// Id of an element (person / system / container / component / custom element /
/// deployment element).
#[derive(Debug, Clone, Eq)]
pub struct ElementId(pub Id);

/// Id of a relationship. Shares the counter with [`ElementId`] — the two id
/// spaces must not collide (`Model::addRelationshipToInternalStructures` checks
/// BOTH maps).
#[derive(Debug, Clone, Eq)]
pub struct RelationshipId(pub Id);

macro_rules! id_newtype_impls {
    ($t:ident) => {
        impl std::ops::Deref for $t {
            type Target = Id;
            fn deref(&self) -> &Id {
                &self.0
            }
        }

        impl std::fmt::Display for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.0.eq(&other.0)
            }
        }

        impl std::hash::Hash for $t {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }

        impl Ord for $t {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.0.cmp(&other.0)
            }
        }

        impl PartialOrd for $t {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }
        }

        impl $t {
            /// Wrap an id string.
            #[must_use]
            pub fn new(text: impl Into<Arc<str>>) -> Self {
                Self(Id::new(text))
            }

            /// The numeric id form (counter allocation).
            #[must_use]
            pub fn from_num(n: u32) -> Self {
                Self(Id::from_num(n))
            }
        }
    };
}

id_newtype_impls!(ElementId);
id_newtype_impls!(RelationshipId);

/// The single shared element/relationship id counter
/// (`SequentialIntegerIdGeneratorStrategy`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdGenerator {
    /// Highest numeric id allocated so far; the next id is `last + 1`.
    last: i32,
}

impl IdGenerator {
    /// A fresh counter: the first allocation yields 1.
    #[must_use]
    pub const fn new() -> IdGenerator {
        IdGenerator { last: 0 }
    }

    /// Allocate the next id. (Not `Iterator::next` — ids are never `Option`.)
    ///
    /// # Panics
    ///
    /// If more than `i32::MAX` ids are allocated (a workspace beyond the
    /// official int-id ceiling — unrecoverable by construction).
    pub fn generate(&mut self) -> u32 {
        self.last += 1;
        u32::try_from(self.last).expect("id counter overflow (more than i32::MAX ids allocated)")
    }

    /// Register an externally assigned id (JSON load): the counter jumps past
    /// NUMERIC ids are ignored, exactly like the official
    /// id registration swallowing parse failures.
    pub fn found(&mut self, id: &Id) {
        if let Some(n) = id.as_num()
            && n > self.last
        {
            self.last = n;
        }
    }
}

impl Default for IdGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_is_sequential_from_one() {
        let mut g = IdGenerator::new();
        assert_eq!(g.generate(), 1);
        assert_eq!(g.generate(), 2);
    }

    #[test]
    fn found_jumps_forward_and_ignores_text() {
        // Mirrors SequentialIntegerIdGeneratorStrategy.found(): the counter moves
        // past any numeric id and never rewinds; non-numeric ids are ignored.
        let mut g = IdGenerator::new();
        g.found(&Id::new("10"));
        assert_eq!(g.generate(), 11);
        g.found(&Id::new("3"));
        assert_eq!(g.generate(), 12);
        g.found(&Id::new("550e8400-e29b-41d4-a716-446655440000"));
        assert_eq!(g.generate(), 13);
    }

    #[test]
    fn id_equality_is_text_equality() {
        // id lookup is a Java HashMap lookup: "+5" and "5" are DISTINCT
        // ids for lookup, even though the ordering comparator
        // (an output concern) would rate them equal when both parse as ints.
        assert_ne!(Id::new("+5"), Id::new("5"));
        assert_ne!(Id::new("03"), Id::new("3"));
        assert_eq!(Id::new("7"), Id::new("7"));
        assert_eq!(Id::new("7").as_num(), Some(7));
        assert_eq!(Id::new("99999999999").as_num(), None); // outside Java int range
        assert_eq!(Id::from_num(42).as_str(), "42");
    }
}
