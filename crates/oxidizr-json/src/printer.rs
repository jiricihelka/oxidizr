//! Jackson `DefaultPrettyPrinter` byte emulation (`WRITE-CATALOG` §0),
//! verified against the goldens (separator `" : "`, 2-space indent, LF,
//! `{ }` / `[ ]`, inline SCALAR arrays, expanded object arrays, no trailing
//! newline).

use crate::javahash;
use oxidizr_core::properties::Properties;

/// A JSON value in output-ready shape. Objects carry their entries in FINAL
/// key order (the caller sorts: alphabetical for POJOs — Jackson
/// `SORT_PROPERTIES_ALPHABETICALLY` — `HashMap` order for `properties`);
/// omissions (`NON_EMPTY` etc.) are applied by the caller, so everything
/// present here serializes.
#[derive(Debug, Clone)]
pub enum Json {
    /// String (escaped at emit time, Jackson rules).
    Str(String),
    /// int/i64 render.
    Num(i64),
    Bool(bool),
    /// Object; empty serializes as `{ }`.
    Obj(Vec<(String, Json)>),
    /// Array; empty `[ ]`, all-scalar inline, object elements expanded.
    Arr(Vec<Json>),
    /// Verbatim scalar literal (`null`, non-`i64` numbers) — produced only
    /// by the `--keep-unknown` overlay (`keep.rs`, D3); the parity writer
    /// never builds one.
    Raw(String),
}

impl Json {
    /// Build an object, dropping `None` entries (the `NON_EMPTY`/`NON_NULL`
    /// omission already decided by the caller).
    pub fn obj(entries: Vec<(&'static str, Option<Json>)>) -> Json {
        Json::Obj(
            entries
                .into_iter()
                .filter_map(|(k, v)| v.map(|v| (k.to_string(), v)))
                .collect(),
        )
    }

    /// Same, from a borrowed entry list (the writer builds merged key tables
    /// as `Vec<(&str, Option<Json>)>` in FINAL alphabetical order).
    pub fn from_entries(entries: &[(&'static str, Option<Json>)]) -> Json {
        Json::Obj(
            entries
                .iter()
                .filter_map(|(k, v)| v.as_ref().map(|v| ((*k).to_string(), v.clone())))
                .collect(),
        )
    }

    /// `NON_EMPTY` string field: `None`/empty → omitted.
    pub fn text(s: &str) -> Option<Json> {
        if s.is_empty() {
            None
        } else {
            Some(Json::Str(s.to_string()))
        }
    }

    /// Optional `NON_EMPTY` string field.
    pub fn opt_text(s: Option<&str>) -> Option<Json> {
        s.and_then(Json::text)
    }

    /// A `string[]` written with reference set/array semantics.
    pub fn str_array(items: &[String]) -> Json {
        Json::Arr(items.iter().map(|s| Json::Str(s.clone())).collect())
    }

    /// Free-form `properties` map in Java `HashMap` order (V-11).
    pub fn props(props: &Properties) -> Option<Json> {
        if props.is_empty() {
            return None;
        }
        Some(Json::Obj(
            javahash::ordered_properties(props)
                .into_iter()
                .map(|(k, v)| (k.to_string(), Json::Str(v.to_string())))
                .collect(),
        ))
    }
}

/// Serialize to the reference's pretty JSON bytes (no trailing newline).
#[must_use]
pub fn to_bytes(root: &Json) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 * 1024);
    emit(root, 0, &mut out);
    out
}

/// `depth` = the indent level of the line the value STARTS on; object keys
/// and array elements are written at `depth + 1`.
fn emit(value: &Json, depth: usize, out: &mut Vec<u8>) {
    match value {
        Json::Str(s) => escape_into(s, out),
        Json::Num(n) => out.extend_from_slice(n.to_string().as_bytes()),
        Json::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Json::Raw(literal) => out.extend_from_slice(literal.as_bytes()),
        Json::Obj(entries) => {
            if entries.is_empty() {
                out.extend_from_slice(b"{ }");
                return;
            }
            out.push(b'{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.push(b'\n');
                pad(depth + 1, out);
                escape_into(k, out);
                out.extend_from_slice(b" : ");
                emit(v, depth + 1, out);
            }
            out.push(b'\n');
            pad(depth, out);
            out.push(b'}');
        }
        Json::Arr(items) => {
            if items.is_empty() {
                out.extend_from_slice(b"[ ]");
                return;
            }
            let objects = matches!(items[0], Json::Obj(_));
            if !objects {
                // Jackson's `DefaultPrettyPrinter` inlines scalar arrays on
                // one line: `[ "a", "b" ]`.
                out.extend_from_slice(b"[ ");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.extend_from_slice(b", ");
                    }
                    emit(item, depth, out);
                }
                out.extend_from_slice(b" ]");
                return;
            }
            // Object arrays expand: `[ {` … `}, {` … `} ]` — each element
            // object closes at `depth` (its keys at depth+1), joined inline.
            out.extend_from_slice(b"[ ");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.extend_from_slice(b", ");
                }
                emit(item, depth, out);
            }
            out.extend_from_slice(b" ]");
        }
    }
}

