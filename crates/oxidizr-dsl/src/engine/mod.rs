//! The faithful single-pass DSL parse engine (P1-D3, "engine of record").
//!
//! This module ports `StructurizrDslParser.java` line-for-line: the same
//! logical-line pipeline (preprocess → BOM strip → empty/comment gates →
//! tokenize → `${}` substitution → `id = ` prefix), the same ordered
//! `if-else` statement dispatch chain over the same context stack (context
//! guards are TOP-OF-STACK `instanceof` checks — `inContext` is
//! `clazz.isAssignableFrom(stack.peek().getClass())`, NOT a stack search).
//! The chain is split into the section modules [`stmt_line`] (arms at Java
//! lines 281–347), [`stmt_model`] (350–611), [`stmt_props`] (612–741),
//! [`stmt_style`] (742–948), [`stmt_deployment`] (949–1028), [`stmt_views`]
//! (1029–1196) and [`stmt_config`] (1197–1339) — each section runs in that
//! order, so the chain order in the reference (which IS the compatibility
//! contract) is preserved exactly; every arm is annotated with its Java line
//! number.
//!
//! The ONE deliberate difference (D1): the reference throws at the first
//! failure and aborts; here each logical line is processed with the reference
//! semantics and, on failure, the diagnostic is recorded, the context stack
//! is popped back to the height recorded before the line, and parsing
//! continues. Reference message texts are reproduced byte-identically
//! ([`DslError`]); locations are structured, not embedded.

mod ctx;
mod error;
mod expr;
mod find;
mod idents;
mod rel;
mod stmt_config;
mod stmt_deployment;
mod stmt_line;
mod stmt_model;
mod stmt_props;
mod stmt_style;
mod stmt_views;
mod tok;
mod tokenize;
mod tokens;
mod uuid;

use std::collections::HashMap;

use oxidizr_core::validate::ElementKind;
use oxidizr_core::{CoreError, Id, ViewRef, Workspace, WorkspaceBuilder};

use crate::compat;
use crate::preprocess::{Logical, text_block_value};

pub use ctx::{ColorScheme, Ctx, DslEnvironment, DslGroup, Holder};
pub use error::DslError;
pub use idents::{IdentifierScope, IdentifiersRegister, PseudoKind, RegItem};
use rel::RelArch;
pub use tokens::Tokens;
pub use uuid::{looks_like_uuid, uuid_v4};

/// How a chain section finished with a logical line.
pub(crate) enum Sec {
    /// An arm in this section matched and the statement was processed.
    Handled,
    /// An arm matched and FAILED — the line carries a diagnostic.
    Err(DslError),
    /// No arm in this section matched; fall through to the next section
    /// (== the next `else if` in the reference).
    No,
}

/// One chain section (`else if` group over a contiguous Java line range).
/// Shared-borrow `&Stmt` per 2R R-dsl-01: no section mutates the statement
/// (they mutate the parser), and the Java chain shares ONE tokens reference
/// across every `else if`.
type Section = fn(&mut Parser, &Stmt) -> Sec;

impl From<Result<(), DslError>> for Sec {
    /// Arm-body shorthand: `ok → Handled`, `err → Err`.
    fn from(r: Result<(), DslError>) -> Self {
        match r {
            Ok(()) => Sec::Handled,
            Err(e) => Sec::Err(e),
        }
    }
}

/// The ordered dispatch chain. Order IS the compatibility contract — the
/// array index mirrors the reference's `else if` sequence.
const SECTIONS: &[Section] = &[
    stmt_line::section,
    stmt_model::section,
    stmt_props::section,
    stmt_style::section,
    stmt_deployment::section,
    stmt_views::section,
    stmt_config::section,
];

/// A dispatched logical line (state the chain arms read; mutations land on
/// the parser, never here — 2R R-dsl-01).
pub(crate) struct Stmt {
    /// The joined logical line, BOM-stripped — what the tokenizer saw.
    pub raw: String,
    /// The logical line number (`DslLine.getLineNumber()`).
    #[allow(dead_code)] // diagnostics re-anchor on it from M5 onward
    pub number: u32,
    /// Tokens after `${}` substitution and `id =` prefix removal.
    pub tokens: Tokens<'static>,
    /// The `id = ` identifier, if the statement carried one (Java
    /// `identifier`).
    pub identifier: Option<String>,
}

impl Stmt {
    /// Java `String firstToken = tokens.get(0)` (post id-strip).
    pub fn first(&self) -> std::borrow::Cow<'_, str> {
        self.tokens.get(0)
    }
}

/// Java `String.equalsIgnoreCase` (compat-routed so the call sites stay
/// honest about the UTF-16 subtleties documented there).
#[must_use]
pub(crate) fn eqi(a: &str, b: &str) -> bool {
    compat::java_eq_ignore_case(a, b)
}

/// Reference `Archetype` (`final class Archetype`), stored parser-side;
/// `archetype_list` indices are the `ArchetypeDslContext` payloads.
#[derive(Debug, Clone)]
pub(crate) struct Archetype {
    /// Lowercased name (`Archetype` ctor).
    pub name: String,
    /// The archetype type token (`person`, `->`, …).
    pub kind: String,
    pub metadata: String,
    pub description: String,
    pub technology: String,
    /// `LinkedHashSet` semantics: insertion-ordered, deduped.
    pub tags: Vec<String>,
    /// `LinkedHashMap` insertion order matters for JSON output.
    pub properties: Vec<(String, String)>,
    pub perspectives: Vec<oxidizr_core::model::Perspective>,
}

impl Archetype {
    /// `new Archetype(name, type)` — an empty name falls back to the type;
    /// the name is lowercased.
    fn new(name: Option<&str>, kind: &str) -> Self {
        // `StringUtils.isNullOrEmpty` TRIMS, but the retained name is NOT
        // trimmed — only lowercased (`this.name = name.toLowerCase()`).
        let name = match name {
            Some(n) if !n.trim().is_empty() => n,
            _ => kind,
        }
        .to_lowercase();
        Archetype {
            name,
            kind: kind.to_string(),
            metadata: String::new(),
            description: String::new(),
            technology: String::new(),
            tags: Vec::new(),
            properties: Vec::new(),
            perspectives: Vec::new(),
        }
    }
}

/// `NameValuePair` / `NameValueType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NameValueType {
    Constant,
    Variable,
    TextBlock,
}

/// A `${name}` substitution entry.
#[derive(Debug, Clone)]
pub(crate) struct NameValue {
    // `name` is the entry's literal key — kept for `!const`/`!var`
    // diagnostics when the M5 directive engine lands.
    #[allow(dead_code)]
    pub name: String,
    pub value: String,
    pub vtype: NameValueType,
}

/// One frame of the reference `SequenceNumber`/`SequenceCounter` stack,
/// mirrored parser-side per dynamic view (`startParallelSequence` /
/// `endParallelSequence` / `getNextSequenceNumber` are DSL-time state; the
/// core receives the resulting `order` string).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SeqFrame {
    sequence: u64,
    incremented: bool,
}

/// Base-workspace loader for `workspace extends <file.json>` (M5): receives
/// the canonical path string, returns the loaded workspace or the
/// reference-shaped failure message.
pub type JsonWorkspaceLoader = fn(&str) -> Result<Workspace, String>;

/// `${VAR}` environment lookup override (Phase 1T, P1T-D1): lets the
/// headless API run parses hermetically (test-provided map instead of the
/// process environment). `None` — the production default — keeps the V-3
/// reference behavior (`System.getenv` equivalent, `std::env::var`).
///
/// `Arc<dyn Fn>` (not a fn pointer) so a captured map rides along;
/// `Send + Sync` keeps [`ParseOptions`] usable from parallel in-process
/// runs (the e2e fan-out).
/// The boxed lookup behind [`EnvLookup`] (named to tame `type_complexity`).
type EnvFn = dyn Fn(&str) -> Option<String> + Send + Sync;

#[derive(Clone)]
pub struct EnvLookup(std::sync::Arc<EnvFn>);

impl EnvLookup {
    /// Wrap any total, thread-safe lookup.
    #[must_use]
    pub fn new(lookup: impl Fn(&str) -> Option<String> + Send + Sync + 'static) -> Self {
        EnvLookup(std::sync::Arc::new(lookup))
    }

    /// Resolve `name` (the substitute-sites' sole entry point).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<String> {
        (self.0)(name)
    }
}

impl std::fmt::Debug for EnvLookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnvLookup(..)")
    }
}

/// Installed-theme name check for `ThemeParser.addTheme`'s installed branch
/// (`InstalledThemes.isInstalled`, P2-D3/M1-d). The pipeline builds the
/// registry BEFORE parsing (reference `AbstractCommand.installThemes`) and
/// injects it here — this crate never touches the filesystem for themes
/// (topology P1-D1). `None` = an empty registry (fragment parses, library
/// defaults): every non-URL theme token takes the FILE branch.
/// The boxed check behind [`InstalledThemes`] (named to tame
/// `type_complexity`).
type ThemeInstalledFn = dyn Fn(&str) -> bool + Send + Sync;

#[derive(Clone)]
pub struct InstalledThemes(std::sync::Arc<ThemeInstalledFn>);

impl InstalledThemes {
    /// Wrap any total, thread-safe registry check.
    #[must_use]
    pub fn new(check: impl Fn(&str) -> bool + Send + Sync + 'static) -> Self {
        InstalledThemes(std::sync::Arc::new(check))
    }

    /// `InstalledThemes.isInstalled` — the sole parse-time entry point.
    #[must_use]
    pub fn is_installed(&self, name: &str) -> bool {
        (self.0)(name)
    }
}

impl std::fmt::Debug for InstalledThemes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InstalledThemes(..)")
    }
}

/// `(identifier, item id)` pairs from a JSON base
/// ([`json_dsl_identifiers`]).
type DslIdentifiers = Vec<(String, Id)>;

