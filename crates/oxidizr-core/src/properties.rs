//! Free-form property maps with STABLE INSERTION ORDER (`PropertyHolder`
//! fields, `structurizr-client` WRITE-CATALOG §1).
//!
//! The reference stores these in Java `HashMap`s and Jackson serializes them
//! in hash-bucket order — not sorted, not insertion order. Bucket order is
//! computed by `oxidizr-json` at write time from (a) the final capacity and
//! (b) the exact insertion sequence, so the model must preserve insertion
//! order verbatim: a `BTreeMap` (sorted) or any order-losing map is wrong
//! for byte parity (V-11).

/// Insertion-ordered `String -> String` map replicating Java `HashMap`
/// mutation semantics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Properties {
    entries: Vec<(String, String)>,
}

impl Properties {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Properties {
            entries: Vec::new(),
        }
    }

    /// Java `HashMap.put` semantics: a new key is appended (its bucket slot
    /// is decided by INSERTION POSITION, so position is part of the contract);
    /// re-putting an existing key replaces the value in place.
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<String>) {
        let (name, value) = (name.into(), value.into());
        match self.entries.iter_mut().find(|(k, _)| *k == name) {
            Some(slot) => slot.1 = value,
            None => self.entries.push((name, value)),
        }
    }

    /// Value for `name`, if present.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    /// Whether `name` is present.
    #[must_use]
    pub fn contains_key(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    /// Java `HashMap.remove` semantics: drops the entry and returns its
    /// value; a later re-insert of the same key takes a NEW slot
    /// (`DslUtils.clearDsl` + DSL retention re-adding `structurizr.dsl`).
    pub fn remove(&mut self, name: &str) -> Option<String> {
        let at = self.entries.iter().position(|(k, _)| k == name)?;
        Some(self.entries.remove(at).1)
    }

    /// Entries in insertion order (the sequence the write-side `HashMap`
    /// emulator replays).
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the map is empty (`NON_EMPTY` inclusion hides empty ones).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl FromIterator<(String, String)> for Properties {
    /// Replays entries in document order with [`Properties::insert`]
    /// semantics (the JSON reader feeds keys exactly as encountered).
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut out = Properties::new();
        for (k, v) in iter {
            out.insert(k, v);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::Properties;

    #[test]
    fn insertion_order_is_preserved() {
        let mut p = Properties::new();
        p.insert("zulu", "1");
        p.insert("alpha", "2");
        p.insert("Mike", "3");
        assert_eq!(
            p.iter().map(|(k, _)| k).collect::<Vec<_>>(),
            ["zulu", "alpha", "Mike"]
        );
    }

    #[test]
    fn reinsert_replaces_in_place() {
        let mut p = Properties::new();
        p.insert("a", "1");
        p.insert("b", "2");
        p.insert("a", "9");
        assert_eq!(p.get("a"), Some("9"));
        assert_eq!(p.len(), 2);
        assert_eq!(
            p.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>(),
            ["a=9", "b=2"]
        );
    }

    #[test]
    fn from_iter_replays_document_order() {
        let p: Properties = [
            ("x".to_owned(), "1".to_owned()),
            ("x".to_owned(), "2".to_owned()),
        ]
        .into_iter()
        .collect();
        assert_eq!(p.len(), 1);
        assert_eq!(p.get("x"), Some("2"));
    }
}
