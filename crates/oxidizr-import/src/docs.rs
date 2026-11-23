//! The `!docs` documentation importers — ports of `structurizr-import`
//! `documentation/DefaultDocumentationImporter`,
//! `RecursiveDefaultDocumentationImporter`, `FormatFinder` and
//! `DefaultImageImporter`, plus `DocumentationContent.findImages`
//! (`structurizr-core`). The DSL wiring (`!docs <path> [fqn]`, block form
//! with `exclude …`) lives in `oxidizr-dsl`; this module is the file
//! system half, run when the DSL context closes.
//!
//! Pinned behaviors (full contract: the import catalog):
//! - the joined path resolves against the DSL file (the official tooling
//!   joins the parent directory with the path — oxidizr JOINS, so an
//!   absolute-looking token concatenates exactly like the official tooling’s
//!   string join of parent and child);
//! - a directory import walks `listFiles()` sorted by path, skipping
//!   directories and dot-prefixed names; a lone FILE is imported
//!   regardless of extension (`FormatFinder` “just assume Markdown”);
//! - sections carry `order = sections.size() + 1` AT ADD TIME and the
//!   canonical filename, which is then TRIMMED against the import root —
//!   including sections added by EARLIER imports of the same documentable
//!   (the official tooling trims the WHOLE section list); the recursive
//!   variant also maps `\` to `/`;
//! - `exclude` is `name.equals(x) || name.matches(x)` — a FULL-MATCH
//!   REGEX (`regexlite`), evaluated against the file NAME only;
//! - the reluctant image importer (auto-run after a DIRECTORY import)
//!   walks recursively in sorted order (the official tooling uses FS order —
//!   non-deterministic there; sorted is oxidizr's registered choice),
//!   keeps `<subdir>/<file>.<ext>` names, and embeds RAW file bytes
//!   (a documented deviation — the official tooling re-encodes non-SVG).

use std::collections::BTreeSet;
use std::path::Path;

use base64::Engine as _;
use oxidizr_core::docs::{Documentation, Format, Image, Section};

use crate::image::content_type_of;
use crate::regexlite::Regex;
use crate::{ImportError, java_absolute, java_canonical, normalize_newlines};

/// Which documentation importer the DSL selected (`!docs` default is
/// [`DocsKind::Default`]; the recursive variant is FQN-reachable only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocsKind {
    /// `DefaultDocumentationImporter` — top-level directory entries only.
    Default,
    /// `RecursiveDefaultDocumentationImporter` — descending.
    Recursive,
}

/// `FormatFinder.isMarkdownOrAsciiDoc` — extension from the LAST dot,
/// case-sensitive, membership in either set.
#[must_use]
pub fn is_markdown_or_asciidoc(name: &str) -> bool {
    let Some(dot) = name.rfind('.') else {
        return false;
    };
    markdown_ext(&name[dot..]) || asciidoc_ext(&name[dot..])
}

fn markdown_ext(ext: &str) -> bool {
    matches!(ext, ".md" | ".markdown" | ".text")
}

fn asciidoc_ext(ext: &str) -> bool {
    matches!(ext, ".asciidoc" | ".adoc" | ".asc")
}

/// `FormatFinder.findFormat` — Markdown/AsciiDoc by extension, anything
/// else “just assume Markdown”.
#[must_use]
pub fn find_format(name: &str) -> Format {
    let ext = match name.rfind('.') {
        Some(dot) => &name[dot..],
        None => "",
    };
    if asciidoc_ext(ext) {
        Format::AsciiDoc
    } else {
        Format::Markdown
    }
}