/// Entry options (the reference `parse(String, File)` entry: main file,
/// source tracked, not a fragment).
#[derive(Debug, Clone)]
pub struct ParseOptions<'a> {
    /// Path reported in reference-style messages (`of <path>`); `None` for
    /// stdin/fragment input (the reference prints no `of …` for non-files).
    pub file: Option<&'a str>,
    /// Directory relative paths (includes) resolve against; `file`'s parent
    /// by default. `None` = the process CWD (`new File(".").getParent()`).
    pub base_dir: Option<&'a str>,
    /// `.json` base loader for `workspace extends <file.json>` (M5,
    /// `PHASE_1.md` §M5-b): receives the canonical path string, returns the
    /// loaded workspace or the reference-shaped failure message. Topology
    /// keeps `oxidizr-json` out of this crate (P1-D1) — the CLI injects it;
    /// `None` (default, e.g. fragment parsing) rejects `.json` bases with a
    /// clean error.
    pub json_loader: Option<JsonWorkspaceLoader>,
    /// `${VAR}` fallback lookup ([`EnvLookup`]); `None` (production, CLI)
    /// reads the process environment exactly as the reference parser does
    /// (V-3). Set by the headless API for hermetic in-process runs
    /// (`agents/plan/build/PHASE_1T.md` P1T-D1).
    pub env_lookup: Option<EnvLookup>,
    /// Installed-theme registry check ([`InstalledThemes`], M1-d): gates
    /// `ThemeParser.addTheme`'s installed-name branch. `None` = empty
    /// registry (the reference's `InstalledThemes` when no themes dir was
    /// installed) — non-URL theme tokens then take the FILE branch.
    pub installed_themes: Option<InstalledThemes>,
}

impl<'a> ParseOptions<'a> {
    /// File entry options with includes relative to the file.
    #[must_use]
    pub fn file(path: &'a str) -> Self {
        ParseOptions {
            file: Some(path),
            base_dir: None,
            json_loader: None,
            env_lookup: None,
            installed_themes: None,
        }
    }

    /// File entry options with an explicit base directory for relative
    /// `dslFile`s.
    #[must_use]
    pub fn file_with_base(path: &'a str, base_dir: &'a str) -> Self {
        ParseOptions {
            file: Some(path),
            base_dir: Some(base_dir),
            json_loader: None,
            env_lookup: None,
            installed_themes: None,
        }
    }

    /// Fragment options (no file, includes unresolvable ⇒ reference parity
    /// for `!include` errors).
    #[must_use]
    pub fn fragment() -> Self {
        ParseOptions {
            file: None,
            base_dir: None,
            json_loader: None,
            env_lookup: None,
            installed_themes: None,
        }
    }
}

/// The result of a complete parse.
pub struct ParseResult {
    /// The built workspace.
    pub workspace: Workspace,
    /// Diagnostics recorded via D1 recovery (empty for reference-accepted
    /// inputs whose only errors would be reference aborts).
    pub errors: Vec<DslError>,
}

/// The parser (reference `StructurizrDslParser` instance state).
pub(crate) struct Parser<'a> {
    pub builder: WorkspaceBuilder,
    pub ids: IdentifiersRegister,
    /// The context stack (`contextStack`). Java stacks hold ONE object per
    /// frame; the portable flag lives ON the context, so it rides a parallel
    /// stack kept in lock-step by [`Parser::start_context`] /
    /// [`Parser::end_context`].
    pub stack: Vec<Ctx>,
    frame_portable: Vec<bool>,
    pub opts: ParseOptions<'a>,

    // --- reference parser fields -----------------------------------------
    /// `workspace != null` ⇔ the `workspace` token was seen (gates the
    /// catch-all's permitted-token list and the `structurizr.dsl` property
    /// at finish).
    pub have_workspace: bool,
    /// `parsedTokens` — the duplicate-scope-keyword guard set (`workspace`,
    /// `model`, `views`).
    pub parsed_tokens: Vec<String>,
    /// `extendingWorkspace` — set only by the `workspace extends` arm once
    /// the workspace statement completes (`StructurizrDslParser` line 719);
    /// the four element parsers read it (their contexts capture it at push
    /// time — the flag never changes after, so the parser-level read is
    /// equivalent) and switch to get-or-add by name on the base workspace.
    pub extending_workspace: bool,
    /// `dslPortable` (parser level — AND over CLOSED contexts; cleared per
    /// context by file-system `!include`).
    pub dsl_portable: bool,
    /// `dslSourceLines`: the MAIN file's raw physical lines (BOM included —
    /// the reference accumulates before stripping; includes excluded).
    pub dsl_source_lines: Vec<String>,
    /// `constantsAndVariables`.
    pub constants: HashMap<String, NameValue>,
    /// `archetypes`: type token → lowercased name → index into
    /// `archetype_list`.
    pub archetypes: HashMap<String, HashMap<String, usize>>,
    /// All archetypes ever created (contexts and throwaway lookups index
    /// into this).
    pub archetype_list: Vec<Archetype>,
    /// `dslFile` per recursion level (includes push theirs; `None` =
    /// fragment/no file).
    pub file_stack: Vec<Option<String>>,
    /// Pseudo-object uid source (Java object identity surrogate).
    pub next_uid: u64,
    /// D1 diagnostics.
    pub errors: Vec<DslError>,
    /// Per dynamic view: the `SequenceNumber` counter stack (empty vec =
    /// the implicit root counter starting at 0).
    pub seq: HashMap<usize, Vec<SeqFrame>>,
    /// Transient set by the `!include` arm: the `IncludedFile.getFile()`
    /// the recursion reports as its `dslFile`.
    pub(crate) last_include_path: Option<String>,
    /// The `ElementGroup` member sets, indexed by group `uid`: the live
    /// membership the ONE registered group object accumulates via
    /// `addElement` (which propagates to the parent group object).
    pub groups: Vec<GroupRecord>,
}

/// One `ElementGroup` object's state (`ElementGroup.elements` plus the
/// `parentGroup` link needed to propagate `addElement`).
#[derive(Debug, Clone, Default)]
pub(crate) struct GroupRecord {
    /// Members, first-insertion order (the portable stand-in for the
    /// reference `HashSet` iteration — see `agents/plan/PROGRESS.md`).
    pub members: Vec<Id>,
    /// The enclosing group's `uid` (nested `group` blocks).
    pub parent_uid: Option<u64>,
}

impl<'a> Parser<'a> {
    fn new(opts: ParseOptions<'a>) -> Self {
        let mut archetypes = HashMap::new();
        for kind in [
            tok::GROUP,
            tok::CUSTOM_ELEMENT,
            tok::PERSON,
            tok::SOFTWARE_SYSTEM,
            tok::CONTAINER,
            tok::COMPONENT,
            tok::DEPLOYMENT_NODE,
            tok::INFRASTRUCTURE_NODE,
            tok::RELATIONSHIP,
        ] {
            archetypes.insert(kind.to_string(), HashMap::new());
        }
        // `opts` is moved into the struct below, so read `file` first.
        let file_stack = vec![opts.file.map(str::to_string)];
        Parser {
            builder: WorkspaceBuilder::new(),
            ids: IdentifiersRegister::new(),
            stack: Vec::new(),
            frame_portable: Vec::new(),
            opts,
            have_workspace: false,
            parsed_tokens: Vec::new(),
            extending_workspace: false,
            dsl_portable: true,
            dsl_source_lines: Vec::new(),
            constants: HashMap::new(),
            archetypes,
            archetype_list: Vec::new(),
            file_stack,
            next_uid: 1,
            errors: Vec::new(),
            seq: HashMap::new(),
            last_include_path: None,
            groups: Vec::new(),
        }
    }

    fn next_uid(&mut self) -> u64 {
        let u = self.next_uid;
        self.next_uid += 1;
        u
    }

    // ------------------------------------------------------------------
    // context stack helpers (reference `startContext`/`endContext`/…)
    // ------------------------------------------------------------------

    fn start_context(&mut self, ctx: Ctx) {
        self.stack.push(ctx);
        self.frame_portable.push(true);
    }

    /// The top of the stack (`getContext()`), Java `peek()`.
    fn top(&self) -> Option<&Ctx> {
        self.stack.last()
    }

    /// `inContext(Foo.class)` — TOP frame only (`isAssignableFrom(peek())`).
    fn in_ctx(&self, p: impl FnOnce(&Ctx) -> bool) -> bool {
        self.top().is_some_and(p)
    }

    /// `getContext(Foo.class)` against a predicate: the Java cast error text
    /// uses fully-qualified reference class names — pinned here.
    fn ctx_cast(&self, expected: &str, ok: impl FnOnce(&Ctx) -> bool) -> Result<(), DslError> {
        if self.in_ctx(ok) {
            Ok(())
        } else {
            let got = self.top().map_or("null", java_class_name);
            Err(DslError::stmt(format!(
                "Expected com.structurizr.dsl.{expected} but got com.structurizr.dsl.{got}"
            )))
        }
    }

    /// `endContext()`: pop, run `DslContext.end()`, fold portability.
    fn end_context(&mut self) -> Result<(), DslError> {
        match self.stack.pop() {
            Some(ctx) => {
                let portable = self.frame_portable.pop().unwrap_or(true);
                self.end_ctx(&ctx)?;
                // `dslPortable &= context.isDslPortable()` (reference).
                self.dsl_portable &= portable;
                Ok(())
            }
            None => Err(DslError::stmt("Unexpected end of context")),
        }
    }

