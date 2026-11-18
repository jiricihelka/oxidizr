//! `ThemeStore` (P2-D3, `agents/plan/build/PHASE_2.md`): the offline-first
//! theme resolution of `ThemeUtils` (structurizr-client, at the pin —
//! catalog `EXPORT-PROPERTIES.md` §6) split into its two reference legs:
//!
//! * **install** (name → `<dir>/<name>/theme.json` registry,
//!   `AbstractCommand.installThemes` + `ThemeUtils.installThemes`): source
//!   order `--themes-dir` flag → `OXIDZR_THEMES_DIR` →
//!   `<workspace-parent>/themes` (the reference default) →
//!   `STRUCTURIZR_THEMES` (drop-in fallback); the FIRST source that is a
//!   directory wins (the reference probes one directory only). The
//!   `OXIDZR_*` sources are the documented deviation (our env namespace);
//!   the reference chain is preserved.
//! * **load** (`ThemeUtils.loadThemes`, called by the pipeline ONLY for
//!   `DiagramExporter` formats, per P2-D4): URL → fetch (behind
//!   `--allow-net`, D5; system `curl` via `std::process::Command` — std has
//!   no TLS, Amendment 2026-11-17; local cache checked first — oxidizr
//!   addition) → content-type must START WITH `application/json` or
//!   `text/plain` (case-sensitive, like the reference `startsWith`) →
//!   relative icons rewritten to `baseUrl + icon` (absolute `http(s)://` and
//!   `data:image` pass through); installed name → file read, icons NOT
//!   rewritten (reference asymmetry); anything else → silently skipped.
//!   Themes append to the NON-serialized `Styles.themes` — resolution-time
//!   only, never merged into the workspace styles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use oxidizr_core::{Workspace, views::Theme};

use crate::paths;

/// Theme-load failures. The pipeline surfaces `NetDisabled` as `OXZ-5001`
/// (D5) and every other variant as `OXZ-5008` (`OXZ-5006` stays reserved
/// for input/output I/O).
#[derive(Debug, thiserror::Error)]
pub enum ThemeError {
    /// D5: a `themes { url … }` / `theme <url>` entry and `--allow-net` was
    /// not given. The message names the URL and the offline escape hatch
    /// (P2-D3 error UX).
    #[error(
        "theme `{url}` was not fetched: network access requires --allow-net; \
         place the theme at <themes-dir>/<name>/theme.json and reference it \
         by name to load it offline"
    )]
    NetDisabled {
        /// The theme URL (never fetched).
        url: String,
    },

    /// The fetch failed (or no `curl` binary exists to perform it).
    #[error("theme `{url}` could not be fetched: {reason}")]
    Fetch {
        /// The theme URL.
        url: String,
        /// Why (curl exit, transport error, unreadable cache file).
        reason: String,
    },

    /// `ThemeUtils.loadThemes` throws when the content-type is neither JSON
    /// nor plain text — message mirrored verbatim (`%s - expected content
    /// type of application/json, actual content type is %s`).
    #[error("{url} - expected content type of application/json, actual content type is {actual}")]
    ContentType {
        /// The theme URL.
        url: String,
        /// The content-type the server actually sent.
        actual: String,
    },

    /// An installed theme's `theme.json` cannot be read.
    #[error("installed theme `{name}` could not be read from {path}: {reason}")]
    Read {
        /// The theme name.
        name: String,
        /// The registered file.
        path: String,
        /// OS error text.
        reason: String,
    },

    /// A theme document failed to parse.
    #[error("theme from {origin} could not be parsed: {reason}")]
    Parse {
        /// URL or installed-theme name.
        origin: String,
        /// The JSON-layer message.
        reason: String,
    },
}

/// The install registry + load policy. Cloneable snapshot; the pipeline
/// builds ONE before the DSL parse (reference `AbstractCommand` order) and
/// reuses it for `loadThemes` and the parser's installed-name check.
#[derive(Debug, Clone, Default)]
pub struct ThemeStore {
    registry: BTreeMap<String, PathBuf>,
    cache_dir: Option<PathBuf>,
    allow_net: bool,
}

