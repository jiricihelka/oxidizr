//! File-context legs of the parse driver: the current-file accessors the
//! error context and `!include` resolution use, the `!include` recursion
//! itself and the `workspace extends` JSON-base identifier register. The
//! parse driver itself stays in the parent module.

use super::idents;
use super::stmt_line;
use super::{DslError, ParseOptions, Parser, json_dsl_identifiers};

impl Parser<'_> {
    // ------------------------------------------------------------------
    // file context (includes)
    // ------------------------------------------------------------------

    /// The file of the current recursion level, in the form official
    /// messages use it: the absolute path when the file exists —
    /// `None` (no `of …` in messages) when there is no file or it does not
    /// exist.
    pub(crate) fn current_file(&self) -> Option<String> {
        let f = self.file_stack.last()?.as_ref()?;
        let p = std::path::Path::new(f);
        p.is_file().then(|| stmt_line::java_absolute(p))
    }

    /// The raw (stored) `dslFile` of the current recursion level — paths
    /// resolve against this, not the message form.
    pub(crate) fn current_file_raw(&self) -> Option<String> {
        self.file_stack.last().and_then(|o| o.as_ref()).cloned()
    }

    /// The directory `!include`/content paths resolve against —
    /// The current file's parent joined with `source`; no file ⇒ the CWD
    /// (the process dir).
    pub(crate) fn current_dir(&self) -> Option<String> {
        match self.current_file_raw() {
            Some(f) => match std::path::Path::new(&f).parent() {
                Some(p) if !p.as_os_str().is_empty() => Some(p.to_string_lossy().into_owned()),
                _ => Some(String::new()), // relative filename input: parent == ""
            },
            None => self.opts.base_dir.map(str::to_string),
        }
    }

    /// Enter an included file: push its `dslFile`, parse, pop (the official parser
    /// recursion `parse(paddedLines, file, true, false)` — fragments are not
    /// source-tracked and their context stack must return to the entry
    /// height; under recover-and-continue the truncation-per-line already guarantees that, so
    /// no extra check).
    pub(crate) fn parse_included(&mut self, source: &str) {
        let path = self.last_include_path.clone();
        self.file_stack.push(path);
        let logicals = crate::preprocess::preprocess(source);
        self.parse_lines(&logicals, source, false);
        self.file_stack.pop();
        self.last_include_path = None;
    }

    /// `workspace extends <file|url>` — the file branch.
    /// The loaded base REPLACES
    /// the workspace under construction; later statements layer over it.
    /// The official non-file guard skips the
    /// whole branch silently — same quirk as file `!include`s.
    pub(crate) fn extend_from(&mut self, source: &str) -> Result<(), DslError> {
        // Mirrors the `!include` arm: ALL network input rejected, the
        // official feature-gate texts kept (the HTTPS and HTTP
        // rejection messages).
        if source.starts_with("https://") {
            return Err(DslError::coded(
                oxidizr_diag::codes::NET_DISABLED,
                "Extends via HTTPS are not permitted",
            ));
        }
        if source.starts_with("http://") {
            return Err(DslError::coded(
                oxidizr_diag::codes::NET_DISABLED,
                "Extends via HTTP are not permitted",
            ));
        }
        let Some(dsl_file) = self.current_file_raw() else {
            return Ok(());
        };
        let base = std::path::Path::new(&dsl_file)
            .parent()
            .map_or_else(std::path::PathBuf::new, std::path::PathBuf::from);
        let path = base.join(source);
        if !path.exists() {
            return Err(DslError::stmt(format!(
                "{} could not be found",
                stmt_line::java_canonical(&path)
            )));
        }
        if path.is_dir() {
            return Err(DslError::stmt(format!(
                "{} should be a single file",
                stmt_line::java_canonical(&path)
            )));
        }

        // the `.json` test was computed twice in this path.
        let is_json = source.to_lowercase().ends_with(".json");
        let loaded = if is_json {
            // `WorkspaceUtils.loadWorkspaceFromJson(file)` — via the
            // injected loader (topology: no `json` dep in this crate).
            let Some(loader) = self.opts.json_loader else {
                return Err(DslError::stmt(format!(
                    "extending {source} requires a JSON workspace loader, none is configured"
                )));
            };
            loader(&stmt_line::java_canonical(&path)).map_err(DslError::stmt)?
        } else {
            // The official child-parser pattern (create + parse): fresh
            // parser state over the same file entry; its
            // configure-from handover happens below, before finishing.
            let bytes = std::fs::read(&path).map_err(|_| {
                DslError::stmt(format!(
                    "Error reading file at {}: stream read failed",
                    stmt_line::java_absolute(&path)
                ))
            })?;
            let text = String::from_utf8_lossy(&bytes).into_owned();
            let child_opts = ParseOptions {
                file: Some(std::path::Path::new(&path).to_str().unwrap_or(source)),
                base_dir: self.opts.base_dir,
                json_loader: self.opts.json_loader,
                env_lookup: self.opts.env_lookup.clone(),
                installed_themes: self.opts.installed_themes.clone(),
            };
            let mut child = Parser::new(child_opts);
            child.parse(&text);
            // The official configure-from handover:
            // identifier scope, archetypes and constants/variables move into
            // THIS parser; the identifier register is context-shared, so the
            // child's registrations merge into ours.
            self.ids.set_scope(child.ids.scope());
            self.ids.adopt_entries(&mut child.ids);
            self.archetypes = std::mem::take(&mut child.archetypes);
            self.archetype_list = std::mem::take(&mut child.archetype_list);
            self.constants = std::mem::take(&mut child.constants);
            let child = child.finish();
            self.errors.extend(child.errors);
            child.workspace
        };

        self.builder.adopt(loaded);
        // JSON bases: the official identifier registration — elements AND
        // relationships carrying `structurizr.dsl.identifier` enter the
        // (context-shared) register so `!element <identifier>` finds them.
        if is_json {
            self.register_json_identifiers()?;
        }
        // The base's retained `structurizr.dsl` is
        // discarded — the EXTENDING file is the source — and the official
        // parser seeds the workspace context NOT-portable, so the
        // extending file keeps no source either (`source-child.dsl`
        // retains an empty `structurizr.dsl`). The statement
        // runs on an empty stack, so this is the parser-level flag.
        self.builder.remove_property("structurizr.dsl");
        self.dsl_portable = false;
        Ok(())
    }

    /// Identifier registration over the ADOPTED workspace:
    /// elements AND relationships carrying `structurizr.dsl.identifier`
    /// enter the (context-shared) register. Shared by the `workspace
    /// extends` JSON-base leg and the public [`crate::query`] entry, so
    /// identifiers resolve identically in both consumers.
    pub(crate) fn register_json_identifiers(&mut self) -> Result<(), DslError> {
        let (elements, relationships) = json_dsl_identifiers(self.builder.workspace());
        for (identifier, item) in elements {
            self.ids
                .register_element(
                    Some(&identifier),
                    idents::RegItem::Element(item),
                    None,
                    &identifier,
                )
                .map_err(DslError::stmt)?;
        }
        for (identifier, item) in relationships {
            self.ids
                .register_relationship(Some(&identifier), &item, &identifier)
                .map_err(DslError::stmt)?;
        }
        Ok(())
    }
}