    /// `DslContext.end()` side effects.
    fn end_ctx(&mut self, ctx: &Ctx) -> Result<(), DslError> {
        match ctx {
            // `PerspectiveDslContext.end()` attaches the finished
            // perspective to the enclosing PerspectivesDslContext's holders.
            Ctx::Perspective { def } => {
                let holders: Vec<Holder> = self
                    .stack
                    .iter()
                    .rev()
                    .find_map(|c| match c {
                        Ctx::Perspectives { holders } => Some(holders.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                self.add_perspective_to_holders(&holders, def.clone())?;
                Ok(())
            }
            // `DynamicViewParallelSequenceDslContext.end()`:
            // `getView().endParallelSequence(!relationships)`.
            Ctx::DynamicParallel {
                view,
                has_relationships,
            } => {
                self.seq_end_parallel(*view, !has_relationships);
                Ok(())
            }
            // D7: the !docs/!adrs importers land in Phase 4; the deferred
            // error surfaces where the reference would have LOADED content.
            Ctx::Documentation { .. } => Err(DslError::coded(
                oxidizr_diag::codes::DOCS_DEFERRED,
                "!docs documentation import arrives in Phase 4 (D7)",
            )),
            Ctx::Decisions { .. } => Err(DslError::coded(
                oxidizr_diag::codes::DOCS_DEFERRED,
                "!adrs decisions import arrives in Phase 4 (D7)",
            )),
            // `ImageViewDslContext.end()`: a view that ends without any
            // (light/dark/plain) content fails — checked on EVERY frame pop,
            // so `light { ... }` inside an empty image view fails there too.
            Ctx::ImageView { view, .. } => {
                if self.builder.image_has_content(*view) {
                    Ok(())
                } else {
                    Err(DslError::stmt(format!(
                        "The image view \"{}\" has no content",
                        self.builder.view_key(*view)
                    )))
                }
            }
            _ => Ok(()),
        }
    }

    // ------------------------------------------------------------------
    // identifier registration (reference `registerIdentifier`)
    // ------------------------------------------------------------------

    /// Register an element (or pseudo-element) under the statement's
    /// `identifier` (or a fresh uuid) and set its
    /// `structurizr.dsl.identifier` property — EXACTLY the reference: even
    /// anonymous elements get the (uuid-valued) property.
    // Justified: every caller passes a freshly built item it no longer uses;
    // taking ownership keeps the call sites move-clean (the registry stores
    // its own copy).
    #[allow(clippy::needless_pass_by_value)]
    fn register_identifier_element(
        &mut self,
        identifier: Option<&str>,
        item: RegItem,
        canonical_name: &str,
    ) -> Result<(), DslError> {
        let prefix = self.hierarchical_prefix(&item);
        let final_id = self
            .ids
            .register_element(identifier, item.clone(), prefix, canonical_name)
            .map_err(DslError::stmt)?;
        match &item {
            RegItem::Element(id) => {
                let _ = self
                    .builder
                    .add_item_property(id, "structurizr.dsl.identifier", final_id);
            }
            // Pseudo objects are throwaway — `addProperty` lands on an
            // object that never enters the model.
            RegItem::Pseudo { .. } | RegItem::Relationship(_) => {}
        }
        Ok(())
    }

    fn register_identifier_relationship(
        &mut self,
        identifier: Option<&str>,
        id: &Id,
        canonical_name: &str,
    ) -> Result<(), DslError> {
        let final_id = self
            .ids
            .register_relationship(identifier, id, canonical_name)
            .map_err(DslError::stmt)?;
        // reference: property added only when an identifier was GIVEN.
        if identifier.is_some_and(|i| !i.is_empty()) {
            let _ = self
                .builder
                .add_item_property(id, "structurizr.dsl.identifier", final_id);
        }
        Ok(())
    }

    /// `calculateHierarchicalIdentifier` — the parent's FOUND identifier
    /// (`null` → literal `"null"` string concatenation). For a parentless
    /// `DeploymentNode` (and a `DeploymentGroup`, whose parent is the
    /// context's environment) the prefix comes from
    /// `findIdentifier(new DeploymentEnvironment(env))` — DSL
    /// `DeploymentEnvironment.equals` is NAME-BASED, so this resolves to the
    /// environment's registered identifier (oracle-verified: `live.n1`),
    /// falling back to the literal `"null"` only when no environment of that
    /// name was registered. `None` = no prefix at all (parentless, non-node).
    fn hierarchical_prefix(&self, item: &RegItem) -> Option<String> {
        if self.ids.scope() != IdentifierScope::Hierarchical {
            return None;
        }
        match item {
            RegItem::Element(id) => match self.builder.element_parent(id) {
                Some(p) => Some(self.ids.find_element_id(&p).unwrap_or("null").to_string()),
                None if self.builder.element_kind(id) == Some(ElementKind::DeploymentNode) => {
                    let env = self.builder.node_ref(id).map(|n| n.environment.clone());
                    Some(
                        env.as_deref()
                            .and_then(|e| self.ids.find_env(e))
                            .unwrap_or("null")
                            .to_string(),
                    )
                }
                None => None,
            },
            RegItem::Pseudo { kind, .. } => match kind {
                // `ElementGroup.getParent()` — set only in the system/
                // container group arms.
                PseudoKind::Group { parent, .. } => parent
                    .as_ref()
                    .map(|p| self.ids.find_element_id(p).unwrap_or("null").to_string()),
                // `DeploymentEnvironment`: no parent, not a node.
                PseudoKind::DeploymentEnvironment(_) => None,
                // `DeploymentGroup.parent` is the context's environment —
                // name-equal to the registered pseudo, so the prefix
                // resolves through it.
                PseudoKind::DeploymentGroup { environment, .. } => {
                    Some(self.ids.find_env(environment).unwrap_or("null").to_string())
                }
            },
            RegItem::Relationship(_) => None,
        }
    }

    // ------------------------------------------------------------------
    // element lookup (reference `DslContext.getElement`)
    // ------------------------------------------------------------------

    /// `context.getElement(identifier)` — case-insensitive,
    /// hierarchical-aware.
    fn get_element(&self, identifier: &str) -> Option<RegItem> {
        self.get_element_typed(identifier, None)
    }

    /// `context.getElement(identifier, Type.class)` — kind-filtered.
    /// `checkElementType` is REVERSED assignability
    /// (`element.getClass().isAssignableFrom(type)`), which for the three
    /// DSL call-site classes (`SoftwareSystem`, `Container`,
    /// `DeploymentGroup` — no subclasses anywhere in the DSL path) means
    /// exact-kind matching. The filter runs PER PARENT HOP (the Java loop
    /// re-checks inside the walk) and once more on the fallback result.
    fn get_element_typed(&self, identifier: &str, kind: Option<KindFilter>) -> Option<RegItem> {
        let identifier = identifier.to_lowercase();
        let mut found = None;
        if self.ids.scope() == IdentifierScope::Hierarchical {
            if let Some(start) = self.hierarchy_start() {
                let mut parent = Some(start);
                while let Some(p) = parent {
                    let parent_identifier = self.ids.find_element_id(&p).unwrap_or("null");
                    if let Some(item) = self
                        .ids
                        .element(&format!("{parent_identifier}.{identifier}"))
                        .filter(|item| self.passes_type_filter(item, kind))
                    {
                        found = Some(item.clone());
                        break;
                    }
                    parent = self.builder.element_parent(&p);
                }
            } else if let Some(env) = self.top().and_then(Ctx::env_name) {
                // `findIdentifier(context.getEnvironment())` — DSL
                // `DeploymentEnvironment.equals` is name-based, so the
                // context's fresh environment object RESOLVES to the
                // registered identifier (literal `"null"` only when no
                // environment of that name was registered).
                let prefix = self.ids.find_env(env).unwrap_or("null");
                found = self.ids.element(&format!("{prefix}.{identifier}")).cloned();
            }
            if found.is_none() {
                // default to finding a top-level element
                found = self.ids.element(&identifier).cloned();
            }
        } else {
            found = self.ids.element(&identifier).cloned();
        }
        found.filter(|item| self.passes_type_filter(item, kind))
    }

    /// `checkElementType` with reversed assignability
    /// (`element.getClass().isAssignableFrom(type)`): for the three DSL
    /// call-site classes (no subclasses in the DSL path) this is exact-kind
    /// matching.
    fn passes_type_filter(&self, item: &RegItem, kind: Option<KindFilter>) -> bool {
        let Some(kind) = kind else {
            return true;
        };
        match (kind, item) {
            (KindFilter::SoftwareSystem, RegItem::Element(id)) => {
                self.builder.element_kind(id) == Some(ElementKind::SoftwareSystem)
            }
            (KindFilter::Container, RegItem::Element(id)) => {
                self.builder.element_kind(id) == Some(ElementKind::Container)
            }
            (
                KindFilter::DeploymentGroup,
                RegItem::Pseudo {
                    kind: PseudoKind::DeploymentGroup { .. },
                    ..
                },
            ) => true,
            _ => false,
        }
    }

    /// The element whose registered identifier seeds the hierarchical
    /// lookup walk: the TOP context's element when TOP is an
    /// `ElementDslContext`, or the frame BELOW it when TOP is the
    /// `ElementsDslContext` created over an element context
    /// (`getParentDslContext()` captured at push time).
    fn hierarchy_start(&self) -> Option<Id> {
        let top = self.stack.last()?;
        if let Some(id) = top.item_id() {
            return Some(id.clone());
        }
        if matches!(top, Ctx::Elements { .. }) {
            return self
                .stack
                .iter()
                .rev()
                .nth(1)
                .and_then(|c| c.item_id())
                .cloned();
        }
        None
    }

    /// `context.getRelationship(identifier)`.
    fn get_relationship(&self, identifier: &str) -> Option<Id> {
        self.ids
            .relationship(&identifier.to_lowercase())
            .and_then(|item| match item {
                RegItem::Relationship(id) => Some(id.clone()),
                _ => None,
            })
    }

    /// `DeploymentElementParser.getDeploymentGroups` — comma-split refs, each
    /// resolved through the node context's `getElement(ref,
    /// DeploymentGroup.class)`; unresolved refs retry the hierarchical
    /// `<envIdentifier>.<ref>` form (the environment resolves by name, like
    /// the Java `equals`), and anything still unresolved is SILENTLY dropped
    /// ("backwards compatibility — deployment environment name rather than
    /// identifier"). Java's `HashSet` order is not portable; insertion order
    /// stands in (D8 family, agents/plan/PROGRESS.md).
    pub(crate) fn deployment_groups(&self, node: &Id, token: &str) -> Vec<String> {
        let env = self.builder.node_ref(node).map(|n| n.environment.clone());
        let mut groups: Vec<String> = Vec::new();
        for reference in split_keeping_lead_dropping_trail(token) {
            let mut found = self.get_element_typed(reference, Some(KindFilter::DeploymentGroup));
            if found.is_none() {
                let prefix = env
                    .as_deref()
                    .and_then(|e| self.ids.find_env(e))
                    .unwrap_or("null");
                found = self.get_element_typed(
                    &format!("{prefix}.{reference}"),
                    Some(KindFilter::DeploymentGroup),
                );
            }
            if let Some(RegItem::Pseudo {
                kind: PseudoKind::DeploymentGroup { name, .. },
                ..
            }) = found
                && !groups.iter().any(|g| g == &name)
            {
                groups.push(name);
            }
        }
        groups
    }

    // ------------------------------------------------------------------
    // dynamic-view sequence numbers (reference `SequenceNumber`)
    // ------------------------------------------------------------------

    fn dynamic_idx(view: ViewRef) -> usize {
        match view {
            ViewRef::Dynamic(i) => i,
            _ => usize::MAX,
        }
    }

    /// `SequenceNumber.getNext()` — read the number a step WOULD take
    /// (top + 1) without burning it. Java evaluates `getNext()` as the
    /// argument of the final `addRelationship(...)` call, i.e. only after
    /// every lookup/validation throw; pair this with [`Self::seq_commit`]
    /// right after a successful core add so failed steps consume nothing.
    fn seq_peek(&mut self, view: ViewRef) -> String {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.is_empty() {
            stack.push(SeqFrame {
                sequence: 0,
                incremented: false,
            });
        }
        (stack.last().expect("root frame").sequence + 1).to_string()
    }

    /// Burn one sequence number (`getNext()`'s increment) — call only after
    /// the core add returned `Ok`, mirroring where Java evaluates `getNext()`.
    fn seq_commit(&mut self, view: ViewRef) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.is_empty() {
            stack.push(SeqFrame {
                sequence: 0,
                incremented: false,
            });
        }
        let top = stack.last_mut().expect("root frame");
        top.sequence += 1;
        top.incremented = true;
    }

    /// `startParallelSequence()` — a child counter seeded from the parent.
    fn seq_start_parallel(&mut self, view: ViewRef) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        let seed = stack.last().map_or(0, |f| f.sequence);
        stack.push(SeqFrame {
            sequence: seed,
            incremented: false,
        });
    }

    /// `endParallelSequence(endAll)` — both `endAll` branches copy the
    /// child sequence up; the child being un-incremented adds the extra
    /// parent increment (the "empty parallel block doesn't burn a number"
    /// semantics).
    fn seq_end_parallel(&mut self, view: ViewRef, end_all: bool) {
        let stack = self.seq.entry(Self::dynamic_idx(view)).or_default();
        if stack.len() < 2 {
            // Java: counter.getParent() == null → NPE; the DSL never
            // closes a parallel sequence that was never started, and D1
            // truncation can't leave one open. No-op.
            return;
        }
        let child = stack.pop().expect("len >= 2");
        if end_all {
            let parent = stack.last_mut().expect("len >= 1");
            parent.sequence = child.sequence;
            if !child.incremented {
                parent.sequence += 1;
                parent.incremented = true;
            }
        }
    }

    // ------------------------------------------------------------------
    // ${} substitution (`substituteStrings`)
    // ------------------------------------------------------------------

    /// Java `String.replace` is literal; the `Matcher` iterates occurrences
    /// of the ORIGINAL token while each hit rewrites the current token in
    /// full — occurrences introduced by an earlier replacement are NOT
    /// substituted. That is modeled here by collecting the occurrences from
    /// the original first.
    fn substitute_strings(&self, token: &str) -> String {
        // `(\$\{[a-zA-Z0-9-_.]+?})` — the lazy `+` equals "up to the first
        // }", and name chars exclude `}`, so scan `${` … nearest `}` with
        // all-valid non-empty name.
        let mut occurrences: Vec<String> = Vec::new();
        let mut i = 0usize;
        while let Some(rel) = token[i..].find("${") {
            let start = i + rel;
            let rest = &token[start + 2..];
            let name_end = rest.find('}');
            let name_ok = name_end.is_some_and(|e| {
                let name = &rest[..e];
                !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            });
            if name_ok {
                let e = name_end.unwrap_or(0);
                occurrences.push(token[start..=(start + 2 + e)].to_string());
                i = start + 2 + e + 1;
            } else {
                i = start + 1;
            }
        }
        // The Java Matcher scans the ORIGINAL token but `token.replace`
        // mutates the working string — so every match (including repeats)
        // triggers a replace-all on the CURRENT token, in scan order.
        // Replacement-introduced `${…}` is never rescanned; a constant's
        // value is substituted RAW (only TextBlocks re-substitute).
        let mut token = token.to_string();
        for before in occurrences {
            let name = &before[2..before.len() - 1];
            let after = if let Some(nv) = self.constants.get(name) {
                Some(match nv.vtype {
                    NameValueType::TextBlock => self.substitute_strings(&nv.value),
                    _ => nv.value.clone(),
                })
            } else {
                // V-3: env substitution is ON (reference parser ctor default)
                // unless a hermetic lookup is injected (Phase 1T P1T-D1).
                match &self.opts.env_lookup {
                    Some(lookup) => lookup.get(name),
                    None => std::env::var(name).ok(),
                }
            };
            if let Some(after) = after {
                token = token.replace(&before, &after);
            }
        }
        token
    }

    // ------------------------------------------------------------------
    // archetype registry (reference `getArchetype`/`isElement…Archetype`)
    // ------------------------------------------------------------------

    fn is_element_keyword_or_archetype(&self, token: &str, keyword: &str) -> bool {
        eqi(token, keyword)
            || self
                .archetypes
                .get(keyword)
                .is_some_and(|m| m.contains_key(&token.to_lowercase()))
    }

    /// Java `isRelationshipKeywordOrArchetype`. Known corner: the token
    /// `-->` satisfies both prefix and suffix, and Java's
    /// `substring(2, len-2)` THROWS (`String index out of range`) — here it
    /// evaluates as no-match (recorded in agents/plan/PROGRESS.md; no corpus file
    /// uses `-->`).
    fn is_relationship_keyword_or_archetype(&self, token: &str) -> bool {
        if eqi(token, tok::RELATIONSHIP) {
            return true;
        }
        if token.len() >= 4
            && token.starts_with(tok::RELATIONSHIP_ARCHETYPE_PREFIX)
            && token.ends_with(tok::RELATIONSHIP_ARCHETYPE_SUFFIX)
        {
            let inner = &token[2..token.len() - 2];
            return self
                .archetypes
                .get(tok::RELATIONSHIP)
                .is_some_and(|m| m.contains_key(&inner.to_lowercase()));
        }
        false
    }

    /// `getArchetype(type, name)` — the registered archetype index, or a
    /// fresh EMPTY (unregistered) archetype for an unknown name (the chain
    /// guarantees the token was a keyword/archetype check hit; the keyword
    /// itself is NOT registered, so `person …` with no archetype gets an
    /// empty throwaway exactly like the reference).
    fn get_archetype(&mut self, kind: &str, name: &str) -> usize {
        let mut name = name.to_string();
        if kind == tok::RELATIONSHIP
            && name.len() >= 4
            && name.starts_with(tok::RELATIONSHIP_ARCHETYPE_PREFIX)
            && name.ends_with(tok::RELATIONSHIP_ARCHETYPE_SUFFIX)
        {
            name = name[2..name.len() - 2].to_string();
        }
        let key = name.to_lowercase();
        if let Some(&idx) = self.archetypes.get(kind).and_then(|m| m.get(&key)) {
            return idx;
        }
        self.archetype_list.push(Archetype::new(Some(&name), kind));
        self.archetype_list.len() - 1
    }

    /// `addArchetype` — register the (context's) archetype.
    fn add_archetype(&mut self, idx: usize) {
        let a = &self.archetype_list[idx];
        let (kind, name) = (a.kind.clone(), a.name.clone());
        self.archetypes.entry(kind).or_default().insert(name, idx);
    }

    /// `extendArchetype`: copy metadata/description/technology/tags from
    /// the (possibly freshly created, empty) parent archetype.
    fn extend_archetype(&mut self, idx: usize, archetype_name: &str) {
        let kind = self.archetype_list[idx].kind.clone();
        let parent = self.get_archetype(&kind, archetype_name);
        if parent == idx {
            return; // extending itself: copying its own values is a no-op
        }
        let p = self.archetype_list[parent].clone();
        let a = &mut self.archetype_list[idx];
        a.metadata = p.metadata;
        a.description = p.description;
        a.technology = p.technology;
        for t in p.tags {
            // Parent tags are already trimmed; this is pure `addTags`
            // deduplication into the child's `LinkedHashSet`.
            Self::add_archetype_tag(a, &t);
        }
    }

    /// One value of Java `Archetype.addTags(String...)`: the value is
    /// TRIMMED and added to the `LinkedHashSet` — an empty trimmed value IS
    /// added (`this.tags.add(tag.trim())` is unconditional, verified against
    /// `Archetype.java`); duplicates are ignored.
    fn add_archetype_tag(a: &mut Archetype, tag: &str) {
        let tag = tag.trim();
        if !a.tags.iter().any(|t| t == tag) {
            a.tags.push(tag.to_string());
        }
    }

    /// The `tags <tags> [tags]` statement: `addTags(tags.split(","))` per
    /// token — Java `split` DROPS trailing empty strings; leading/embedded
    /// empties are kept and reach `addTags` (they land as empty tags).
    fn add_archetype_tags(a: &mut Archetype, tags: &str) {
        for t in split_keeping_lead_dropping_trail(tags) {
            Self::add_archetype_tag(a, t);
        }
    }

    // ------------------------------------------------------------------
    // properties / perspectives holders
    // ------------------------------------------------------------------

    fn apply_property(&mut self, holder: &Holder, name: &str, value: &str) {
        match holder {
            Holder::Workspace => self.builder.set_property(name, value),
            Holder::Model => {
                self.builder.model_mut().properties.insert(name, value);
            }
            Holder::Item(id) => {
                let _ = self.builder.add_item_property(id, name, value);
            }
            Holder::Items(ids) => {
                for id in ids {
                    let _ = self.builder.add_item_property(id, name, value);
                }
            }
            Holder::Archetype { idx } => {
                // `LinkedHashMap.put` semantics: first-insert order,
                // overwrite value.
                let a = &mut self.archetype_list[*idx];
                match a.properties.iter_mut().find(|(n, _)| n == name) {
                    Some(slot) => slot.1 = value.to_string(),
                    None => a.properties.push((name.to_string(), value.to_string())),
                }
            }
            Holder::View(view) => {
                if let Some(v) = self.builder.view_base_mut(*view) {
                    v.properties.insert(name, value);
                }
            }
            Holder::DynamicRel { view, rel } => {
                if let Some(rv) = self.relationship_view_mut(*view, rel) {
                    rv.properties.insert(name, value);
                }
            }
            Holder::ElementStyle { idx } => {
                if let Some(s) = self
                    .builder
                    .views_mut()
                    .configuration
                    .styles
                    .element_styles
                    .get_mut(*idx)
                {
                    s.properties.insert(name, value);
                }
            }
            Holder::RelationshipStyle { idx } => {
                if let Some(s) = self
                    .builder
                    .views_mut()
                    .configuration
                    .styles
                    .relationship_styles
                    .get_mut(*idx)
                {
                    s.properties.insert(name, value);
                }
            }
            // `views { properties … }` — the reference's ViewsDslContext is
            // a PropertyHolder over the view-set configuration.
            Holder::ViewSet => {
                self.builder
                    .views_mut()
                    .configuration
                    .properties
                    .insert(name, value);
            }
        }
    }

    /// `PerspectivesDslContext` statement form / `PerspectiveDslContext.end()`
    /// — add to EVERY holder of the context.
    // Justified: the perspective is cloned into every holder, so the last
    // clone could consume the parameter — but the loop shape mirrors the
    // reference and keeps `?`-early-exit semantics identical.
    #[allow(clippy::needless_pass_by_value)]
    fn add_perspective_to_holders(
        &mut self,
        holders: &[Holder],
        perspective: oxidizr_core::model::Perspective,
    ) -> Result<(), DslError> {
        for h in holders {
            self.add_perspective_to_holder(h, perspective.clone())?;
        }
        Ok(())
    }

    fn add_perspective_to_holder(
        &mut self,
        holder: &Holder,
        perspective: oxidizr_core::model::Perspective,
    ) -> Result<(), DslError> {
        match holder {
            Holder::Item(id) => {
                self.builder
                    .add_perspective(id, perspective)
                    .map_err(|e| DslError::core(&e))?;
                Ok(())
            }
            Holder::Items(ids) => {
                // The reference loop applies holders in order; a duplicate
                // on a later one leaves the earlier ones MUTATED (faithful).
                for id in ids {
                    self.builder
                        .add_perspective(id, perspective.clone())
                        .map_err(|e| DslError::core(&e))?;
                }
                Ok(())
            }
            Holder::Archetype { idx } => {
                // `Archetype.addPerspective(Perspective)` — the same
                // duplicate guard.
                let dup = self.archetype_list[*idx]
                    .perspectives
                    .iter()
                    .any(|p| p.name == perspective.name);
                if dup {
                    return Err(DslError::stmt(format!(
                        "A perspective named \"{}\" already exists.",
                        perspective.name
                    )));
                }
                self.archetype_list[*idx].perspectives.push(perspective);
                Ok(())
            }
            // The reference's `PerspectivesHolder` universe covers model
            // items and archetypes only; a perspective elsewhere is parsed
            // and dropped (the context has no holder to append to).
            _ => Ok(()),
        }
    }

    // ------------------------------------------------------------------
    // builder accessors used across sections
    // ------------------------------------------------------------------

    fn relationship_view_mut(
        &mut self,
        view: ViewRef,
        rel: &Id,
    ) -> Option<&mut oxidizr_core::views::RelationshipView> {
        let content = self.builder.content_mut(view)?;
        // Java `DynamicViewRelationshipContext` resolves the view through
        // `getRelationshipView(id)`-style lookups over the (TreeSet-ordered)
        // relationship views; with duplicate ids the reference's setter hits
        // the LAST match, so mutate from the back.
        content
            .relationships
            .iter_mut()
            .rev()
            .find(|rv| &rv.id == rel)
    }

    // ------------------------------------------------------------------
    // parse driver
    // ------------------------------------------------------------------

    /// `parse(List<String> lines, File dslFile, fragment, includeInSource)`
    /// over already-preprocessed logical lines. `source` is the file text
    /// the logical lines index into.
    fn parse_lines(&mut self, lines: &[Logical], source: &str, track_source: bool) {
        if track_source {
            // Java accumulates the RAW lines (before per-line BOM strip and
            // before preprocessing folding); includes are never tracked.
            // The reference's entry is `parse(String)`: `dsl.split("\\r?\\n")`
            // — one `\r` before each `\n` is consumed by the split, and Java's
            // `split` drops ALL trailing empty entries (a file ending with a
            // newline never retains it; trailing blank lines vanish too).
            // M1-e A/B probe 2026-11-17 (`m1d` witness): the reference's
            // `structurizr.dsl` base64 has no trailing newline.
            self.dsl_source_lines
                .extend(compat::java_split_dsl_lines(source));
        }
        for logical in lines {
            if logical.dropped {
                // the reference's buffer never flushed this tail
                continue;
            }
            let height = self.stack.len();
            let outcome = (|| {
                let raw = self.logical_raw(logical, source)?;
                self.process_line(logical.number, raw)
            })();
            if let Err(e) = outcome {
                // D1 recovery: record, pop back to the statement boundary.
                let file = self.current_file();
                let fatal = e.fatal_file;
                self.stack.truncate(height);
                self.errors.push(e.at(
                    file,
                    Some(logical.number),
                    Some(compat::java_trim(&logical.joined(source)).to_string()),
                ));
                // A failed text-block fold is fatal for the whole FILE in
                // the reference (preProcessLines runs BEFORE any line is
                // processed) — closest recoverable behavior: stop here.
                if fatal {
                    break;
                }
            }
        }
    }

    /// The logical line text the parse loop sees: the joined segments, plus
    /// the text-block fold — the reference replaces `prefix """…"""` with
    /// `prefix "${<uuid>}"` and registers the content as a `TextBlock`
    /// constant BEFORE the loop (`preProcessLines`).
    fn logical_raw(&mut self, logical: &Logical, source: &str) -> Result<String, DslError> {
        let mut raw = logical.joined(source); // BOM already stripped by preprocess
        if let Some(tb) = logical.text_block {
            let value = text_block_value(source, &tb).ok_or_else(|| {
                let mut e = DslError::stmt("String index out of range: -1");
                e.fatal_file = true;
                e
            })?;
            let name = uuid_v4();
            raw.push_str("\"${");
            raw.push_str(&name);
            raw.push_str("}\"");
            self.constants.insert(
                name.clone(),
                NameValue {
                    name,
                    value,
                    vtype: NameValueType::TextBlock,
                },
            );
        }
        Ok(raw)
    }

    /// The per-logical-line pipeline: gates (empty/comment), tokenizer,
    /// `${}` substitution, `id = ` prefix, then the dispatch chain.
    fn process_line(&mut self, number: u32, raw: String) -> Result<(), DslError> {
        // 255: EMPTY_LINE_PATTERN (`^\s*` .matches()).
        if raw.chars().all(compat::is_java_regex_s) {
            return Ok(());
        }
        // 257: COMMENT_PATTERN (`^\s*?(//|#).*$`).
        {
            let rest = raw.trim_start_matches(compat::is_java_regex_s);
            if rest.starts_with("//") || rest.starts_with('#') {
                return Ok(());
            }
        }
        // 259–264: InlineScriptDslContext swallowing — unreachable: `!script`
        // is rejected (OXZ-5002), so that context is never on the stack.

        // 266–269: tokenize + per-token substitution.
        let raw_tokens = tokenize::tokenize(&raw);
        let mut substituted: Vec<String> = raw_tokens
            .iter()
            .map(|t| self.substitute_strings(t))
            .collect();

        // 271–278: the `id = ` prefix. Applied on the token Vec before the
        // `Tokens` wrap (2R R-dsl-01 — the old `tokens.sublist(2)` reassign
        // cannot express itself over borrowed views; the unescape view and
        // the check order are identical).
        let mut identifier = None;
        if substituted.len() >= 3 && Tokens::unescape(&substituted[1]) == tok::ASSIGNMENT {
            let ident = Tokens::unescape(&substituted[0]).into_owned();
            IdentifiersRegister::validate_identifier_name(&ident).map_err(DslError::stmt)?;
            identifier = Some(ident);
            substituted = substituted.split_off(2);
        }
        let tokens = Tokens::new(substituted);

        // `String firstToken = tokens.get(0)` — Java throws
        // IndexOutOfBoundsException for a token list that is empty AFTER the
        // id-strip (only reachable via degenerate lines like `""` /
        // `x = ""`); the JDK message is reproduced.
        if tokens.is_empty() {
            return Err(DslError::stmt("Index 0 out of bounds for length 0"));
        }

        let stmt = Stmt {
            raw,
            number,
            tokens,
            identifier,
        };
        for section in SECTIONS {
            match section(self, &stmt) {
                Sec::Handled => return Ok(()),
                Sec::Err(e) => return Err(e),
                Sec::No => {}
            }
        }
        Err(self.catch_all())
    }

    /// The chain's catch-all (1340–1366).
    fn catch_all(&self) -> DslError {
        // Java picks the expected-token list from the TOP context; with an
        // empty stack, from whether the workspace object exists yet.
        if self.stack.is_empty() {
            if !self.have_workspace {
                return DslError::unexpected("Unexpected tokens (expected: workspace)");
            }
            return DslError::unexpected("Unexpected tokens");
        }
        match self.top().and_then(Ctx::permitted_tokens_message) {
            Some(m) => DslError::unexpected(m),
            None => DslError::unexpected("Unexpected tokens"),
        }
    }

    /// `shouldStartContext(tokens)` — the statement ends with `{`.
    fn should_start_context(tokens: &Tokens) -> bool {
        tokens.last().is_some_and(|t| eqi(&t, tok::CONTEXT_START))
    }

    // ------------------------------------------------------------------
    // file context (includes)
    // ------------------------------------------------------------------

    /// The `dslFile` of the current recursion level, in the form reference
    /// messages use it: `dslFile.isFile() ? dslFile.getAbsolutePath()` —
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
    /// `new File(dslFile.getParent(), source)`; no file ⇒ the CWD
    /// (`new File(".").getParent()` = `null` ⇒ process dir in Java).
    pub(crate) fn current_dir(&self) -> Option<String> {
        match self.current_file_raw() {
            Some(f) => match std::path::Path::new(&f).parent() {
                Some(p) if !p.as_os_str().is_empty() => Some(p.to_string_lossy().into_owned()),
                _ => Some(String::new()), // relative filename input: parent == ""
            },
            None => self.opts.base_dir.map(str::to_string),
        }
    }

    /// Enter an included file: push its `dslFile`, parse, pop (the reference
    /// recursion `parse(paddedLines, file, true, false)` — fragments are not
    /// source-tracked and their context stack must return to the entry
    /// height; under D1 the truncation-per-line already guarantees that, so
    /// no extra check).
    pub(crate) fn parse_included(&mut self, source: &str) {
        let path = self.last_include_path.clone();
        self.file_stack.push(path);
        let logicals = crate::preprocess::preprocess(source);
        self.parse_lines(&logicals, source, false);
        self.file_stack.pop();
        self.last_include_path = None;
    }

    /// `workspace extends <file|url>` (M5, `PHASE_1.md` §M5-b) — the file
    /// branch of `WorkspaceParser.parse` @`ad34036`. The loaded base REPLACES
    /// the workspace under construction; later statements layer over it.
    /// The reference's non-file guard (`context.getFile() == null`) skips the
    /// whole branch silently — same quirk as file `!include`s.
    pub(crate) fn extend_from(&mut self, source: &str) -> Result<(), DslError> {
        // D5 (mirrors the `!include` arm): ALL network input rejected, the
        // reference feature-gate texts kept (`Features.HTTPS`/`HTTP` messages
        // of `WorkspaceParser.parse`).
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

        let loaded = if source.to_lowercase().ends_with(".json") {
            // `WorkspaceUtils.loadWorkspaceFromJson(file)` — via the
            // injected loader (topology: no `json` dep in this crate).
            let Some(loader) = self.opts.json_loader else {
                return Err(DslError::stmt(format!(
                    "extending {source} requires a JSON workspace loader, none is configured"
                )));
            };
            loader(&stmt_line::java_canonical(&path)).map_err(DslError::stmt)?
        } else {
            // child `StructurizrDslParser` (`createParser` + `parse(context,
            // file)`): fresh parser state over the same file entry; its
            // `configureFrom` handover happens below, before `finish()`.
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
            // `context.getParser().configureFrom(structurizrDslParser)`:
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
        // JSON bases: `WorkspaceParser.registerIdentifiers` — elements AND
        // relationships carrying `structurizr.dsl.identifier` enter the
        // (context-shared) register so `!element <identifier>` finds them.
        if source.to_lowercase().ends_with(".json") {
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
        }
        // `DslUtils.clearDsl(workspace)` (the base's own retention is
        // discarded — the EXTENDING file is the source) and the reference's
        // `context.setDslPortable(false)` on the WorkspaceParser context:
        // the `WorkspaceDslContext` frame is seeded NOT-portable, so the
        // extending file keeps no source either (`DslTests` asserts
        // `DslUtils.getDsl` == "" for `source-child.dsl`). The statement
        // runs on an empty stack, so this is the parser-level flag.
        self.builder.remove_property("structurizr.dsl");
        self.dsl_portable = false;
        Ok(())
    }

    // ------------------------------------------------------------------
    // finalization
    // ------------------------------------------------------------------

    fn finish(mut self) -> ParseResult {
        // EOF: unclosed contexts (1376–1378, `!fragment` only — the engine's
        // public entry is always a whole workspace file).
        if !self.stack.is_empty() {
            self.errors.push(DslError {
                code: oxidizr_diag::codes::UNEXPECTED_EOF,
                message:
                    "Unexpected end of DSL content - are one or more closing curly braces missing?"
                        .to_string(),
                file: None,
                line: None,
                source_line: None,
                fatal_file: false,
            });
        }

        // `getWorkspace()`: `if (dslPortable)` attach the retained source.
        if self.have_workspace && self.dsl_portable {
            let existing = self
                .builder
                .workspace()
                .properties
                .get("structurizr.dsl")
                .map(str::to_string);
            if existing.is_none_or(|v| compat::java_eq_ignore_case(&v, "true")) {
                let joined = self.dsl_source_lines.join("\n");
                self.builder
                    .set_property("structurizr.dsl", base64_std(&joined));
            }
        }

        ParseResult {
            workspace: self.builder.build(),
            errors: self.errors,
        }
    }
}

/// The pseudo `RegItem` for a `deploymentEnvironment` name/uid pair.
#[must_use]
pub(crate) fn env_pseudo(uid: u64, name: &str) -> RegItem {
    RegItem::Pseudo {
        uid,
        kind: PseudoKind::DeploymentEnvironment(name.to_string()),
    }
}

/// The pseudo `RegItem` for a `deploymentGroup`.
#[must_use]
pub(crate) fn deployment_group_pseudo(uid: u64, environment: &str, name: &str) -> RegItem {
    RegItem::Pseudo {
        uid,
        kind: PseudoKind::DeploymentGroup {
            environment: environment.to_string(),
            name: name.to_string(),
        },
    }
}

/// The pseudo `RegItem` for an `ElementGroup` (`parent` = the
/// system/container the group block sits in — `ElementGroup.setParent`).
#[must_use]
pub(crate) fn group_pseudo(uid: u64, name: &str, parent: Option<&Id>) -> RegItem {
    RegItem::Pseudo {
        uid,
        kind: PseudoKind::Group {
            name: name.to_string(),
            parent: parent.cloned(),
        },
    }
}

impl Parser<'_> {
    /// Open the member set for a fresh `ElementGroup` object (a `group`
    /// statement); returns its `uid`. `parent_uid` is the enclosing group
    /// (`ElementGroup.parentGroup`).
    // `cast_possible_truncation` justification: uids count model items;
    // the u64→usize narrowing can not overflow any real corpus.
    #[allow(clippy::cast_possible_truncation)]
    pub(crate) fn group_open(&mut self, parent_uid: Option<u64>) -> u64 {
        let uid = self.next_uid();
        // `next_uid` also serves pseudo identity; the record table is
        // indexed uid-1, so uids consumed elsewhere leave empty records.
        while self.groups.len() < uid as usize {
            self.groups.push(GroupRecord::default());
        }
        self.groups[uid as usize - 1].parent_uid = parent_uid;
        uid
    }

    /// `ElementGroup.addElement`: record membership, propagating to the
    /// parent group object exactly like the reference.
    #[allow(clippy::cast_possible_truncation)] // uid counts model items (see group_open)
    pub(crate) fn group_add_member(&mut self, uid: u64, member: &Id) {
        let mut cur = Some(uid);
        while let Some(u) = cur {
            let idx = u as usize - 1;
            if idx >= self.groups.len() {
                break;
            }
            if !self.groups[idx].members.contains(member) {
                self.groups[idx].members.push(member.clone());
            }
            cur = self.groups[idx].parent_uid;
        }
    }

    /// `ElementGroup.getElements()` — the (propagated) member set.
    #[allow(clippy::cast_possible_truncation)] // uid counts model items (see group_open)
    pub(crate) fn group_members(&self, uid: u64) -> Vec<Id> {
        self.groups
            .get(uid as usize - 1)
            .map(|r| r.members.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // element-definition and relationship-statement post-processing
    // ------------------------------------------------------------------

    /// The shared post-creation sequence of `PersonParser`,
    /// `SoftwareSystemParser`, `ContainerParser` and `ComponentParser`
    /// (the custom-element parser deviates — see `stmt_model`):
    /// description is set UNCONDITIONALLY (`null` → `""`), then
    /// `addTags(archetype ++ token parts)`, the archetype properties and
    /// perspectives, and finally the enclosing context's group
    /// (`setGroup` + `group.addElement`, which propagates to parent
    /// group objects).
    ///
    /// `tags` is the reference's combined list (archetype tags first,
    /// then the comma-split statement token, each entry trimmed and
    /// first-position-deduped by [`WorkspaceBuilder::add_tag_parts`]).
    pub(crate) fn define_element(
        &mut self,
        id: &Id,
        description: &str,
        tags: &[String],
        arch: &RelArch,
    ) -> Result<(), DslError> {
        let _ = self.builder.set_description(id, description);
        self.builder.add_tag_parts(id, tags);
        for (name, value) in &arch.properties {
            let _ = self.builder.add_item_property(id, name, value);
        }
        for p in &arch.perspectives {
            // The archetype application goes through the plural
            // `addPerspectives`, which delegates to the 3-arg
            // `addPerspective` — `Perspective.setName` rejects names that
            // are null/blank FIRST, then the duplicate guard fires (a
            // blank-named archetype perspective therefore fails HERE, on
            // application, not when the archetype block defined it).
            if p.name.trim().is_empty() {
                return Err(DslError::core(&CoreError::PerspectiveNameRequired));
            }
            self.builder
                .add_perspective(id, p.clone())
                .map_err(|e| DslError::core(&e))?;
        }
        let group = self.top().and_then(Ctx::group_payload).cloned();
        if let Some(group) = group {
            let _ = self.builder.set_group(id, &group.name);
            self.group_add_member(group.uid, id);
        }
        Ok(())
    }

    /// The relationship-statement registration block shared by the four
    /// chain arms: exactly ONE created relationship is registered (and a
    /// `{` opens a `RelationshipDslContext`); several create a
    /// `RelationshipsDslContext` only (`getContext()` — top of stack).
    /// `raw` is the RAW token list — `shouldStartContext` reads the
    /// original tokens including the trailing `{`.
    pub(crate) fn register_rel_result(
        &mut self,
        identifier: Option<&str>,
        rels: Vec<Id>,
        raw: &Tokens,
    ) -> Result<(), DslError> {
        let open = Self::should_start_context(raw);
        if rels.len() == 1 {
            let id = rels[0].clone();
            self.register_identifier_relationship(identifier, &id, "")?;
            if open {
                self.start_context(Ctx::Rel { id });
            }
        } else if open {
            self.start_context(Ctx::Relationships { items: rels });
        }
        Ok(())
    }

    /// `GroupParser.parseContext` + `startContext`: re-pushes the TOP
    /// context of the stack carrying a FRESH `ElementGroup` object (the
    /// five groupable context classes, each reconstructed with the new
    /// payload — reference `new SoftwareSystemDslContext(ss, group)`).
    // Justified: the group is cloned into the replacement frame; callers
    // hand over a group they no longer use (reference `new X(group)`).
    #[allow(clippy::needless_pass_by_value)]
    pub(crate) fn start_group_context(&mut self, top: &Ctx, group: DslGroup) {
        let replacement = match top {
            Ctx::Model { .. } => Ctx::Model {
                group: Some(group.clone()),
            },
            Ctx::SoftwareSystem { id, .. } => Ctx::SoftwareSystem {
                id: id.clone(),
                group: Some(group.clone()),
            },
            Ctx::Container { id, .. } => Ctx::Container {
                id: id.clone(),
                group: Some(group.clone()),
            },
            Ctx::Environment { env, .. } => Ctx::Environment {
                env: env.clone(),
                group: Some(group.clone()),
            },
            // A `group` inside `source -/> destination {` matches the
            // ENVIRONMENT arm (subclass instanceof) and re-pushes a PLAIN
            // `new DeploymentEnvironmentDslContext(name, group)` — the
            // NoRelInEnv nature (and its linked relationship) is lost.
            Ctx::NoRelInEnv { environment, .. } => Ctx::Environment {
                env: DslEnvironment {
                    name: environment.clone(),
                },
                group: Some(group.clone()),
            },
            Ctx::DeploymentNode { id, .. } => Ctx::DeploymentNode {
                id: id.clone(),
                group: Some(group.clone()),
            },
            other => {
                // Unreachable: callers only pass groupable payload-bearing
                // tops; defensive replay of the Java `instanceof` ladder.
                let _ = other;
                return;
            }
        };
        self.start_context(replacement);
    }
}

/// Kind filters for `getElement(id, Type.class)` — see
/// [`Parser::passes_type_filter`] for why exact-kind is the right reading of
/// the reversed `isAssignableFrom` check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    SoftwareSystem,
    Container,
    DeploymentGroup,
}

/// Java fully-qualified-name tail for the `Expected … but got …` cast error.
pub(crate) fn java_class_name(ctx: &Ctx) -> &'static str {
    match ctx {
        Ctx::Workspace => "WorkspaceDslContext",
        Ctx::Comment => "CommentDslContext",
        Ctx::Model { .. } => "ModelDslContext",
        Ctx::Archetypes => "ArchetypesDslContext",
        Ctx::ElementArchetype { kind, .. } => match kind.as_str() {
            tok::PERSON => "PersonArchetypeDslContext",
            tok::SOFTWARE_SYSTEM => "SoftwareSystemArchetypeDslContext",
            tok::CUSTOM_ELEMENT => "CustomElementArchetypeDslContext",
            tok::CONTAINER => "ContainerArchetypeDslContext",
            tok::COMPONENT => "ComponentArchetypeDslContext",
            tok::DEPLOYMENT_NODE => "DeploymentNodeArchetypeDslContext",
            _ => "InfrastructureNodeArchetypeDslContext",
        },
        Ctx::RelationshipArchetype { .. } => "RelationshipArchetypeDslContext",
        Ctx::Person { .. } => "PersonDslContext",
        Ctx::CustomElement { .. } => "CustomElementDslContext",
        Ctx::SoftwareSystem { .. } => "SoftwareSystemDslContext",
        Ctx::Container { .. } => "ContainerDslContext",
        Ctx::Component { .. } => "ComponentDslContext",
        Ctx::Rel { .. } => "RelationshipDslContext",
        Ctx::Elements { .. } => "ElementsDslContext",
        Ctx::Relationships { .. } => "RelationshipsDslContext",
        Ctx::Environment { .. } => "DeploymentEnvironmentDslContext",
        Ctx::DeploymentNode { .. } => "DeploymentNodeDslContext",
        Ctx::InfrastructureNode { .. } => "InfrastructureNodeDslContext",
        Ctx::SsInstance { .. } => "SoftwareSystemInstanceDslContext",
        Ctx::ContainerInstance { .. } => "ContainerInstanceDslContext",
        Ctx::NoRelInEnv { .. } => "NoRelationshipInDeploymentEnvironmentDslContext",
        Ctx::Views => "ViewsDslContext",
        Ctx::LandscapeView { .. } => "SystemLandscapeViewDslContext",
        Ctx::ContextView { .. } => "SystemContextViewDslContext",
        Ctx::ContainerView { .. } => "ContainerViewDslContext",
        Ctx::ComponentView { .. } => "ComponentViewDslContext",
        Ctx::FilteredView { .. } => "FilteredViewDslContext",
        Ctx::DynamicView { .. } => "DynamicViewDslContext",
        Ctx::DynamicParallel { .. } => "DynamicViewParallelSequenceDslContext",
        Ctx::DynamicRel { .. } => "DynamicViewRelationshipContext",
        Ctx::DeploymentView { .. } => "DeploymentViewDslContext",
        Ctx::CustomView { .. } => "CustomViewDslContext",
        Ctx::ImageView { .. } => "ImageViewDslContext",
        Ctx::StaticViewAnimation { .. } => "StaticViewAnimationDslContext",
        Ctx::DeploymentViewAnimation { .. } => "DeploymentViewAnimationDslContext",
        Ctx::CustomViewAnimation { .. } => "CustomViewAnimationDslContext",
        Ctx::Styles { .. } => "StylesDslContext",
        Ctx::ElementStyle { .. } => "ElementStyleDslContext",
        Ctx::RelationshipStyle { .. } => "RelationshipStyleDslContext",
        Ctx::Terminology => "TerminologyDslContext",
        Ctx::Configuration => "ConfigurationDslContext",
        Ctx::Users => "UsersDslContext",
        Ctx::Documentation { .. } => "DocumentationDslContext",
        Ctx::Decisions { .. } => "DecisionsDslContext",
        Ctx::Properties { .. } => "PropertiesDslContext",
        Ctx::Perspectives { .. } => "PerspectivesDslContext",
        Ctx::Perspective { .. } => "PerspectiveDslContext",
    }
}

/// Standard-base64 (RFC 4648, padded) — matches Java `Base64.getEncoder()`.
/// Generic over byte sources so raw file bytes (which need not be valid UTF-8)
/// encode exactly like Java's `Files.readAllBytes` + `Base64.getEncoder()`.
#[must_use]
pub(crate) fn base64_std(bytes: impl AsRef<[u8]>) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Java `String.split(",")`: leading and MIDDLE empties are kept, trailing
/// empties dropped — and an all-empty split (including empty input) yields
/// an EMPTY array, exactly like the reference (`"".split(",")` → `[]`).
pub(crate) fn split_keeping_lead_dropping_trail(s: &str) -> Vec<&str> {
    let parts: Vec<&str> = s.split(',').collect();
    drop_trailing_empties(&parts)
}

/// Java `String.split(String)` (literal), same all-empty ⇒ `[]` semantics.
pub(crate) fn java_split_str<'a>(s: &'a str, sep: &str) -> Vec<&'a str> {
    let parts: Vec<&str> = s.split(sep).collect();
    drop_trailing_empties(&parts)
}

