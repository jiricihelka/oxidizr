//! Seeded corpus mutations: `cargo xtask e2e mutate [SEED]` (P1T-D4). Pure,
//! deterministic generation — a splitmix64 walk over the valid corpus (sorted
//! discovery order), one text op per mutation, each op aiming at one failure
//! class: duplicate/drop a brace (OXZ-2003/2001), drop a required token,
//! rename a relationship endpoint, duplicate a top-level name (OXZ-2002).
//! The same seed re-runs byte-identically (V-T3). Output is COMMITTED under
//! `corpus/mutated/` (hermetic + diff-reviewable) and blessed once like any
//! other entry — the sidecars record the real codes, the ops only target.

use std::fs;
use std::path::Path;

use oxidizr_api::{ExitClass, Input, Request, execute};

use crate::corpus::{Entry, Kind};
use crate::{corpus_root, hermetic_options};

/// Mutations generated per op (× the number of ops = total mutations).
const PER_OP: usize = 24;

/// One generated mutation: the committed directory name, the op, the source
/// entry and the mutated `workspace.dsl` text.
#[derive(Debug, Clone)]
pub struct Mutation {
    /// Directory name under `corpus/mutated/` (`m042-drop-brace-17-…`).
    pub name: String,
    /// Op label (part of the name; kept for reports).
    pub op: &'static str,
    /// Source entry directory name.
    pub source: String,
    /// The mutated DSL text.
    pub text: String,
    /// Source entry directory (children are copied alongside).
    pub source_dir: std::path::PathBuf,
}

/// splitmix64 — deterministic, std-only, plenty for fixture selection.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next_u64() % n.max(1) as u64).unwrap_or(0)
    }
}

type Op = fn(&str, &mut Rng) -> Option<String>;

/// (label, op) table. Mutation `i` uses entry `i` and op `i % len`.
const OPS: &[(&str, Op)] = &[
    ("drop-brace", op_drop_brace),
    ("dup-brace", op_dup_brace),
    ("drop-equals", op_drop_equals),
    ("ghost-rel", op_ghost_rel),
    ("dup-name", op_dup_name),
];

/// The candidate pick every op ends with (2R R-tool-04): an EMPTY list draws
/// NOTHING, a non-empty one exactly one `rng.below` — the same draw sequence
/// the five inline prologues made, so every seeded mutation (the blessed
/// `m<i>-<label>-…` corpus dirs) stays bit-identical.
fn pick_index(cands: &[usize], rng: &mut Rng) -> Option<usize> {
    if cands.is_empty() {
        return None;
    }
    Some(cands[rng.below(cands.len())])
}

/// The line-scanning ops' shared prologue (2R R-tool-04): split to lines,
/// collect the indices `pred` accepts, pick one — the preds are pure, so the
/// filter never touches the rng state. Each op keeps only its mutation tail.
fn pick_line(
    text: &str,
    rng: &mut Rng,
    pred: impl Fn(&str) -> bool,
) -> Option<(Vec<String>, usize)> {
    let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let cands: Vec<usize> = (0..lines.len()).filter(|&i| pred(&lines[i])).collect();
    let i = pick_index(&cands, rng)?;
    Some((lines, i))
}

/// Remove one `}` — the context it closed stays open ⇒ OXZ-2003 at EOF.
fn op_drop_brace(text: &str, rng: &mut Rng) -> Option<String> {
    let (mut lines, i) = pick_line(text, rng, |l| l.trim_end().ends_with('}'))?;
    let pos = lines[i].rfind('}')?;
    lines[i].remove(pos);
    Some(lines.join("\n"))
}

/// Duplicate one `}` line — one closing brace too many ⇒ OXZ-2001.
fn op_dup_brace(text: &str, rng: &mut Rng) -> Option<String> {
    let (mut lines, i) = pick_line(text, rng, |l| l.trim_end().ends_with('}'))?;
    lines.insert(i + 1, lines[i].clone());
    Some(lines.join("\n"))
}