impl ThemeStore {
    /// The reference `installThemes` scan for the first source that is a
    /// directory (source order in the module docs). `env` is the `${VAR}`
    /// resolver (hermetic `ApiOptions.env` in tests/e2e, the process
    /// environment in the CLI).
    #[must_use]
    pub fn install(
        themes_dir: Option<&Path>,
        workspace_parent: Option<&Path>,
        env: &dyn Fn(&str) -> Option<String>,
        allow_net: bool,
    ) -> Self {
        // 2R R-exp-16: the source chain written out in its documented
        // order (module docs, catalog §6.2) — the old shape looped the two
        // env-var NAMES and string-compared `var == "OXIDZR_THEMES_DIR"` to
        // splice the workspace `themes/` dir in after the first leg (the
        // reference order puts that dir BEFORE its env var, our env var
        // before both).
        let env_dir = |var: &str| {
            env(var)
                .filter(|value| !value.trim().is_empty())
                .map(PathBuf::from)
        };
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(dir) = themes_dir {
            candidates.push(dir.to_path_buf());
        }
        candidates.extend(env_dir("OXIDZR_THEMES_DIR"));
        if let Some(parent) = workspace_parent {
            candidates.push(parent.join("themes"));
        }
        candidates.extend(env_dir("STRUCTURIZR_THEMES"));
        let mut store = ThemeStore {
            cache_dir: paths::themes_cache_dir(),
            allow_net,
            ..ThemeStore::default()
        };
        for candidate in candidates {
            if candidate.is_dir() {
                store.registry = scan_themes_dir(&candidate);
                break;
            }
        }
        store
    }

    /// Override the cache root (tests; `None` disables the cache).
    #[must_use]
    pub fn with_cache_dir(mut self, cache_dir: Option<PathBuf>) -> Self {
        self.cache_dir = cache_dir;
        self
    }

    /// `InstalledThemes.isInstalled` — exact name match.
    #[must_use]
    pub fn is_installed(&self, name: &str) -> bool {
        self.registry.contains_key(name)
    }

    /// `InstalledThemes.getTheme`.
    #[must_use]
    pub fn installed_path(&self, name: &str) -> Option<&Path> {
        self.registry.get(name).map(PathBuf::as_path)
    }

    #[must_use]
    pub const fn allow_net(&self) -> bool {
        self.allow_net
    }

    /// `ThemeUtils.loadThemes`: every `configuration.themes` entry resolves
    /// via URL → installed → silent skip; successes append to
    /// `Styles.themes` (which the resolution engine consults first).
    ///
    /// # Errors
    ///
    /// [`ThemeError`] on the first leg that fails (the reference throws
    /// `RuntimeException` and the CLI aborts likewise).
    pub fn load_themes(&self, workspace: &mut Workspace) -> Result<(), ThemeError> {
        // 2R R-exp-16: iterate `configuration.themes` directly — the clone
        // guarded against a borrow that never overlaps: `add_styles_from_theme`
        // appends to the SIBLING `styles.themes` field (catalog §6.1-3), so
        // the two field borrows are disjoint. Error timing unchanged: the
        // first failing location aborts with the earlier themes appended
        // (the reference throws mid-loop the same way).
        for location in &workspace.views.configuration.themes {
            if let Some(theme) = self.load_one(location)? {
                workspace
                    .views
                    .configuration
                    .styles
                    .add_styles_from_theme(theme);
            }
        }
        Ok(())
    }

    fn load_one(&self, location: &str) -> Result<Option<Theme>, ThemeError> {
        if is_http_url(location) {
            let content = self.fetch(location)?;
            let mut theme = parse_theme(location, &content)?;
            // `loadThemes` rewrites relative icons against the URL's dir —
            // ONLY on this leg (catalog §6.1-3).
            let base = base_url(location);
            for style in &mut theme.elements {
                if let Some(icon) = &style.icon {
                    if !is_absolute_icon(icon) && !icon.starts_with("data:image") {
                        style.icon = Some(format!("{base}{icon}"));
                    }
                }
            }
            Ok(Some(theme))
        } else if let Some(path) = self.registry.get(location) {
            let text = std::fs::read_to_string(path).map_err(|e| ThemeError::Read {
                name: location.to_string(),
                path: path.display().to_string(),
                reason: e.to_string(),
            })?;
            Ok(Some(parse_theme(location, &text)?))
        } else {
            // Not a URL, not installed: the reference `loadThemes` SILENTLY
            // skips the entry (catalog §6.1-3) — no warning, no error.
            Ok(None)
        }
    }