/// The reference out-of-bounds condition a Java `parts[i]` on a split
/// result would hit (e.g. the expression `" && "` ⇒ `split` yields `[]`);
/// surfaced as the pinned JVM message instead of a panic.
pub(crate) fn java_index_or_oob<'a>(parts: &'a [&'a str], i: usize) -> Result<&'a str, String> {
    parts
        .get(i)
        .copied()
        .ok_or_else(|| format!("Index {i} out of bounds for length {}", parts.len()))
}

fn drop_trailing_empties<'a>(parts: &[&'a str]) -> Vec<&'a str> {
    let end = parts
        .iter()
        .rposition(|p| !p.is_empty())
        .map_or(0, |i| i + 1);
    parts[..end].to_vec()
}

/// `WorkspaceParser.registerIdentifiers` (JSON bases of `workspace extends`):
/// elements AND relationships carrying `structurizr.dsl.identifier` — the
/// property stores the FINAL (hierarchical-composite) key, so identifiers are
/// registered verbatim.
fn json_dsl_identifiers(ws: &Workspace) -> (DslIdentifiers, DslIdentifiers) {
    let mut items: Vec<&oxidizr_core::model::ModelItem> = Vec::new();
    let m = &ws.model;
    items.extend(m.custom_elements.iter().map(|e| &e.item));
    items.extend(m.people.iter().map(|e| &e.item));
    for s in &m.software_systems {
        items.push(&s.item);
        for c in &s.containers {
            items.push(&c.item);
            items.extend(c.components.iter().map(|k| &k.item));
        }
    }
    for n in &m.deployment_nodes {
        collect_json_item_refs(n, &mut items);
    }
    let elements = items
        .iter()
        .filter_map(|item| {
            item.properties
                .get("structurizr.dsl.identifier")
                .map(|s| (s.to_string(), item.id.clone()))
        })
        .collect();
    let relationships = m
        .relationships
        .iter()
        .filter_map(|r| {
            r.properties
                .get("structurizr.dsl.identifier")
                .map(|s| (s.to_string(), r.id.clone()))
        })
        .collect();
    (elements, relationships)
}