/// `DefaultDocumentationImporter.excluded` / `AbstractDecisionImporter
/// .excluded` — equality short-circuit, then a FULL-MATCH regex per
/// exclude. An uncompilable regex is surfaced (the official tooling lets
/// a regex-compile abort the import).
pub(crate) fn excluded(name: &str, excludes: &[String]) -> Result<bool, ImportError> {
    for x in excludes {
        if name == x {
            return Ok(true);
        }
        let re = Regex::new(x)
            .map_err(|e| ImportError::Failed(format!("invalid exclude pattern `{x}`: {e}")))?;
        if re.is_match_full(name) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `importDocumentation(documentable, path)` for the default/recursive
/// importers. `path` is the JOINED path (DSL dir + token); `excludes`
/// are the context's `exclude` tokens.
///
/// # Errors
///
/// [`ImportError::Failed`] carries the official messages verbatim
/// (`"<abs> does not exist."`); [`ImportError::Io`] for read failures
/// (the DSL layer surfaces them as import failures).
pub fn import_docs(
    kind: DocsKind,
    target: &mut Documentation,
    path: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    // the recursive variant's message carries NO trailing period (verified)
    if !path.exists() {
        return Err(ImportError::Failed(format!(
            "{} does not exist{}",
            java_absolute(path).display(),
            if kind == DocsKind::Recursive { "" } else { "." }
        )));
    }
    if path.is_dir() {
        match kind {
            DocsKind::Default => walk_files(target, path, excludes)?,
            DocsKind::Recursive => walk_files_recursive(target, path, excludes)?,
        }
    } else {
        import_file(target, path, excludes)?;
    }
    // "now trim the filenames" — EVERY section of this documentable,
    // including ones earlier imports added (verified).
    let base = java_canonical(path);
    let base_s = base.to_string_lossy().into_owned();
    for s in &mut target.sections {
        let mut filename = s.filename.clone().unwrap_or_default();
        filename = filename.replace(&base_s, "");
        if filename.starts_with('/') {
            filename.remove(0); // Java: substring(1) after the separator check
        }
        if kind == DocsKind::Recursive {
            filename = filename.replace('\\', "/");
        }
        s.filename = Some(filename);
    }
    Ok(())
}

fn walk_files(
    target: &mut Documentation,
    dir: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    for file in sorted_files(dir)? {
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        // `!file.isDirectory() && !file.getName().startsWith(".")`
        if !file.is_dir() && !name.starts_with('.') {
            import_file(target, &file, excludes)?;
        }
    }
    Ok(())
}

fn walk_files_recursive(
    target: &mut Documentation,
    dir: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    for file in sorted_files(dir)? {
        let name = file
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if file.is_dir() {
            // the official tooling recurses into dot-directories too (the dot
            // test only gates FILES)
            walk_files_recursive(target, &file, excludes)?;
        } else if !name.starts_with('.') {
            import_file(target, &file, excludes)?;
        }
    }
    Ok(())
}

/// Directory entries sorted by path — `Arrays.sort(File[])`
/// (`File::compareTo` compares path strings).
fn sorted_files(dir: &Path) -> Result<Vec<std::path::PathBuf>, ImportError> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|source| ImportError::Io {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in rd {
        let entry = entry.map_err(|source| ImportError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        out.push(entry.path());
    }
    out.sort();
    Ok(out)
}

/// `importFile` — `FormatFinder.isMarkdownOrAsciiDoc(file) && !excluded`
/// gates, then `{format, content (UTF-8, newline-normalized), canonical
/// filename, order}` appended with the official add-section order rule.
fn import_file(
    target: &mut Documentation,
    file: &Path,
    excludes: &[String],
) -> Result<(), ImportError> {
    let name = file
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    if !is_markdown_or_asciidoc(&name) {
        return Ok(());
    }
    if excluded(&name, excludes)? {
        return Ok(());
    }
    let content = read_utf8(file)?;
    target.sections.push(Section {
        order: i32::try_from(target.sections.len() + 1).unwrap_or(i32::MAX),
        filename: Some(java_canonical(file).to_string_lossy().into_owned()),
        content: normalize_newlines(&content),
        format: Some(find_format(&name)),
        element_id: None,
    });
    Ok(())
}

/// UTF-8 read emulating the official tooling’s LOSSY decode:
/// invalid sequences become U+FFFD, the read never fails on content.
pub(crate) fn read_utf8(path: &Path) -> Result<String, ImportError> {
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// `DocumentationContent.findImages` — the Markdown `!\[.*]\((.*)\)` or
/// `AsciiDoc` `image:{1,2}(.*)\[.*]` group-1 captures over the content
/// (`format == Markdown` picks Markdown; NULL format takes the `AsciiDoc`
/// branch — verified).
pub(crate) fn find_images(content: &str, format: Option<Format>) -> BTreeSet<String> {
    let pattern = if format == Some(Format::Markdown) {
        r"!\[.*]\((.*)\)"
    } else {
        r"image:{1,2}(.*)\[.*]"
    };
    let Ok(re) = Regex::new(pattern) else {
        return BTreeSet::new();
    };
    re.find_iter(content)
        .filter_map(|m| m.group(content, 1).map(str::to_owned))
        .collect()
}

/// The referenced-image set of the WHOLE documentation tree — the
/// reluctant image-importer seed: sections first, then decisions
/// (the official scan order).
fn referenced_images(target: &Documentation) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in &target.sections {
        out.extend(find_images(&s.content, s.format));
    }
    for d in &target.decisions {
        out.extend(find_images(&d.content, d.format));
    }
    out
}

/// `DefaultImageImporter.importDocumentation` — `<subdir>/<name>` keyed
/// images, `.png .jpg .jpeg .gif .svg` on the LOWERCASED name, recursive
/// walk (hidden directories skipped, hidden FILES are NOT), standard
/// base64 of the RAW bytes (a documented deviation), reluctant mode keeping only
/// the image-referenced names.
///
/// # Errors
///
/// [`ImportError::Failed`] for the official tooling `"<abs> does not exist."`;
/// [`ImportError::Io`] for read failures.
pub fn import_images(
    target: &mut Documentation,
    path: &Path,
    reluctant: bool,
) -> Result<(), ImportError> {
    if !path.exists() {
        return Err(ImportError::Failed(format!(
            "{} does not exist.",
            java_absolute(path).display()
        )));
    }
    let referenced = if reluctant {
        referenced_images(target)
    } else {
        BTreeSet::new()
    };
    if path.is_dir() {
        import_images_dir(target, path, "", &referenced, reluctant)?;
    } else {
        import_one_image(target, "", path, &referenced, reluctant)?;
    }
    Ok(())
}

fn import_images_dir(
    target: &mut Documentation,
    dir: &Path,
    root: &str,
    referenced: &BTreeSet<String>,
    reluctant: bool,
) -> Result<(), ImportError> {
    for entry in sorted_files(dir)? {
        let name = entry
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if entry.is_dir() && !name.starts_with('.') {
            // `File.isHidden()` on Unix is the dot prefix
            let next_root = if root.is_empty() {
                name
            } else {
                format!("{root}/{name}")
            };
            import_images_dir(target, &entry, &next_root, referenced, reluctant)?;
        } else if IMAGE_SUFFIXES
            .iter()
            .any(|s| name.to_lowercase().ends_with(s))
        {
            import_one_image(target, root, &entry, referenced, reluctant)?;
        }
    }
    Ok(())
}

const IMAGE_SUFFIXES: [&str; 5] = [".png", ".jpg", ".jpeg", ".gif", ".svg"];

fn import_one_image(
    target: &mut Documentation,
    root: &str,
    file: &Path,
    referenced: &BTreeSet<String>,
    reluctant: bool,
) -> Result<(), ImportError> {
    let name = file
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let full = if root.is_empty() {
        name.clone()
    } else {
        format!("{root}/{name}")
    };
    if reluctant && !referenced.contains(&full) {
        return Ok(());
    }
    // The content-type gate: the guess table must
    // resolve an `image/*` type (`content_type_of` is that table); then
    // the base64 payload — SVG raw bytes ARE the official tooling behavior, the
    // non-SVG image re-encode is a documented deviation (raw bytes here too).
    let content_type = content_type_of(&name).ok_or_else(|| {
        ImportError::Failed(format!(
            "{} is not a supported image file.",
            java_absolute(file).display()
        ))
    })?;
    let bytes = std::fs::read(file).map_err(|source| ImportError::Io {
        path: file.to_path_buf(),
        source,
    })?;
    let payload = base64::engine::general_purpose::STANDARD.encode(&bytes);
    target.images.push(Image {
        name: full,
        content: payload,
        content_type: Some(content_type.to_owned()),
    });
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Scratch dir per test: `<tag>` under `target/tmp/p4m2f-docs/`.
    pub(crate) fn scratch(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/tmp/p4m2f-docs")
            .join(format!("{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (name, body) in files {
            let path = dir.join(name);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            std::fs::write(path, body).expect("write");
        }
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir.canonicalize().expect("canonical")
    }

    #[test]
    fn format_finder_is_case_sensitive_last_dot() {
        assert!(is_markdown_or_asciidoc("a.md"));
        assert!(!is_markdown_or_asciidoc("a.MD")); // case-sensitive: `.MD` is NOT in the sets
        assert!(is_markdown_or_asciidoc("x.y.adoc"));
        assert!(!is_markdown_or_asciidoc("noext"));
        assert_eq!(find_format("a.asciidoc"), Format::AsciiDoc);
        assert_eq!(find_format("a.txt"), Format::Markdown); // "just assume Markdown"
    }

    #[test]
    fn default_import_walks_top_level_sorted_with_order_and_trim() {
        let dir = scratch(
            "default",
            &[
                ("02-second.md", "second\n"),
                ("01-first.adoc", "first\n"),
                ("skip.txt", "no"),
                (".hidden.md", "hidden\n"),
                ("sub/inner.md", "inner\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_docs(DocsKind::Default, &mut doc, &dir, &[]).expect("import");
        let names: Vec<&str> = doc
            .sections
            .iter()
            .map(|s| s.filename.as_deref().expect("filename"))
            .collect();
        assert_eq!(names, vec!["01-first.adoc", "02-second.md"]);
        assert_eq!(
            doc.sections.iter().map(|s| s.order).collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(
            doc.sections.iter().map(|s| s.format).collect::<Vec<_>>(),
            vec![Some(Format::AsciiDoc), Some(Format::Markdown)]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn recursive_walk_descends_and_normalizes_names() {
        let dir = scratch(
            "recursive",
            &[
                ("a.md", "top\n"),
                ("sub/b.md", "sub\n"),
                ("sub/deep/c.adoc", "deep\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_docs(DocsKind::Recursive, &mut doc, &dir, &[]).expect("import");
        let names: Vec<&str> = doc
            .sections
            .iter()
            .map(|s| s.filename.as_deref().expect("filename"))
            .collect();
        assert_eq!(names, vec!["a.md", "sub/b.md", "sub/deep/c.adoc"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn excludes_are_full_match_regexes() {
        let dir = scratch(
            "excludes",
            &[
                ("README.md", "readme\n"),
                ("01-x.md", "one\n"),
                ("02-y.md", "two\n"),
            ],
        );
        let mut doc = Documentation::default();
        import_docs(
            DocsKind::Default,
            &mut doc,
            &dir,
            &["README.md".into(), r"02[.-].*\.md".into()],
        )
        .expect("import");
        let names: Vec<&str> = doc
            .sections
            .iter()
            .map(|s| s.filename.as_deref().expect("filename"))
            .collect();
        assert_eq!(names, vec!["01-x.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_file_imports_with_empty_trimmed_name() {
        let dir = scratch("single", &[("only.md", "only content\n")]);
        let mut doc = Documentation::default();
        import_docs(DocsKind::Default, &mut doc, &dir.join("only.md"), &[]).expect("import");
        assert_eq!(doc.sections.len(), 1);
        assert_eq!(doc.sections[0].filename.as_deref(), Some(""));
        // the FormatFinder gate is SHARED with the single-file form (the
        // the official check is the file format first) — a
        // lone `.txt` imports NOTHING.
        std::fs::write(dir.join("lone.txt"), "lone").expect("write");
        import_docs(DocsKind::Default, &mut doc, &dir.join("lone.txt"), &[]).expect("import2");
        assert_eq!(doc.sections.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_path_carries_the_reference_message() {
        let dir = std::path::Path::new("/nonexistent-oxidizr-p4m2f");
        let mut doc = Documentation::default();
        let err = import_docs(DocsKind::Default, &mut doc, dir, &[]).expect_err("missing");
        let msg = err.to_string();
        assert!(msg.ends_with(" does not exist."), "{msg}");
        assert!(msg.starts_with('/'), "absolute path expected: {msg}");
    }

    #[test]
    fn content_newlines_normalize_in_one_pass() {
        let dir = scratch("crlf", &[("c.md", "a\r\nb\rc\r\nd")]);
        let mut doc = Documentation::default();
        import_docs(DocsKind::Default, &mut doc, &dir, &[]).expect("import");
        assert_eq!(doc.sections[0].content, "a\nb\nc\nd");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trim_reaches_sections_of_earlier_imports() {
        // a second `!docs` of a SUBdirectory re-trims every section,
        // exactly like the official full-list trim scan.
        let dir = scratch("retree", &[("outer.md", "o\n"), ("inner/i.md", "i\n")]);
        let mut doc = Documentation::default();
        import_docs(DocsKind::Default, &mut doc, &dir, &[]).expect("first");
        import_docs(DocsKind::Default, &mut doc, &dir.join("inner"), &[]).expect("second");
        let names: Vec<&str> = doc
            .sections
            .iter()
            .map(|s| s.filename.as_deref().expect("filename"))
            .collect();
        // `outer.md` never carried the inner base ⇒ untouched; `i.md` was
        // canonical, now trimmed once.
        assert_eq!(names, vec!["outer.md", "i.md"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reluctant_images_keep_only_referenced_names() {
        let dir = scratch(
            "images",
            &[
                ("d.md", "see ![alt](sub/ref.png) end\n"),
                ("x.md", "none\n"),
            ],
        );
        let png = base64::engine::general_purpose::STANDARD.encode(b"\x89PNG fake");
        std::fs::create_dir_all(dir.join("sub")).expect("mkdir");
        std::fs::write(dir.join("sub/ref.png"), b"\x89PNG fake").expect("write png");
        std::fs::write(dir.join("sub/unref.gif"), b"GIF89a fake").expect("write gif");
        std::fs::write(dir.join("top.svg"), b"<svg/>").expect("write svg");

        let mut doc = Documentation::default();
        import_docs(DocsKind::Default, &mut doc, &dir, &[]).expect("docs");
        assert!(doc.images.is_empty(), "the docs importer adds no images");
        import_images(&mut doc, &dir, true).expect("reluctant images");
        let names: Vec<&str> = doc.images.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["sub/ref.png"]);
        assert_eq!(doc.images[0].content, png);
        assert_eq!(
            doc.images[0].content_type.as_deref(),
            Some("image/png"),
            "content type"
        );

        // EAGER mode takes everything (incl. the top svg), sorted walk
        let mut eager = Documentation::default();
        import_images(&mut eager, &dir, false).expect("eager images");
        let names: Vec<&str> = eager.images.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["sub/ref.png", "sub/unref.gif", "top.svg"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn find_images_formats_split() {
        let md = "![a](x.png)";
        assert!(find_images(md, Some(Format::Markdown)).contains("x.png"));
        // null format takes the ASCIIDOC branch (verified)
        assert!(find_images(md, None).is_empty());
        assert!(find_images("image::y.png[a]", None).contains("y.png"));
        assert!(find_images("image::y.png[a]", Some(Format::AsciiDoc)).contains("y.png"));
    }
}