/// Delete the top-level ` = ` of a definition — the identifier and keyword
/// run on unparsed ⇒ statement/lexer-class failure.
fn op_drop_equals(text: &str, rng: &mut Rng) -> Option<String> {
    let (mut lines, i) = pick_line(text, rng, |l| {
        !l.trim_start().starts_with('!') && equals_outside_quotes(l).is_some()
    })?;
    let pos = equals_outside_quotes(&lines[i])?;
    lines[i].replace_range(pos..=pos, "");
    Some(lines.join("\n"))
}

/// Rename one relationship endpoint to a never-defined identifier — the
/// reference's unknown-reference failure (OXZ-2002 class).
fn op_ghost_rel(text: &str, rng: &mut Rng) -> Option<String> {
    let (mut lines, i) = pick_line(text, rng, |l| {
        let toks: Vec<&str> = l.split_whitespace().collect();
        toks.contains(&"->")
            && toks.first().is_some_and(|t| is_bare_ident(t))
            && toks.last().is_some_and(|t| t.contains('"'))
    })?;
    let mut ghosted = false;
    let toks: Vec<String> = lines[i]
        .split_whitespace()
        .map(|t| {
            if !ghosted && t == "->" {
                ghosted = true;
                "->".to_owned()
            } else if ghosted {
                ghosted = false;
                "ghostRelRefQz".to_owned()
            } else {
                t.to_owned()
            }
        })
        .collect();
    lines[i] = toks.join(" ");
    Some(lines.join("\n"))
}

/// Duplicate a top-level person/software-system NAME under a fresh identifier
/// — the reference's duplicate-name rejection (OXZ-2002).
fn op_dup_name(text: &str, rng: &mut Rng) -> Option<String> {
    let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
    let mut depth: i32 = 0;
    let mut cands: Vec<usize> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if depth == 2 && !l.trim_start().starts_with('!') && l.ends_with('"') {
            let t = l.trim_start();
            if let Some(rest) = t.strip_prefix("person ") {
                if rest.starts_with('"') {
                    cands.push(i);
                }
            } else if let Some(rest) = t.strip_prefix("softwareSystem ")
                && rest.starts_with('"')
            {
                cands.push(i);
            }
        }
        depth = delta_depth(l, depth);
    }
    let i = pick_index(&cands, rng)?; // depth scan stays op-local
    // `person "Name" "desc"` (or without desc) — re-issue it with a fresh
    // (never-taken) identifier and the SAME name.
    let indent: String = lines[i].chars().take_while(|c| c.is_whitespace()).collect();
    let body = lines[i].trim_start();
    let name_end = body.rfind('"').unwrap_or(body.len());
    let dup = format!("{indent}personDupQz = {}", &body[..=name_end]);
    lines.insert(i + 1, dup);
    Some(lines.join("\n"))
}