/// Deployment-subtree `ModelItem` refs for [`json_dsl_identifiers`].
fn collect_json_item_refs<'a>(
    n: &'a oxidizr_core::model::DeploymentNode,
    out: &mut Vec<&'a oxidizr_core::model::ModelItem>,
) {
    out.push(&n.item);
    out.extend(n.infrastructure_nodes.iter().map(|i| &i.item));
    out.extend(n.software_system_instances.iter().map(|i| &i.item));
    out.extend(n.container_instances.iter().map(|i| &i.item));
    for child in &n.children {
        collect_json_item_refs(child, out);
    }
}

/// Parse a DSL source (main-file entry, `parse(String, File)`).
///
/// # Errors
///
/// Returns a fatal statement-level [`DslError`] when `source` is empty;
/// per-statement problems are collected in [`ParseResult::errors`] as the
/// reference does (D1 recovery), not returned here.
pub fn parse_dsl(source: &str, opts: ParseOptions<'_>) -> Result<ParseResult, DslError> {
    if source.is_empty() {
        // reference: StringUtils.isNullOrEmpty(dsl) → throw
        return Err(DslError::stmt("A DSL fragment must be specified"));
    }
    let mut parser = Parser::new(opts);
    parser.parse(source);
    Ok(parser.finish())
}