fn pad(depth: usize, out: &mut Vec<u8>) {
    out.extend_from_slice(&[b' '; 2].repeat(depth));
}

/// Jackson `JsonStringEncoder` escaping: `"`, `\`, and the named controls;
/// other `< 0x20` as `\u00XX` (UPPERCASE hex); everything else — including
/// `/`, DEL and all non-ASCII — passes through raw UTF-8.
fn escape_into(s: &str, out: &mut Vec<u8>) {
    out.push(b'"');
    for c in s.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(format!("\\u00{:02X}", c as u32).as_bytes());
            }
            c => {
                let mut buf = [0u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::{Json, escape_into, to_bytes};

    fn esc(s: &str) -> String {
        let mut v = Vec::new();
        escape_into(s, &mut v);
        String::from_utf8(v).unwrap()
    }

    #[test]
    fn escaping_matches_jackson_defaults() {
        assert_eq!(esc("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(esc("tab\there\nnl"), "\"tab\\there\\nnl\"");
        // Named controls, not \u, for the five Jackson names:
        assert_eq!(esc("\u{08}\u{0C}\u{0D}"), "\"\\b\\f\\r\"");
        // Other controls: uppercase hex.
        assert_eq!(esc("\u{1}\u{1F}"), "\"\\u0001\\u001F\"");
        // Pass-throughs (Jackson does NOT escape these by default):
        assert_eq!(esc("a/b\u{7F}é漢😀"), "\"a/b\u{7F}é漢😀\"");
    }

    #[test]
    fn empty_containers() {
        assert_eq!(
            String::from_utf8(to_bytes(&Json::obj(vec![]))).unwrap(),
            "{ }"
        );
        assert_eq!(
            String::from_utf8(to_bytes(&Json::Arr(vec![]))).unwrap(),
            "[ ]"
        );
    }

    #[test]
    fn object_shape_matches_golden() {
        let j = Json::obj(vec![
            ("id", Some(Json::Str("1".into()))),
            (
                "elements",
                Some(Json::Arr(vec![Json::obj(vec![
                    ("x", Some(Json::Num(0))),
                    ("y", Some(Json::Num(7))),
                ])])),
            ),
            ("themes", Some(Json::str_array(&["a".into(), "b".into()]))),
        ]);
        // Entries are written in GIVEN order (the caller applies the key
        // sort), with Jackson's `,`-after-each-entry and `" : "` separator:
        let expect = "{\n  \"id\" : \"1\",\n  \"elements\" : [ {\n    \"x\" : 0,\n    \"y\" : 7\n  } ],\n  \"themes\" : [ \"a\", \"b\" ]\n}";
        assert_eq!(String::from_utf8(to_bytes(&j)).unwrap(), expect);
    }
}
