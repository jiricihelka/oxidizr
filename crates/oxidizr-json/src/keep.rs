//! `--keep-unknown` superset-preserving JSON→JSON round-trip.
//!
//! The default path drops every member the official tooling drops (byte
//! parity). The opt-in path here instead *pairs* the input
//! document with the workspace's writable `Json` tree and re-appends every
//! input member the tree does not carry — unknown AND known-legacy fields
//! (`enterprise`, `location`, `lastModifiedDate*`, `revision`, …) — verbatim
//! after the known keys, alphabetical (flat
//! `BTreeMap` shape for this mode; output is intentionally
//! non-parity).
//!
//! Pairing rules: objects pair by member name; arrays pair elements by
//! identity (`id`/`key`/`selector` string members — the writer re-sorts
//! arrays, so position is meaningless) and then, for identity-less elements,
//! by compatibility (every output member present-and-equal in the input).
//! An input element that pairs with nothing (the reader dropped it) loses
//! its extras exactly like the parity path.

use serde_json::Value;

use crate::error::JsonError;
use crate::printer::{self, Json};
use crate::writer;

/// Read (parity parse + validation) and write with unknown /
/// legacy members preserved verbatim. Bytes in, bytes out; the CLI wires it
/// to `json2json --keep-unknown`.
///
/// # Errors
///
/// Identical to [`crate::from_slice`] + [`crate::to_vec`].
pub fn roundtrip_keep_unknown(bytes: &[u8]) -> Result<Vec<u8>, JsonError> {
    let input: Value = serde_json::from_slice(bytes)?;
    let workspace = crate::from_slice(bytes)?;
    let mut tree = writer::to_tree(&workspace)?;
    overlay(&input, &mut tree);
    Ok(printer::to_bytes(&tree))
}

/// Recursive overlay: descend `tree` along members the input also has, and
/// append the members it lacks (sorted, after the known keys).
fn overlay(input: &Value, node: &mut Json) {
    match (input, node) {
        (Value::Object(ino), Json::Obj(entries)) => {
            let mut extras: Vec<(String, Json)> = Vec::new();
            for (key, value) in ino {
                match entries.iter_mut().find(|(k, _)| k == key) {
                    Some((_, slot)) => overlay(value, slot),
                    None => extras.push((key.clone(), value_to_json(value))),
                }
            }
            extras.sort_by(|a, b| a.0.cmp(&b.0));
            entries.extend(extras);
        }
        (Value::Array(initems), Json::Arr(outitems)) => {
            for (i, j) in pair_items(initems, outitems) {
                overlay(&initems[i], &mut outitems[j]);
            }
        }
        _ => {}
    }
}

/// Input element `i` pairs with output element `j`: identity pass first
/// (`id` → `key` → `selector`), then greedy compatibility on the rest.
fn pair_items(initems: &[Value], out: &[Json]) -> Vec<(usize, usize)> {
    let mut taken = vec![false; out.len()];
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    let mut identityless: Vec<usize> = Vec::new();
    for (i, iv) in initems.iter().enumerate() {
        let hit = identity(iv).and_then(|id| {
            out.iter()
                .enumerate()
                .find(|(j, oj)| !taken[*j] && identity_json(oj).as_deref() == Some(id.as_str()))
                .map(|(j, _)| j)
        });
        match hit {
            Some(j) => {
                taken[j] = true;
                pairs.push((i, j));
            }
            None => identityless.push(i),
        }
    }
    let mut free: Vec<usize> = (0..out.len()).filter(|j| !taken[*j]).collect();
    for i in identityless {
        if let Some(pos) = free.iter().position(|&j| compatible(&initems[i], &out[j])) {
            let j = free.remove(pos);
            pairs.push((i, j));
        }
    }
    pairs.sort_unstable();
    pairs
}

/// The first `id`/`key`/`selector` string member (array-element identity).
fn identity(item: &Value) -> Option<String> {
    ["id", "key", "selector"]
        .iter()
        .find_map(|k| item.get(k).and_then(Value::as_str))
        .map(str::to_string)
}

fn identity_json(item: &Json) -> Option<String> {
    let Json::Obj(entries) = item else {
        return None;
    };
    ["id", "key", "selector"].iter().find_map(|k| {
        entries.iter().find_map(|(ek, ev)| match (ek.as_str(), ev) {
            (name, Json::Str(s)) if name == *k => Some(s.clone()),
            _ => None,
        })
    })
}

/// An identity-less input element pairs with an output element whose every
/// member it carries with an equal value (the writer may add omitted
/// defaults; it never invents content the input lacked).
fn compatible(input: &Value, out: &Json) -> bool {
    match (input, out) {
        (Value::Object(io), Json::Obj(oe)) => oe
            .iter()
            .all(|(k, jv)| io.get(k).is_some_and(|v| json_eq(jv, v))),
        (Value::Array(ia), Json::Arr(oa)) => {
            ia.len() == oa.len() && ia.iter().zip(oa).all(|(iv, jo)| json_eq(jo, iv))
        }
        (iv, jo) => json_eq(jo, iv),
    }
}

