//! The `image` importer — file variant of the official image importer
//! plus the official image-utils helpers. The semantics: the `ImageView` gets
//! `content = data:<type>;base64,<payload>`, `contentType`, and
//! `title = file name` (`ImageView.name` IS the title in the JSON, see
//! the writer's `View::Image` rule).
//!
//! Pinned behaviors:
//! - the supported-type table follows the official extension constants
//!   (`.png`/`.jpg`/`.jpeg` → jpeg, `.gif`, `.svg`; anything else is not
//!   a supported image file — the official name-guesser never
//!   resolves a non-`image/*` type through this table);
//! - the data URI embeds the RAW file bytes for EVERY type. Deviation
//!   (documented deviation): the official tooling runs non-SVG images
//!   through a read+WRITE image re-encode, whose bytes depend
//!   on the runtime version — irreproducible by design; the raw payload is
//!   deterministic, identical in shape, and exactly what the official tooling
//!   does for SVG (SVG bytes pass through untouched).
//! - the URL variant of the official tooling importer is a network fetch
//!   (an HTTP fetch) — network work, deferred like the kroki leg;
//!   only the local-file variant is implemented.

use std::path::Path;

use base64::Engine as _;
use oxidizr_core::WorkspaceBuilder;

use crate::{ImportError, Imported};

/// Options for [`import_image`] (the CLI's `--view-key`/`--view-name`).
#[derive(Debug, Default, Clone)]
pub struct ImageImport<'a> {
    /// Explicit view key; `None`/empty ⇒ the official tooling
    /// `generateViewKey("Image")` = `Image-001` (+ `generatedKey`).
    pub view_key: Option<&'a str>,
    /// View title override (the JSON `name` of an `ImageView` is the
    /// title verbatim); `None` ⇒ the file name, exactly like the
    /// the official tooling `view.setTitle(file.getName())`.
    pub view_name: Option<&'a str>,
}

/// The official tooling content-type table (`ImageUtils` extension constants).
/// `name` is the FILE NAME; the extension match is case-insensitive
/// (name guessing is suffix-based, `.PNG` resolves).
#[must_use]
pub(crate) fn content_type_of(name: &str) -> Option<&'static str> {
    let dot = name.rfind('.')?;
    match name[dot..].to_ascii_lowercase().as_str() {
        ".png" => Some("image/png"),
        ".jpg" | ".jpeg" => Some("image/jpeg"),
        ".gif" => Some("image/gif"),
        ".svg" => Some("image/svg+xml"),
        _ => None,
    }
}

