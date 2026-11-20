//! The `ParseResult::includes` file set (§P3-M4): every file-system
//! `!include`d file, in read order, canonical PATH form — the closure
//! the live-reload watcher must cover. Fragment parses record nothing
//! (includes are silently ignored without a `dslFile`, reference
//! parity); the main file itself is NOT part of the set.

use std::path::{Path, PathBuf};

use oxidizr_dsl::engine::{ParseOptions, parse_dsl};

#[allow(clippy::unwrap_used)]
fn temp_dir(topic: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "oxidizr-m4-includes-{topic}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[allow(clippy::unwrap_used)]
fn parse_main(dir: &Path) -> Vec<String> {
    let main = dir.join("main.dsl");
    let source = std::fs::read_to_string(&main).unwrap();
    let result = parse_dsl(&source, ParseOptions::file(&main.to_string_lossy())).unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    result.includes
}

#[allow(clippy::unwrap_used)]
#[test]
fn includes_records_nested_and_directory_reads_in_order() {
    let dir = temp_dir("nested");
    std::fs::write(
        dir.join("main.dsl"),
        "workspace \"I\" {\n    model {\n        !include child.dsl\n        !include parts\n    }\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("child.dsl"),
        "a = person \"A\"\n    !include deep/grand.dsl\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("deep")).unwrap();
    std::fs::write(dir.join("deep/grand.dsl"), "b = person \"B\"\n").unwrap();
    std::fs::create_dir_all(dir.join("parts")).unwrap();
    std::fs::write(dir.join("parts/zeta.dsl"), "c = person \"C\"\n").unwrap();
    std::fs::write(dir.join("parts/alpha.dsl"), "d = person \"D\"\n").unwrap();

    let includes = parse_main(&dir);
    // Parse order: the child (and its nested grand) before the directory
    // entries; directory recursion is Arrays.sort order (alpha, zeta).
    let names: Vec<String> = includes
        .iter()
        .map(|p| {
            PathBuf::from(p)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(names, ["child.dsl", "grand.dsl", "alpha.dsl", "zeta.dsl"]);
    // Canonical PATH form: absolute, cwd-joined, lexical (no `..`).
    for path in &includes {
        assert!(PathBuf::from(path).is_absolute(), "{path}");
        assert!(!path.contains(".."), "{path}");
    }
    assert!(includes[0].starts_with(&*dir.to_string_lossy()));
}

#[allow(clippy::unwrap_used)]
#[test]
fn fragment_and_include_free_parses_record_nothing() {
    let dir = temp_dir("empty");
    std::fs::write(dir.join("main.dsl"), "workspace \"I\" {\n}\n").unwrap();
    let none: Vec<String> = Vec::new();
    assert_eq!(parse_main(&dir), none);
    // Fragment: the include is silently ignored (no dslFile) — nothing
    // can be recorded either.
    let result = parse_dsl(
        "workspace \"I\" {\n    !include nope.dsl\n}\n",
        ParseOptions::fragment(),
    )
    .unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(result.includes, none);
}
