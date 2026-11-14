//! M1 corpus tests: `expect-test` snapshots of the CST/lexer output for the named
//! fixtures (text-block, multi-line*, utf8, bom*) and losslessness properties over
//! every UTF-8 `.dsl` file in the vendored corpus.
//!
//! `iso-8859.dsl` is not valid UTF-8 — decoding is a loader concern (V-8, M5), so
//! the corpus walker skips non-UTF-8 bytes and a dedicated test pins WHY it is
//! skipped.
// The inline `expect![[…]]` snapshot data counts toward clippy's function-length
// limit; the data-heavy snapshot fns are exempt (their logic is two lines).
#![allow(clippy::too_many_lines)]

use std::path::{Path, PathBuf};

use expect_test::expect;
use oxidizr_ast::{Cst, Node};
use oxidizr_dsl::cst;
use oxidizr_dsl::lexer;
use oxidizr_dsl::preprocess::{self, TextBlock};

fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/dsl")
}

fn read(name: &str) -> String {
    let path = testdata().join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    String::from_utf8(bytes).unwrap_or_else(|_| panic!("{name} must be UTF-8 (M1 scope)"))
}

/// Stable tree dump: one line per node (`kind start..end "text"` for leaves), then
/// per logical line the token values and text-block values (the reference's view).
fn dump(source: &str) -> String {
    let tree = cst::build(source);
    assert_eq!(tree.verify_lossless(), Ok(()), "CST must be lossless");

    let mut lines = Vec::new();
    dump_node(&tree, &tree.root, 0, &mut lines);
    for logical in &preprocess::preprocess(source) {
        let is_code = logical.text_block.is_some()
            || (!logical.is_empty_line(source) && !logical.is_comment_line(source));
        let mut tail = String::new();
        if is_code {
            for t in &lexer::lex_line(source, logical).tokens {
                tail.push_str(" | ");
                tail.push_str(&escape_preview(&t.value(source)));
            }
        }
        if let Some(tb) = logical.text_block {
            tail.push_str(&format_text_block(source, &tb));
        }
        lines.push(format!("L{}{}", logical.number, tail));
    }
    lines.join("\n") + "\n"
}

fn format_text_block(source: &str, tb: &TextBlock) -> String {
    match preprocess::text_block_value(source, tb) {
        Some(v) => format!(" | text-block = {}", escape_preview(&v)),
        None => " | text-block = <reference throws; modeled as None>".to_string(),
    }
}

fn dump_node(tree: &Cst, node: &Node, depth: usize, out: &mut Vec<String>) {
    let indent = "  ".repeat(depth);
    if node.children.is_empty() {
        out.push(format!(
            "{indent}{:?} {}..{} {}",
            node.kind,
            node.range.start,
            node.range.end,
            escape_preview(tree.text(node))
        ));
    } else {
        out.push(format!(
            "{indent}{:?} {}..{}",
            node.kind, node.range.start, node.range.end
        ));
        for child in &node.children {
            dump_node(tree, child, depth + 1, out);
        }
    }
}

/// Escape control chars and truncate: snapshots must stay readable and stable.
fn escape_preview(text: &str) -> String {
    let mut s: String = text.chars().take(60).collect();
    s = s
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t");
    if text.chars().count() > 60 {
        s.push('…');
    }
    format!("\"{s}\"")
}