    /// The URL leg: cache hit → file; else (with `--allow-net`) a bounded
    /// `curl` fetch with the reference's content-type gate, then cache
    /// store. Cache hits are trusted (no re-fetch, no content-type check —
    /// an oxidizr addition per P2-D3, offline-first).
    fn fetch(&self, url: &str) -> Result<String, ThemeError> {
        if !self.allow_net {
            return Err(ThemeError::NetDisabled {
                url: url.to_string(),
            });
        }
        let hash = paths::sha256_hex(url.as_bytes());
        let cache_path = self
            .cache_dir
            .as_ref()
            .map(|dir| dir.join(&hash).join("theme.json"));
        if let Some(path) = &cache_path {
            if path.is_file() {
                return std::fs::read_to_string(path).map_err(|e| ThemeError::Fetch {
                    url: url.to_string(),
                    reason: format!("cached theme {} is unreadable: {e}", path.display()),
                });
            }
        }

        let work_dir = cache_path
            .as_ref()
            .and_then(|p| p.parent().map(PathBuf::from))
            .or_else(|| Some(std::env::temp_dir()))
            .filter(|d| std::fs::create_dir_all(d).is_ok());
        let tmp = work_dir.as_ref().map_or_else(
            || std::env::temp_dir().join(format!("oxidizr-theme-{hash}.part")),
            |d| d.join(format!("oxidizr-theme-{hash}.part")),
        );

        let out = std::process::Command::new("curl")
            .arg("--silent")
            .arg("--show-error")
            .arg("--location")
            .arg("--fail")
            // The reference HttpClient has a fixed socket timeout; a bounded
            // fetch keeps CI jobs from hanging.
            .arg("--max-time")
            .arg("30")
            .arg("--output")
            .arg(&tmp)
            .arg("--write-out")
            .arg("%{content_type}")
            .arg(url)
            .output();
        let out = match out {
            Ok(out) => out,
            Err(e) => {
                let reason = if e.kind() == std::io::ErrorKind::NotFound {
                    "system `curl` is not available to perform the fetch; fetch the theme \
                     manually into the cache or use --themes-dir"
                        .to_string()
                } else {
                    e.to_string()
                };
                return Err(ThemeError::Fetch {
                    url: url.to_string(),
                    reason,
                });
            }
        };
        if !out.status.success() {
            let _ = std::fs::remove_file(&tmp);
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            let reason = if stderr.is_empty() {
                format!("curl exited with {}", out.status)
            } else {
                stderr
            };
            return Err(ThemeError::Fetch {
                url: url.to_string(),
                reason,
            });
        }
        // Case-sensitive startsWith, like the reference gate.
        let actual = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !actual.starts_with("application/json") && !actual.starts_with("text/plain") {
            let _ = std::fs::remove_file(&tmp);
            return Err(ThemeError::ContentType {
                url: url.to_string(),
                actual,
            });
        }
        let content = std::fs::read_to_string(&tmp).map_err(|e| ThemeError::Fetch {
            url: url.to_string(),
            reason: format!("fetched theme is unreadable: {e}"),
        })?;
        match &cache_path {
            Some(path) => {
                // Best effort: a cache-store failure never loses the fetch.
                if std::fs::rename(&tmp, path).is_err() {
                    let _ = std::fs::remove_file(&tmp);
                }
            }
            None => {
                let _ = std::fs::remove_file(&tmp);
            }
        }
        Ok(content)
    }
}

/// `ThemeUtils.installThemes`: one level down, every subdirectory holding a
/// `theme.json` registers under its own name.
fn scan_themes_dir(dir: &Path) -> BTreeMap<String, PathBuf> {
    let mut registry = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return registry;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let theme_json = path.join("theme.json");
        if theme_json.is_file() {
            registry.insert(name.to_string(), theme_json);
        }
    }
    registry
}