/// Import one local image file: build a fresh workspace whose single
/// `ImageView` carries the data URI, content type and title (module
/// docs for the verified semantics).
///
/// # Errors
///
/// [`ImportError::Missing`]/[`ImportError::NotFile`]/
/// [`ImportError::Unsupported`] mirror the official tooling’s
/// `invalid-argument crash` checks in its order;
/// [`ImportError::Io`] for read failures.
pub fn import_image(path: &Path, opts: &ImageImport<'_>) -> Result<Imported, ImportError> {
    // The official tooling checks exist() then isFile() before guessing the
    // type — keep the order (a directory named `x.png` errors, not
    // imports).
    if !path.exists() {
        return Err(ImportError::Missing(path.to_path_buf()));
    }
    if !path.is_file() {
        return Err(ImportError::NotFile(path.to_path_buf()));
    }
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let content_type =
        content_type_of(&name).ok_or_else(|| ImportError::Unsupported(path.to_path_buf()))?;
    let bytes = std::fs::read(path).map_err(|source| ImportError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    // `ImageUtils.getImageAsDataUri`: standard-alphabet base64 (no line
    // breaks), `data:<ct>;base64,` prefix.
    let payload = base64::engine::general_purpose::STANDARD.encode(&bytes);
    let content = format!("data:{content_type};base64,{payload}");

    // The view is created through the official tooling `createImageView` rules
    // (generated key when empty, order = max + 1, uniqueness).
    let mut builder = WorkspaceBuilder::new();
    let view = builder.create_image_view(
        None,
        opts.view_key.unwrap_or(""),
        Some(content),
        Some(content_type.to_owned()),
    )?;
    // `importDiagram(ImageView, File)`: title = file name (over any
    // creation-time name); the CLI's --view-name overrides the override.
    if let Some(base) = builder.view_base_mut(view) {
        base.title = Some(opts.view_name.map_or_else(|| name.clone(), str::to_owned));
    }
    Ok(Imported {
        workspace: builder.build(),
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use expect_test::expect;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("oxidizr-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let path = temp_dir().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    /// A real 1x1 PNG (the raw-bytes oxidizr embeds it BYTE-IDENTICAL).
    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    fn png_bytes() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(PNG_1X1)
            .unwrap()
    }

    #[test]
    fn content_type_table_matches_the_reference_constants() {
        assert_eq!(content_type_of("a.png"), Some("image/png"));
        assert_eq!(content_type_of("a.PNG"), Some("image/png"));
        assert_eq!(content_type_of("a.jpg"), Some("image/jpeg"));
        assert_eq!(content_type_of("a.JPEG"), Some("image/jpeg"));
        assert_eq!(content_type_of("a.gif"), Some("image/gif"));
        assert_eq!(content_type_of("a.svg"), Some("image/svg+xml"));
        assert_eq!(content_type_of("a.txt"), None);
        assert_eq!(content_type_of("noextension"), None);
        // Suffix-based like the official guesser: the LAST dot decides.
        assert_eq!(content_type_of("archive.tar.png"), Some("image/png"));
    }

    #[test]
    fn png_imports_as_data_uri_image_view() {
        let path = fixture("dot.png", &png_bytes());
        let imported = import_image(&path, &ImageImport::default()).unwrap();
        assert_eq!(imported.warnings, [] as [String; 0]);
        let views = &imported.workspace.views;
        assert_eq!(views.image_views.len(), 1);
        let view = &views.image_views[0];
        // Fresh workspace ⇒ the official tooling generateViewKey + order 1.
        assert_eq!(view.base.key, "Image-001");
        assert!(view.base.generated_key);
        assert_eq!(view.base.order, 1);
        // The official tooling title = file name (the JSON name of an image
        // view IS the title).
        assert_eq!(view.base.title.as_deref(), Some("dot.png"));
        assert_eq!(view.content_type.as_deref(), Some("image/png"));
        // Raw bytes: the payload IS the file, base64'd.
        expect!["data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="]
            .assert_eq(view.content.as_deref().unwrap());
    }

    #[test]
    fn svg_imports_raw_and_flags_override() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
        let path = fixture("diagram.svg", svg);
        let imported = import_image(
            &path,
            &ImageImport {
                view_key: Some("cover"),
                view_name: Some("Cover page"),
            },
        )
        .unwrap();
        let view = &imported.workspace.views.image_views[0];
        assert_eq!(view.base.key, "cover");
        assert!(!view.base.generated_key);
        assert_eq!(view.base.title.as_deref(), Some("Cover page"));
        assert_eq!(view.content_type.as_deref(), Some("image/svg+xml"));
        let payload = view
            .content
            .as_deref()
            .unwrap()
            .strip_prefix("data:image/svg+xml;base64,")
            .unwrap();
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(payload)
                .unwrap(),
            svg
        );
    }

    #[test]
    fn error_paths_mirror_the_reference_checks_in_order() {
        let missing = temp_dir().join("absent.png");
        assert!(matches!(
            import_image(&missing, &ImageImport::default()),
            Err(ImportError::Missing(_))
        ));
        let dir = temp_dir();
        assert!(matches!(
            import_image(&dir, &ImageImport::default()),
            Err(ImportError::NotFile(_))
        ));
        // A DIRECTORY named x.png fails the isFile() check first.
        let fake = dir.join("fake.png");
        std::fs::create_dir_all(&fake).unwrap();
        assert!(matches!(
            import_image(&fake, &ImageImport::default()),
            Err(ImportError::NotFile(_))
        ));
        let txt = fixture("notes.txt", b"hi");
        assert!(matches!(
            import_image(&txt, &ImageImport::default()),
            Err(ImportError::Unsupported(_))
        ));
    }
}