#[test]
fn snapshot_multi_line() {
    // Continuations: middle lines keep indentation; the group reports the LAST
    // physical line's number; token hulls split across the joins.
    expect![[r#"
        Root 0..137
          LogicalLine 0..12
            TokenBare 0..9 "workspace"
            Whitespace 9..10 " "
            TokenBare 10..11 "{"
            Newline 11..12 "\n"
          BlankLine 12..13
            Newline 12..13 "\n"
          LogicalLine 13..25
            Whitespace 13..17 "    "
            TokenBare 17..22 "model"
            Whitespace 22..23 " "
            TokenBare 23..24 "{"
            Newline 24..25 "\n"
          LogicalLine 25..125
            Whitespace 25..33 "        "
            TokenBare 33..47 "softwareSystem"
            Whitespace 47..48 " "
            TokenBare 48..49 "="
            Whitespace 49..50 " "
            Continuation 50..51 "\\"
            Newline 51..52 "\n"
            Whitespace 52..64 "            "
            TokenBare 64..78 "softwareSystem"
            Whitespace 78..79 " "
            Continuation 79..80 "\\"
            Newline 80..81 "\n"
            Whitespace 81..93 "            "
            TokenQuoted 93..103 ""Software "
            Continuation 103..104 "\\"
            Newline 104..105 "\n"
            Whitespace 105..117 "            "
            TokenQuoted 117..124 "System""
            Newline 124..125 "\n"
          LogicalLine 125..131
            Whitespace 125..129 "    "
            TokenBare 129..130 "}"
            Newline 130..131 "\n"
          BlankLine 131..136
            Whitespace 131..135 "    "
            Newline 135..136 "\n"
          LogicalLine 136..137
            TokenBare 136..137 "}"
        L1 | "workspace" | "{"
        L2
        L3 | "model" | "{"
        L7 | "softwareSystem" | "=" | "softwareSystem" | "Software System"
        L8 | "}"
        L9
        L10 | "}"
    "#]]
    .assert_eq(&dump(&read("multi-line.dsl")));
}

#[test]
fn snapshot_text_block() {
    expect![[r#"
        Root 0..532
          LogicalLine 0..12
            TokenBare 0..9 "workspace"
            Whitespace 9..10 " "
            TokenBare 10..11 "{"
            Newline 11..12 "\n"
          BlankLine 12..13
            Newline 12..13 "\n"
          LogicalLine 13..25
            Whitespace 13..17 "    "
            TokenBare 17..22 "views"
            Whitespace 22..23 " "
            TokenBare 23..24 "{"
            Newline 24..25 "\n"
          LogicalLine 25..46
            Whitespace 25..33 "        "
            TokenBare 33..43 "properties"
            Whitespace 43..44 " "
            TokenBare 44..45 "{"
            Newline 45..46 "\n"
          LogicalLine 46..105
            Whitespace 46..58 "            "
            TokenQuoted 58..72 ""plantuml.url""
            Whitespace 72..73 " "
            TokenQuoted 73..104 ""https://plantuml.com/plantuml""
            Newline 104..105 "\n"
          LogicalLine 105..115
            Whitespace 105..113 "        "
            TokenBare 113..114 "}"
            Newline 114..115 "\n"
          BlankLine 115..116
            Newline 115..116 "\n"
          LogicalLine 116..184
            Whitespace 116..124 "        "
            TokenBare 124..130 "!const"
            Whitespace 130..131 " "
            TokenBare 131..137 "SOURCE"
            Whitespace 137..138 " "
            TextBlockRaw 138..184 """"\n            class MyClass\n            """\n"
          BlankLine 184..185
            Newline 184..185 "\n"
          LogicalLine 185..337
            Whitespace 185..193 "        "
            TokenBare 193..197 "!var"
            Whitespace 197..198 " "
            TokenBare 198..204 "STYLES"
            Whitespace 204..205 " "
            TextBlockRaw 205..337 """"\n            <style>\n            root {\n                B…"
          BlankLine 337..338
            Newline 337..338 "\n"
          LogicalLine 338..364
            Whitespace 338..346 "        "
            TokenBare 346..351 "image"
            Whitespace 351..352 " "
            TokenBare 352..353 "*"
            Whitespace 353..354 " "
            TokenQuoted 354..361 ""image""
            Whitespace 361..362 " "
            TokenBare 362..363 "{"
            Newline 363..364 "\n"
          LogicalLine 364..514
            Whitespace 364..376 "            "
            TokenBare 376..384 "plantuml"
            Whitespace 384..385 " "
            TextBlockRaw 385..514 """"\n                 @startuml\n\n                 ${STYLES}\n\n…"
          LogicalLine 514..524
            Whitespace 514..522 "        "
            TokenBare 522..523 "}"
            Newline 523..524 "\n"
          LogicalLine 524..530
            Whitespace 524..528 "    "
            TokenBare 528..529 "}"
            Newline 529..530 "\n"
          BlankLine 530..531
            Newline 530..531 "\n"
          LogicalLine 531..532
            TokenBare 531..532 "}"
        L1 | "workspace" | "{"
        L2
        L3 | "views" | "{"
        L4 | "properties" | "{"
        L5 | "plantuml.url" | "https://plantuml.com/plantuml"
        L6 | "}"
        L7
        L10 | "!const" | "SOURCE" | text-block = "class MyClass"
        L11
        L18 | "!var" | "STYLES" | text-block = "<style>\nroot {\n    BackgroundColor: #ffffff;\n}\n</style>"
        L19
        L20 | "image" | "*" | "image" | "{"
        L28 | "plantuml" | text-block = "@startuml\n\n${STYLES}\n\n${SOURCE}\n@enduml"
        L29 | "}"
        L30 | "}"
        L31
        L32 | "}"
    "#]]
    .assert_eq(&dump(&read("text-block.dsl")));
}

#[test]
fn snapshot_utf8() {
    expect![[r#"
        Root 0..75
          LogicalLine 0..12
            TokenBare 0..9 "workspace"
            Whitespace 9..10 " "
            TokenBare 10..11 "{"
            Newline 11..12 "\n"
          BlankLine 12..13
            Newline 12..13 "\n"
          LogicalLine 13..25
            Whitespace 13..17 "    "
            TokenBare 17..22 "model"
            Whitespace 22..23 " "
            TokenBare 23..24 "{"
            Newline 24..25 "\n"
          LogicalLine 25..67
            Whitespace 25..33 "        "
            TokenBare 33..37 "user"
            Whitespace 37..38 " "
            TokenBare 38..39 "="
            Whitespace 39..40 " "
            TokenBare 40..46 "person"
            Whitespace 46..47 " "
            TokenQuoted 47..66 ""你好 Usér 🙂""
            Newline 66..67 "\n"
          LogicalLine 67..73
            Whitespace 67..71 "    "
            TokenBare 71..72 "}"
            Newline 72..73 "\n"
          BlankLine 73..74
            Newline 73..74 "\n"
          LogicalLine 74..75
            TokenBare 74..75 "}"
        L1 | "workspace" | "{"
        L2
        L3 | "model" | "{"
        L4 | "user" | "=" | "person" | "你好 Usér 🙂"
        L5 | "}"
        L6
        L7 | "}"
    "#]]
    .assert_eq(&dump(&read("utf8.dsl")));
}

#[test]
fn snapshot_bom() {
    expect![[r##"
        Root 0..576
          LogicalLine 0..74
            Bom 0..3 "﻿"
            TokenBare 3..12 "workspace"
            Whitespace 12..13 " "
            TokenQuoted 13..30 ""Getting Started""
            Whitespace 30..31 " "
            TokenQuoted 31..71 ""This is a model of my software system.""
            Whitespace 71..72 " "
            TokenBare 72..73 "{"
            Newline 73..74 "\n"
          BlankLine 74..75
            Newline 74..75 "\n"
          LogicalLine 75..117
            Whitespace 75..79 "    "
            TokenBare 79..87 "!include"
            Whitespace 87..88 " "
            TokenBare 88..116 "workspace-with-bom-model.dsl"
            Newline 116..117 "\n"
          BlankLine 117..118
            Newline 117..118 "\n"
          LogicalLine 118..130
            Whitespace 118..122 "    "
            TokenBare 122..127 "views"
            Whitespace 127..128 " "
            TokenBare 128..129 "{"
            Newline 129..130 "\n"
          LogicalLine 130..227
            Whitespace 130..138 "        "
            TokenBare 138..151 "systemContext"
            Whitespace 151..152 " "
            TokenBare 152..166 "softwareSystem"
            Whitespace 166..167 " "
            TokenQuoted 167..182 ""SystemContext""
            Whitespace 182..183 " "
            TokenQuoted 183..224 ""An example of a System Context diagram.""
            Whitespace 224..225 " "
            TokenBare 225..226 "{"
            Newline 226..227 "\n"
          LogicalLine 227..249
            Whitespace 227..239 "            "
            TokenBare 239..246 "include"
            Whitespace 246..247 " "
            TokenBare 247..248 "*"
            Newline 248..249 "\n"
          LogicalLine 249..272
            Whitespace 249..261 "            "
            TokenBare 261..271 "autoLayout"
            Newline 271..272 "\n"
          LogicalLine 272..282
            Whitespace 272..280 "        "
            TokenBare 280..281 "}"
            Newline 281..282 "\n"
          BlankLine 282..283
            Newline 282..283 "\n"
          LogicalLine 283..300
            Whitespace 283..291 "        "
            TokenBare 291..297 "styles"
            Whitespace 297..298 " "
            TokenBare 298..299 "{"
            Newline 299..300 "\n"
          LogicalLine 300..340
            Whitespace 300..312 "            "
            TokenBare 312..319 "element"
            Whitespace 319..320 " "
            TokenQuoted 320..337 ""Software System""
            Whitespace 337..338 " "
            TokenBare 338..339 "{"
            Newline 339..340 "\n"
          LogicalLine 340..375
            Whitespace 340..356 "                "
            TokenBare 356..366 "background"
            Whitespace 366..367 " "
            TokenBare 367..374 "#1168bd"
            Newline 374..375 "\n"
          LogicalLine 375..405
            Whitespace 375..391 "                "
            TokenBare 391..396 "color"
            Whitespace 396..397 " "
            TokenBare 397..404 "#ffffff"
            Newline 404..405 "\n"
          LogicalLine 405..419
            Whitespace 405..417 "            "
            TokenBare 417..418 "}"
            Newline 418..419 "\n"
          LogicalLine 419..450
            Whitespace 419..431 "            "
            TokenBare 431..438 "element"
            Whitespace 438..439 " "
            TokenQuoted 439..447 ""Person""
            Whitespace 447..448 " "
            TokenBare 448..449 "{"
            Newline 449..450 "\n"
          LogicalLine 450..479
            Whitespace 450..466 "                "
            TokenBare 466..471 "shape"
            Whitespace 471..472 " "
            TokenBare 472..478 "person"
            Newline 478..479 "\n"
          LogicalLine 479..514
            Whitespace 479..495 "                "
            TokenBare 495..505 "background"
            Whitespace 505..506 " "
            TokenBare 506..513 "#08427b"
            Newline 513..514 "\n"
          LogicalLine 514..544
            Whitespace 514..530 "                "
            TokenBare 530..535 "color"
            Whitespace 535..536 " "
            TokenBare 536..543 "#ffffff"
            Newline 543..544 "\n"
          LogicalLine 544..558
            Whitespace 544..556 "            "
            TokenBare 556..557 "}"
            Newline 557..558 "\n"
          LogicalLine 558..568
            Whitespace 558..566 "        "
            TokenBare 566..567 "}"
            Newline 567..568 "\n"
          LogicalLine 568..574
            Whitespace 568..572 "    "
            TokenBare 572..573 "}"
            Newline 573..574 "\n"
          BlankLine 574..575
            Newline 574..575 "\n"
          LogicalLine 575..576
            TokenBare 575..576 "}"
        L1 | "workspace" | "Getting Started" | "This is a model of my software system." | "{"
        L2
        L3 | "!include" | "workspace-with-bom-model.dsl"
        L4
        L5 | "views" | "{"
        L6 | "systemContext" | "softwareSystem" | "SystemContext" | "An example of a System Context diagram." | "{"
        L7 | "include" | "*"
        L8 | "autoLayout"
        L9 | "}"
        L10
        L11 | "styles" | "{"
        L12 | "element" | "Software System" | "{"
        L13 | "background" | "#1168bd"
        L14 | "color" | "#ffffff"
        L15 | "}"
        L16 | "element" | "Person" | "{"
        L17 | "shape" | "person"
        L18 | "background" | "#08427b"
        L19 | "color" | "#ffffff"
        L20 | "}"
        L21 | "}"
        L22 | "}"
        L23
        L24 | "}"
    "##]]
    .assert_eq(&dump(&read("workspace-with-bom.dsl")));
}

#[test]
fn snapshot_bom_model() {
    expect![[r#"
        Root 0..218
          LogicalLine 0..15
            Bom 0..3 "﻿"
            Whitespace 3..7 "    "
            TokenBare 7..12 "model"
            Whitespace 12..13 " "
            TokenBare 13..14 "{"
            Newline 14..15 "\n"
          LogicalLine 15..76
            Whitespace 15..23 "        "
            TokenBare 23..27 "user"
            Whitespace 27..28 " "
            TokenBare 28..29 "="
            Whitespace 29..30 " "
            TokenBare 30..36 "person"
            Whitespace 36..37 " "
            TokenQuoted 37..43 ""User""
            Whitespace 43..44 " "
            TokenQuoted 44..75 ""A user of my software system.""
            Newline 75..76 "\n"
          LogicalLine 76..174
            Whitespace 76..84 "        "
            TokenBare 84..98 "softwareSystem"
            Whitespace 98..99 " "
            TokenBare 99..100 "="
            Whitespace 100..101 " "
            TokenBare 101..115 "softwareSystem"
            Whitespace 115..116 " "
            TokenQuoted 116..133 ""Software System""
            Whitespace 133..134 " "
            TokenQuoted 134..173 ""My software system, code-named \\"X\\".""
            Newline 173..174 "\n"
          BlankLine 174..175
            Newline 174..175 "\n"
          LogicalLine 175..213
            Whitespace 175..183 "        "
            TokenBare 183..187 "user"
            Whitespace 187..188 " "
            TokenBare 188..190 "->"
            Whitespace 190..191 " "
            TokenBare 191..205 "softwareSystem"
            Whitespace 205..206 " "
            TokenQuoted 206..212 ""Uses""
            Newline 212..213 "\n"
          LogicalLine 213..218
            Whitespace 213..217 "    "
            TokenBare 217..218 "}"
        L1 | "model" | "{"
        L2 | "user" | "=" | "person" | "User" | "A user of my software system."
        L3 | "softwareSystem" | "=" | "softwareSystem" | "Software System" | "My software system, code-named \\"X\\"."
        L4
        L5 | "user" | "->" | "softwareSystem" | "Uses"
        L6 | "}"
    "#]]
    .assert_eq(&dump(&read("workspace-with-bom-model.dsl")));
}

#[test]
fn snapshot_multi_line_with_error() {
    // A GRAMMAR error (M4 territory); lexes clean at M1.
    expect![[r#"
        Root 0..243
          LogicalLine 0..12
            TokenBare 0..9 "workspace"
            Whitespace 9..10 " "
            TokenBare 10..11 "{"
            Newline 11..12 "\n"
          BlankLine 12..13
            Newline 12..13 "\n"
          LogicalLine 13..25
            Whitespace 13..17 "    "
            TokenBare 17..22 "model"
            Whitespace 22..23 " "
            TokenBare 23..24 "{"
            Newline 24..25 "\n"
          LogicalLine 25..127
            Whitespace 25..33 "        "
            TokenBare 33..47 "softwareSystem"
            Whitespace 47..48 " "
            TokenBare 48..49 "="
            Whitespace 49..50 " "
            Continuation 50..51 "\\"
            Newline 51..52 "\n"
            Whitespace 52..64 "            "
            TokenBare 64..78 "softwareSystem"
            Whitespace 78..79 " "
            Continuation 79..80 "\\"
            Newline 80..81 "\n"
            Whitespace 81..93 "            "
            TokenQuoted 93..103 ""Software "
            Continuation 103..104 "\\"
            Newline 104..105 "\n"
            Whitespace 105..117 "            "
            TokenQuoted 117..124 "System""
            Whitespace 124..125 " "
            TokenBare 125..126 "{"
            Newline 126..127 "\n"
          LogicalLine 127..217
            Whitespace 127..143 "                "
            TokenBare 143..152 "component"
            Whitespace 152..153 " "
            TokenQuoted 153..164 ""Component""
            Whitespace 164..165 " "
            TokenBare 165..167 "//"
            Whitespace 167..168 " "
            TokenBare 168..178 "components"
            Whitespace 178..179 " "
            TokenBare 179..182 "not"
            Whitespace 182..183 " "
            TokenBare 183..192 "permitted"
            Whitespace 192..193 " "
            TokenBare 193..199 "inside"
            Whitespace 199..200 " "
            TokenBare 200..208 "software"
            Whitespace 208..209 " "
            TokenBare 209..216 "systems"
            Newline 216..217 "\n"
          LogicalLine 217..231
            Whitespace 217..229 "            "
            TokenBare 229..230 "}"
            Newline 230..231 "\n"
          LogicalLine 231..237
            Whitespace 231..235 "    "
            TokenBare 235..236 "}"
            Newline 236..237 "\n"
          BlankLine 237..242
            Whitespace 237..241 "    "
            Newline 241..242 "\n"
          LogicalLine 242..243
            TokenBare 242..243 "}"
        L1 | "workspace" | "{"
        L2
        L3 | "model" | "{"
        L7 | "softwareSystem" | "=" | "softwareSystem" | "Software System" | "{"
        L8 | "component" | "Component" | "//" | "components" | "not" | "permitted" | "inside" | "software" | "systems"
        L9 | "}"
        L10 | "}"
        L11
        L12 | "}"
    "#]]
    .assert_eq(&dump(&read("multi-line-with-error.dsl")));
}

#[test]
fn iso8859_fixture_is_not_utf8() {
    // V-8 (loader, M5): the reference reads with a configurable charset; M1 lexes
    // `&str` only. Pin that the file is skipped for a reason, not by accident.
    let path = testdata().join("iso-8859.dsl");
    let bytes = std::fs::read(&path).expect("fixture exists");
    assert!(
        std::str::from_utf8(&bytes).is_err(),
        "iso-8859.dsl must be non-UTF-8"
    );
}

#[test]
fn corpus_cst_is_lossless() {
    let mut checked = 0_usize;
    let mut skipped_non_utf8 = 0_usize;
    for path in walk_dsl(&testdata()) {
        let bytes = std::fs::read(&path).unwrap();
        let Ok(source) = std::str::from_utf8(&bytes) else {
            skipped_non_utf8 += 1; // V-8: loader-level decoding (M5), not the lexer
            continue;
        };
        let tree = cst::build(source);
        tree.verify_lossless()
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        checked += 1;
    }
    // The vendored corpus is 107 files; keep a floor so a broken walker never
    // silently vacates the property.
    assert!(checked >= 100, "only {checked} files checked");
    assert_eq!(skipped_non_utf8, 1, "exactly iso-8859.dsl skips");
}

#[test]
fn corpus_tokens_are_deterministic_and_spanned() {
    // Second property: token pieces are in-bounds char-aligned spans, and lexing
    // twice is identical.
    for path in walk_dsl(&testdata()) {
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        for logical in preprocess::preprocess(&source) {
            let a = lexer::lex_line(&source, &logical);
            let b = lexer::lex_line(&source, &logical);
            assert_eq!(a, b, "{}", path.display());
            for t in &a.tokens {
                for p in &t.pieces {
                    let (s, e) = (p.start as usize, p.end as usize);
                    assert!(s <= e && e <= source.len(), "piece bounds");
                    assert!(source.is_char_boundary(s) && source.is_char_boundary(e));
                }
            }
        }
    }
}

fn walk_dsl(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "dsl") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}