/// Equality between a tree node and an input value (numbers match on `i64`
/// identity; `Raw` literals match their source `null`/number text).
fn json_eq(node: &Json, value: &Value) -> bool {
    match (node, value) {
        (Json::Str(a), Value::String(b)) => a == b,
        (Json::Num(a), Value::Number(n)) => n.as_i64() == Some(*a),
        (Json::Bool(a), Value::Bool(b)) => a == b,
        (Json::Raw(literal), Value::Null) => literal == "null",
        (Json::Raw(literal), Value::Number(n)) => &n.to_string() == literal,
        (Json::Obj(a), Value::Object(vo)) => {
            a.len() == vo.len()
                && a.iter()
                    .all(|(k, jv)| vo.get(k).is_some_and(|v| json_eq(jv, v)))
        }
        (Json::Arr(a), Value::Array(va)) => {
            a.len() == va.len() && a.iter().zip(va).all(|(jo, iv)| json_eq(jo, iv))
        }
        _ => false,
    }
}

/// Capture an untouched input subtree; object keys sorted (the `BTreeMap`
/// flatten semantics), scalars outside `i64` kept as verbatim literals.
fn value_to_json(value: &Value) -> Json {
    match value {
        Value::String(s) => Json::Str(s.clone()),
        Value::Bool(b) => Json::Bool(*b),
        Value::Number(n) => n
            .as_i64()
            .map_or_else(|| Json::Raw(n.to_string()), Json::Num),
        Value::Null => Json::Raw("null".to_string()),
        Value::Array(items) => Json::Arr(items.iter().map(value_to_json).collect()),
        Value::Object(obj) => {
            let mut entries: Vec<(String, Json)> = obj
                .iter()
                .map(|(k, v)| (k.clone(), value_to_json(v)))
                .collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Json::Obj(entries)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::roundtrip_keep_unknown;
    use crate::{from_slice, to_vec};

    const BASE: &str = r#"{
  "configuration" : { },
  "description" : "",
  "documentation" : { },
  "id" : 0,
  "model" : {
    "people" : [ {
      "documentation" : { },
      "id" : "1",
      "name" : "A",
      "tags" : "Element,Person"
    }, {
      "documentation" : { },
      "id" : "2",
      "name" : "B",
      "tags" : "Element,Person"
    } ]
  },
  "name" : "Legacy",
  "views" : {
    "configuration" : {
      "styles" : { },
      "terminology" : { }
    }
  }
}"#;

    /// Canonical input (a parity output) carries nothing the tree lacks -
    /// keep mode then reproduces the parity bytes exactly.
    #[test]
    fn no_extras_output_equals_parity_bytes() {
        let canonical = to_vec(&from_slice(BASE.as_bytes()).unwrap()).unwrap();
        assert_eq!(roundtrip_keep_unknown(&canonical).unwrap(), canonical);
    }

    /// The superset rule also keeps members the parity writer omits
    /// (an empty `description`) - appended after the known keys, and
    /// stable under repetition.
    #[test]
    fn writer_omitted_members_are_preserved_and_stable() {
        let once = roundtrip_keep_unknown(BASE.as_bytes()).unwrap();
        let text = String::from_utf8(once.clone()).unwrap();
        assert!(text.ends_with("\"description\" : \"\"\n}"), "{text}");
        let twice = roundtrip_keep_unknown(&once).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn unknown_members_captured_and_idempotent() {
        let input = BASE.replacen(
            "\"name\" : \"Legacy\",",
            "\"enterprise\" : { \"name\" : \"E\" },\n  \"lastModifiedDate\" : 1.5,\n  \"name\" : \"Legacy\",",
            1,
        );
        let once = roundtrip_keep_unknown(input.as_bytes()).unwrap();
        let text = String::from_utf8(once.clone()).unwrap();
        assert!(text.contains("\"enterprise\" : {"), "{text}");
        // non-i64 numbers survive verbatim:
        assert!(text.contains("\"lastModifiedDate\" : 1.5"), "{text}");
        // the output still reads…
        let ws = from_slice(&once).unwrap();
        assert_eq!(ws.model.people.len(), 2);
        // …and a second round-trip is byte-stable:
        let twice = roundtrip_keep_unknown(&once).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn identity_pairing_follows_resorted_arrays() {
        // Writer people order is the sorted set (name-ci); extras declared on
        // "B" must land on "B", not on position 0 ("A").
        let input = BASE.replacen(
            "\"id\" : \"2\",\n      \"name\" : \"B\"",
            "\"id\" : \"2\",\n      \"location\" : \"Prague\",\n      \"name\" : \"B\"",
            1,
        );
        let out = String::from_utf8(roundtrip_keep_unknown(input.as_bytes()).unwrap()).unwrap();
        let b = out.find("\"B\"").expect("B written");
        let loc = out
            .find("\"location\" : \"Prague\"")
            .expect("location kept");
        assert!(loc > b, "location attached to the wrong person:\n{out}");
    }
}