/// Byte index of the first `=` outside a quoted string that sits between an
/// identifier and a space (the definition form `id = kind …`).
fn equals_outside_quotes(line: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            '=' if !in_quote => {
                let prev_ident = line[..i]
                    .chars()
                    .next_back()
                    .is_some_and(|p| p.is_ascii_alphanumeric() || p == '_');
                if prev_ident && line[i + 1..].starts_with(' ') {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// A bare DSL identifier token (no quotes / braces / punctuation).
fn is_bare_ident(t: &str) -> bool {
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Line brace delta outside quotes, applied to the running depth.
fn delta_depth(line: &str, depth: i32) -> i32 {
    let mut d = depth;
    let mut in_quote = false;
    for c in line.chars() {
        match c {
            '"' => in_quote = !in_quote,
            '{' if !in_quote => d += 1,
            '}' if !in_quote => d -= 1,
            _ => {}
        }
    }
    d
}

/// Generate all mutations for `seed` (pure — nothing is written). Mutation
/// `i` mutates discovery-sorted valid entry `i` with op `i % OPS.len()`;
/// when an op finds no candidate line the next ops are tried in rotation.
///
/// # Errors
///
/// Corpus discovery failures, or fewer valid DSL entries than mutations
/// (the corpus must back the full op matrix).
pub fn generate(seed: u64) -> Result<Vec<Mutation>, String> {
    let entries = crate::corpus::discover()?;
    let pool: Vec<&Entry> = entries
        .iter()
        .filter(|e| e.kind == Kind::Valid && e.dsl.is_some())
        .collect();
    let total = PER_OP * OPS.len();
    if pool.len() < total {
        return Err(format!(
            "mutate needs ≥ {total} valid DSL entries, corpus has {}",
            pool.len()
        ));
    }
    let mut rng = Rng(seed);
    let mut out = Vec::with_capacity(total);
    for i in 0..total {
        let entry = pool[i];
        let Some(dsl) = entry.dsl.as_ref() else {
            return Err(format!("entry {} has no workspace.dsl", entry.name));
        };
        let text = fs::read_to_string(dsl).map_err(|e| format!("{}: {e}", dsl.display()))?;
        let mut done: Option<(&'static str, String)> = None;
        for k in 0..OPS.len() {
            let (label, op) = OPS[(i + k) % OPS.len()];
            if let Some(mutated) = op(&text, &mut rng)
                && mutated != text
            {
                done = Some((label, mutated));
                break;
            }
        }
        let Some((op, mutated)) = done else {
            return Err(format!(
                "mutation source {} admits no op — corpus degenerate",
                entry.name
            ));
        };
        out.push(Mutation {
            name: format!("m{i:03}-{op}-{}", entry.name),
            op,
            source: entry.name.clone(),
            text: mutated,
            source_dir: entry.dir.clone(),
        });
    }
    Ok(out)
}

/// Generate `seed`'s mutations, write them under `corpus/mutated/` (children
/// copied, sidecars excluded), and VERIFY every mutation really fails
/// `dsl2json` in-process — a mutation that still parses clean is a broken op,
/// not a fixture. The previous mutated corpus is replaced wholesale.
///
/// # Errors
///
/// [`generate`] failures, I/O failures, or a mutation that does not fail.
pub fn write_corpus(seed: u64) -> Result<Vec<String>, String> {
    let mutations = generate(seed)?;
    let root = corpus_root().join(Kind::Mutated.dir_name());
    if root.exists() {
        fs::remove_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    }
    fs::create_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;

    let opts = hermetic_options();
    let mut reports = Vec::new();
    for m in &mutations {
        let dir = root.join(&m.name);
        fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        copy_children(&m.source_dir, &dir)?;
        let dsl = dir.join("workspace.dsl");
        fs::write(&dsl, m.text.as_bytes()).map_err(|e| format!("{}: {e}", dsl.display()))?;

        let out = execute(
            Request::Dsl2Json {
                input: Input::File(dsl),
                output: None,
            },
            &opts,
        );
        if out.exit == ExitClass::Ok {
            return Err(format!(
                "mutation {} (op {}) still parses clean — op is broken, not a fixture",
                m.name, m.op
            ));
        }
        reports.push(format!(
            "{}/{}: {} ({})",
            Kind::Mutated.dir_name(),
            m.name,
            m.op,
            m.source
        ));
    }
    Ok(reports)
}

/// Copy every child file/directory of a source entry except its sidecar.
fn copy_children(src: &Path, dst: &Path) -> Result<(), String> {
    for item in fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))? {
        let item = item.map_err(|e| format!("{}: {e}", src.display()))?;
        let from = item.path();
        let file_name = from.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if file_name == crate::manifest::FILE_NAME || file_name == "workspace.dsl" {
            continue;
        }
        let to = dst.join(file_name);
        if from.is_dir() {
            fs::create_dir_all(&to).map_err(|e| format!("{}: {e}", to.display()))?;
            copy_children(&from, &to)?;
        } else {
            fs::copy(&from, &to)
                .map_err(|e| format!("{} -> {}: {e}", from.display(), to.display()))?;
        }
    }
    Ok(())
}