fn parse_theme(origin: &str, text: &str) -> Result<Theme, ThemeError> {
    oxidizr_json::theme_from_slice(text.as_bytes()).map_err(|e| ThemeError::Parse {
        origin: origin.to_string(),
        reason: e.to_string(),
    })
}

/// The load gate's URL test: `Url.isUrl` + the http(s) prefixes — absolute
/// `http(s)://` (case-insensitive, like `isHttpUrl`/`isHttpsUrl`). Other
/// protocol strings are what the reference would try to fetch and fail on;
/// oxidizr treats them as names/skips, never fetches (deviation note).
fn is_http_url(candidate: &str) -> bool {
    let lower = candidate.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// `Url.isHttpUrl(icon) || Url.isHttpsUrl(icon)` for the icon rewrite.
fn is_absolute_icon(icon: &str) -> bool {
    is_http_url(icon)
}

/// `themeLocation.substring(0, themeLocation.lastIndexOf('/') + 1)`.
fn base_url(url: &str) -> &str {
    match url.rfind('/') {
        Some(at) => &url[..=at],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidizr_core::views::ElementStyle;

    fn theme_json(tag: &str, background: &str, icon: Option<&str>) -> String {
        let icon = icon.map_or_else(String::new, |i| format!(",\"icon\":\"{i}\""));
        let element = format!("{{\"tag\":\"{tag}\",\"background\":\"{background}\"{icon}}}");
        format!("{{\"name\":\"tst\",\"elements\":[{element}]}}")
    }

    fn temp_dir(topic: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oxidizr-m1c-{topic}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn env_of(pairs: Vec<(&str, &str)>) -> impl Fn(&str) -> Option<String> + 'static {
        let map: BTreeMap<String, String> = pairs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    fn workspace_with_theme(location: &str) -> Workspace {
        let mut workspace = Workspace::default();
        workspace
            .views
            .configuration
            .themes
            .push(location.to_string());
        workspace
    }

    #[test]
    fn install_scans_one_level_theme_json_dirs() {
        let root = temp_dir("scan");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(
            root.join("mine/theme.json"),
            theme_json("Person", "#123456", None),
        )
        .unwrap();
        std::fs::create_dir_all(root.join("empty")).unwrap();
        std::fs::write(root.join("loose.json"), "{}").unwrap();

        let store = ThemeStore::install(Some(&root), None, &env_of(vec![]), false);
        assert!(store.is_installed("mine"));
        assert!(!store.is_installed("empty"));
        assert!(!store.is_installed("loose.json"));
        assert_eq!(
            store.installed_path("mine").unwrap().file_name().unwrap(),
            "theme.json"
        );
    }

    #[test]
    fn source_order_flag_env_ws_parent_dropin() {
        let flag = temp_dir("order-flag");
        let ours = temp_dir("order-ours");
        let ws = temp_dir("order-ws");
        let dropin = temp_dir("order-dropin");
        for (dir, name) in [
            (&flag, "flag"),
            (&ours, "ours"),
            (&ws.join("themes"), "ws"),
            (&dropin, "dropin"),
        ] {
            std::fs::create_dir_all(dir.join(name)).unwrap();
            std::fs::write(
                dir.join(name).join("theme.json"),
                theme_json("T", "#000000", None),
            )
            .unwrap();
        }
        let env = env_of(vec![
            ("OXIDZR_THEMES_DIR", ours.to_str().unwrap()),
            ("STRUCTURIZR_THEMES", dropin.to_str().unwrap()),
        ]);

        let store = ThemeStore::install(Some(&flag), Some(&ws), &env, false);
        assert!(store.is_installed("flag") && !store.is_installed("ours"));

        let store = ThemeStore::install(None, Some(&ws), &env, false);
        assert!(store.is_installed("ours") && !store.is_installed("ws"));

        let store = ThemeStore::install(None, Some(&ws), &env_of(vec![]), false);
        assert!(store.is_installed("ws") && !store.is_installed("dropin"));

        let store = ThemeStore::install(
            None,
            None,
            &env_of(vec![("STRUCTURIZR_THEMES", dropin.to_str().unwrap())]),
            false,
        );
        assert!(store.is_installed("dropin"));
    }

    #[test]
    fn load_appends_installed_theme_and_skips_unknown() {
        let root = temp_dir("load");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(
            root.join("mine/theme.json"),
            theme_json("Person", "#abcdef", None),
        )
        .unwrap();
        let store = ThemeStore::install(Some(&root), None, &env_of(vec![]), false);

        let mut workspace = workspace_with_theme("mine");
        workspace
            .views
            .configuration
            .styles
            .element_styles
            .push(ElementStyle {
                tag: "Person".into(),
                background: Some("#000000".into()),
                ..ElementStyle::default()
            });
        store.load_themes(&mut workspace).unwrap();
        // Appended to Styles.themes, NOT merged into the workspace styles.
        assert_eq!(workspace.views.configuration.styles.themes.len(), 1);
        assert_eq!(workspace.views.configuration.styles.element_styles.len(), 1);
        // Resolution: workspace style wins, theme participates (P2-D2).
        let style = workspace
            .views
            .configuration
            .styles
            .find_element_style("Person", oxidizr_core::views::ColorScheme::Light)
            .unwrap();
        assert_eq!(style.background.as_deref(), Some("#000000"));

        // Unknown names silently skip (reference parity).
        let mut workspace = workspace_with_theme("ghost");
        store.load_themes(&mut workspace).unwrap();
        assert_eq!(workspace.views.configuration.styles.themes, Vec::new());
    }

    #[test]
    fn url_leg_needs_allow_net_but_cache_hits_work_offline() {
        let cache = temp_dir("cache");
        let url = "https://example.invalid/themes/mine/theme.json";
        let hash = paths::sha256_hex(url.as_bytes());
        std::fs::create_dir_all(cache.join(&hash)).unwrap();
        std::fs::write(
            cache.join(&hash).join("theme.json"),
            theme_json("Person", "#111111", Some("rel/icon.png")),
        )
        .unwrap();

        let store = ThemeStore::install(None, None, &env_of(vec![]), false)
            .with_cache_dir(Some(cache.clone()));
        let mut workspace = workspace_with_theme(url);
        // No --allow-net: OXZ-5001-shaped error naming the URL.
        let err = store.load_themes(&mut workspace).unwrap_err();
        assert!(matches!(&err, ThemeError::NetDisabled { url: u } if u == url));
        assert!(err.to_string().contains("--allow-net"));

        // With --allow-net the cache hit answers WITHOUT touching the
        // network; relative icons rewrite against the URL base.
        let store = store.with_cache_dir(Some(cache)).clone_allow_net();
        let mut workspace = workspace_with_theme(url);
        store.load_themes(&mut workspace).unwrap();
        let theme = &workspace.views.configuration.styles.themes[0];
        assert_eq!(
            theme.elements[0].icon.as_deref(),
            Some("https://example.invalid/themes/mine/rel/icon.png")
        );
    }

    #[test]
    fn installed_leg_does_not_rewrite_icons() {
        let root = temp_dir("icons");
        std::fs::create_dir_all(root.join("mine")).unwrap();
        std::fs::write(
            root.join("mine/theme.json"),
            theme_json("Person", "#111111", Some("rel/icon.png")),
        )
        .unwrap();
        let store = ThemeStore::install(Some(&root), None, &env_of(vec![]), false);
        let mut workspace = workspace_with_theme("mine");
        store.load_themes(&mut workspace).unwrap();
        assert_eq!(
            workspace.views.configuration.styles.themes[0].elements[0]
                .icon
                .as_deref(),
            Some("rel/icon.png")
        );
    }

    #[test]
    fn base_url_and_parse_errors() {
        assert_eq!(base_url("https://x/y/theme.json"), "https://x/y/");
        assert_eq!(base_url("no-slash"), "");
        let err = parse_theme("tst", "{not json").unwrap_err();
        assert!(matches!(err, ThemeError::Parse { .. }));
    }

    impl ThemeStore {
        /// allow-net ON (test helper on top of `with_cache_dir`).
        fn clone_allow_net(&self) -> ThemeStore {
            ThemeStore {
                allow_net: true,
                ..self.clone()
            }
        }
    }
}