impl Parser<'_> {
    /// Top-level: preprocess physical lines, then drive the parse loop.
    pub(crate) fn parse(&mut self, source: &str) {
        let logical = crate::preprocess::preprocess(source);
        self.parse_lines(&logical, source, true);
    }
}

// `oxidizr_core::CoreError` display texts are the reference messages by
// construction; sections convert with this shorthand.
impl From<&CoreError> for DslError {
    fn from(e: &CoreError) -> Self {
        DslError::core(e)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn base64_matches_java_encoder() {
        // Java Base64.getEncoder() vectors, incl. the golden `structurizr.dsl`
        // sample from the 2026-11-14 oracle probe.
        assert_eq!(base64_std(""), "");
        assert_eq!(base64_std("f"), "Zg==");
        assert_eq!(base64_std("fo"), "Zm8=");
        assert_eq!(base64_std("foo"), "Zm9v");
        assert_eq!(base64_std("foob"), "Zm9vYg==");
        assert_eq!(
            base64_std(
                "workspace \"W\" {\n  model {\n    p = person \"P\"\n    s = softwareSystem \"S\"\n    p -> s \"Uses\"\n  }\n}\n"
            ),
            "d29ya3NwYWNlICJXIiB7CiAgbW9kZWwgewogICAgcCA9IHBlcnNvbiAiUCIKICAgIHMgPSBzb2Z0d2FyZVN5c3RlbSAiUyIKICAgIHAgLT4gcyAiVXNlcyIKICB9Cn0K"
        );
    }

    #[test]
    fn retained_dsl_drops_trailing_newlines() {
        // M1-e A/B probe 2026-11-17 (`m1d` witness): the reference splits the
        // source with Java `split("\\r?\\n")` — the file's final newline can
        // never enter the retained `structurizr.dsl`, and trailing blank
        // lines drop out too. Vendored fixtures end `}` without a final
        // newline, so only this shape pins the behavior.
        let dir = std::env::temp_dir().join(format!("oxidizr-m1e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ws.dsl");
        let text = "workspace {\n  model {\n  }\n}\n\n";
        std::fs::write(&path, text).unwrap();
        let r = parse_dsl(text, ParseOptions::file(&path.to_string_lossy())).unwrap();
        assert_eq!(
            r.workspace.properties.get("structurizr.dsl"),
            Some(base64_std("workspace {\n  model {\n  }\n}").as_str())
        );
    }

    #[test]
    fn substitute_strings_semantics() {
        let mut p = Parser::new(ParseOptions::fragment());
        p.constants.insert(
            "name".to_string(),
            NameValue {
                name: "name".into(),
                value: "X".into(),
                vtype: NameValueType::Constant,
            },
        );
        p.constants.insert(
            "twice".to_string(),
            NameValue {
                name: "twice".into(),
                value: "${name} ${name}".into(),
                vtype: NameValueType::Constant,
            },
        );
        assert_eq!(p.substitute_strings("a${name}b"), "aXb");
        // occurrences come from the ORIGINAL token; replace is ALL:
        assert_eq!(p.substitute_strings("${name}-${name}"), "X-X");
        // a Constant's value is substituted RAW (Java only re-substitutes
        // TextBlocks) and the replacement is never rescanned — so `${twice}`
        // yields its literal value, `${name}` references and all:
        assert_eq!(p.substitute_strings("${twice}"), "${name} ${name}");
        // replacement-introduced `${…}` is NOT re-substituted (Java Matcher
        // scans the original): ${x}'s value below contains ${name}
        p.constants.insert(
            "ind".to_string(),
            NameValue {
                name: "ind".into(),
                value: "${name}".into(),
                vtype: NameValueType::Constant,
            },
        );
        assert_eq!(p.substitute_strings("${ind}"), "${name}");
        // inner match inside a non-match: the Matcher finds `${name}` and
        // Java's literal `replace` rewrites it even though it sits inside
        // the (unmatched) `${nope_…}` text:
        assert_eq!(p.substitute_strings("${nope_${name}}"), "${nope_X}");
        assert_eq!(p.substitute_strings("plain"), "plain");
    }

    #[test]
    fn env_lookup_override_is_hermetic() {
        // Phase 1T P1T-D1: with an injected lookup, `${VAR}` resolution must
        // never touch the process environment (hermetic in-process runs).
        fn fake_env(name: &str) -> Option<String> {
            (name == "OXZ_SEAM").then(|| "injected".to_string())
        }
        let opts = ParseOptions {
            file: None,
            base_dir: None,
            json_loader: None,
            env_lookup: Some(EnvLookup::new(fake_env)),
            installed_themes: None,
        };
        let p = Parser::new(opts);
        assert_eq!(p.substitute_strings("${OXZ_SEAM}"), "injected");
        // Absent from the map ⇒ not substituted — even though `PATH` IS set
        // in the process env: the lookup replaces it, never layers on it.
        assert_eq!(p.substitute_strings("${PATH}"), "${PATH}");
    }

    #[test]
    fn java_class_names() {
        assert_eq!(
            java_class_name(&Ctx::Model { group: None }),
            "ModelDslContext"
        );
        assert_eq!(
            java_class_name(&Ctx::NoRelInEnv {
                environment: "Live".into(),
                linked: Id::new("1"),
                group: None,
            }),
            "NoRelationshipInDeploymentEnvironmentDslContext"
        );
    }

    #[test]
    fn java_split_comma_trailing_empties_dropped() {
        assert_eq!(split_keeping_lead_dropping_trail("a,b,"), vec!["a", "b"]);
        assert_eq!(split_keeping_lead_dropping_trail(",a,"), vec!["", "a"]);
        assert_eq!(split_keeping_lead_dropping_trail(","), Vec::<&str>::new());
        assert_eq!(split_keeping_lead_dropping_trail(""), Vec::<&str>::new());
        assert_eq!(
            split_keeping_lead_dropping_trail("a,,b"),
            vec!["a", "", "b"]
        );
    }

    #[test]
    fn sequence_number_parallel_semantics() {
        // SequenceNumberTests.parallelSequences_1, mirrored: a peek never
        // burns a number, only a commit does (Java evaluates `getNext()`
        // only when the final add actually runs).
        let v = ViewRef::Dynamic(0);
        let mut p = Parser::new(ParseOptions::fragment());
        assert_eq!(p.seq_peek(v), "1");
        p.seq_commit(v);
        p.seq_start_parallel(v);
        assert_eq!(p.seq_peek(v), "2");
        p.seq_commit(v);
        p.seq_end_parallel(v, false);
        p.seq_start_parallel(v);
        assert_eq!(p.seq_peek(v), "2");
        p.seq_commit(v);
        p.seq_end_parallel(v, true);
        assert_eq!(p.seq_peek(v), "3");
        // parallelSequences_2 shape: nested, empty inner block burns nothing
        let mut p = Parser::new(ParseOptions::fragment());
        assert_eq!(p.seq_peek(v), "1");
        p.seq_commit(v);
        p.seq_start_parallel(v);
        p.seq_start_parallel(v);
        // the innermost frame was seeded from its parent and never committed
        p.seq_end_parallel(v, false);
        p.seq_end_parallel(v, true);
        assert_eq!(p.seq_peek(v), "3");
    }
}
