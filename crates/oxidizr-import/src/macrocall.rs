//! Shared plumbing for the macro-language importers (F12, §P4-M2 N-2/N-3):
//! PlantUML/Mermaid-flavored comment stripping, a small `!$var`
//! preprocessor, logical-line joining and quote-aware macro-call parsing.
//!
//! The grammar handled here is deliberately a SUBSET (P4-D8, R-1):
//! one macro call per logical line, `"..."` string arguments with `~"`
//! as the literal-quote escape, positional optional arguments mixed with
//! `$key = value` keyword arguments (the official C4-PlantUML calling
//! style), and `'` / `/' … '/` / `rem` comments.

use std::collections::HashMap;

/// Removes `PlantUML` comments: `'` to end of line (and whole `rem` lines),
/// and `/' … '/` block comments possibly spanning lines. `'` is NOT a
/// comment start inside double-quoted strings, nor on `!`-directive
/// lines, where preprocessor literals like `'enterprise'` use apostrophes.
#[must_use]
pub fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut in_block = false;
    let mut directive_line = false;
    while i < chars.len() {
        let c = chars[i];
        if in_block {
            // Only the `'/` terminator matters inside a block comment;
            // newlines are kept so line numbers stay meaningful.
            if c == '\'' && i + 1 < chars.len() && chars[i + 1] == '/' {
                in_block = false;
                i += 2;
            } else {
                if c == '\n' {
                    out.push(c);
                    directive_line = false;
                }
                i += 1;
            }
            continue;
        }
        if c == '\n' {
            out.push(c);
            directive_line = false;
            i += 1;
            continue;
        }
        // First non-space character of a fresh line decides `'` handling.
        if (out.is_empty() || out.ends_with('\n')) && !c.is_whitespace() {
            directive_line = c == '!';
        }
        // Whole-line `rem` comment (PlantUML's second comment form).
        if !directive_line
            && (out.is_empty() || out.ends_with('\n'))
            && text_at(&chars, i, "rem")
            && !matches!(chars.get(i + 3), Some(x) if x.is_alphanumeric())
        {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        match c {
            '~' if chars.get(i + 1) == Some(&'"') => {
                out.push('~');
                out.push('"');
                i += 2;
            }
            '"' => {
                out.push(c);
                i += 1;
                // Strings are copied verbatim to their closing quote
                // (a `~"` inside stays; an unterminated string ends at EOL).
                while i < chars.len() && chars[i] != '\n' {
                    if chars[i] == '~' && chars.get(i + 1) == Some(&'"') {
                        out.push('~');
                        out.push('"');
                        i += 2;
                        continue;
                    }
                    if chars[i] == '"' {
                        out.push('"');
                        i += 1;
                        break;
                    }
                    out.push(chars[i]);
                    i += 1;
                }
            }
            '\'' if !directive_line => {
                // Line comment: drop to (not including) the newline.
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
            }
            '/' if chars.get(i + 1) == Some(&'\'') => {
                in_block = true;
                i += 2;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

fn text_at(chars: &[char], i: usize, word: &str) -> bool {
    word.chars()
        .enumerate()
        .all(|(k, c)| chars.get(i + k) == Some(&c))
}

/// The tiny preprocessor the macro importers tolerate: `!$name = value`
/// variable definitions (with `?=` set-if-absent and `+=` append), and
/// silent tolerance of every other `!` directive — `!include`, `!theme`,
/// `!if`/`!endif`, and whole `!procedure`/`!function`/`!definelong`/
/// `!sub` blocks (their bodies never reach the macro classifier).
#[derive(Default)]
pub struct Preprocessor {
    vars: HashMap<String, String>,
    in_def: bool,
}

impl Preprocessor {
    /// Feeds one (comment-stripped) line. `None` = consumed as a
    /// directive; `Some(line)` = a content line with `$variables`
    /// substituted.
    pub fn feed(&mut self, line: &str) -> Option<String> {
        let trimmed = line.trim_start();
        if self.in_def {
            // Inside a definition block every line is consumed —
            // content lines included (they define the macro, they are
            // not diagram statements).
            if trimmed.starts_with("!end") {
                self.in_def = false;
            }
            return None;
        }
        let Some(rest) = trimmed.strip_prefix('!') else {
            return Some(self.substitute(line));
        };
        let directive = rest.trim_start();
        for kw in ["procedure", "function", "definelong", "sub"] {
            if directive.starts_with(kw)
                && !matches!(directive[kw.len()..].chars().next(), Some(c) if c.is_alphanumeric())
            {
                self.in_def = true;
                return None;
            }
        }
        if let Some(v) = directive.strip_prefix('$') {
            let name: String = v
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            let after = v[name.len()..].trim_start();
            let op = ["?=", "+=", "="].into_iter().find(|o| after.starts_with(o));
            if let (false, Some(o)) = (name.is_empty(), op) {
                let value = self.substitute(&unquote_str(after[o.len()..].trim()));
                match o {
                    "?=" => {
                        self.vars.entry(name).or_insert(value);
                    }
                    "+=" => match self.vars.get_mut(&name) {
                        Some(existing) => existing.push_str(&value),
                        None => {
                            self.vars.insert(name, value);
                        }
                    },
                    _ => {
                        self.vars.insert(name, value);
                    }
                }
                return None;
            }
        }
        None
    }

    /// Replaces every `$name` whose name is defined (maximal identifier
    /// run, so `$a` inside `$ab` is never a partial hit).
    #[must_use]
    pub fn substitute(&self, s: &str) -> String {
        if self.vars.is_empty() || !s.contains('$') {
            return s.to_owned();
        }
        let chars: Vec<char> = s.chars().collect();
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '$' {
                let name: String = chars[i + 1..]
                    .iter()
                    .take_while(|c| c.is_ascii_alphanumeric() || **c == '_')
                    .collect();
                if let Some(value) = self.vars.get(&name) {
                    out.push_str(value);
                    i += 1 + name.chars().count();
                    continue;
                }
            }
            out.push(chars[i]);
            i += 1;
        }
        out
    }
}

/// Paren balance of a line, quote-aware (`~"` is a literal quote).
/// Stray `)` never drives the balance below zero.
#[must_use]
pub fn paren_balance(line: &str) -> i32 {
    let mut depth = 0_i32;
    let mut quoted = false;
    let mut prev_tilde = false;
    for c in line.chars() {
        if quoted {
            if c == '"' && !prev_tilde {
                quoted = false;
            }
            prev_tilde = c == '~';
            continue;
        }
        match c {
            '~' => prev_tilde = true,
            '"' if !prev_tilde => quoted = true,
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            _ => prev_tilde = false,
        }
    }
    depth
}

/// Joins physical lines into logical lines while a `(` stays open —
/// `Rel(a, b,\n "uses")` is one call. Joined parts are space-separated.
#[must_use]
pub fn join_logical_lines<'a>(lines: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut pending = String::new();
    for line in lines {
        if !pending.is_empty() && !pending.ends_with(' ') {
            pending.push(' ');
        }
        pending.push_str(line);
        // Re-balance over the WHOLE pending buffer (a lone `)` line
        // cannot be read as a negative balance of its own).
        if paren_balance(&pending) == 0 {
            if pending.trim().is_empty() {
                pending.clear();
            } else {
                out.push(std::mem::take(&mut pending));
            }
        }
    }
    if !pending.trim().is_empty() {
        out.push(pending);
    }
    out
}

/// One parsed macro call: `Name(pos0, pos1, …, $key = value, …)`.
#[derive(Debug, Clone)]
pub struct Call {
    /// Macro name exactly as spelled (case preserved).
    pub name: String,
    /// Positional arguments, unquoted and trimmed.
    pub positional: Vec<String>,
    /// `$key = value` keyword arguments (in source order; first wins).
    pub kwargs: Vec<(String, String)>,
}

impl Call {
    /// Parses a call at the start of `text`; returns the call plus the
    /// byte index just past its closing `)`. `None` when `text` does not
    /// start with `identifier(` or the parens never balance.
    #[must_use]
    pub fn parse(text: &str) -> Option<(Call, usize)> {
        let start = text.len() - text.trim_start().len();
        let t = &text[start..];
        let open = t.find('(')?;
        let name = &t[..open];
        let mut chars = name.chars();
        let valid = !name.is_empty()
            && chars.next().is_some_and(|c| c.is_ascii_alphabetic())
            && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            return None;
        }
        // Match the closing ')' quote-aware over char indices.
        let mut depth = 0_usize;
        let mut quoted = false;
        let mut prev_tilde = false;
        let mut close = None;
        for (idx, c) in t.char_indices() {
            if idx < open {
                continue;
            }
            if quoted {
                if c == '"' && !prev_tilde {
                    quoted = false;
                }
                prev_tilde = c == '~';
                continue;
            }
            match c {
                '~' => prev_tilde = true,
                '"' if !prev_tilde => quoted = true,
                '(' => {
                    if idx == open {
                        depth = 1;
                    } else {
                        depth += 1;
                    }
                    prev_tilde = false;
                }
                ')' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(idx);
                        break;
                    }
                    prev_tilde = false;
                }
                _ => prev_tilde = false,
            }
        }
        let close = close?;
        let (positional, kwargs) = split_args(&t[open + 1..close]);
        Some((
            Call {
                name: name.to_owned(),
                positional,
                kwargs,
            },
            start + close + ')'.len_utf8(),
        ))
    }

    /// The value of argument `idx` with keyword name `key` — the keyword
    /// form wins over the positional slot, empty values read as `None`
    /// (C4-PlantUML defaults every optional to `""`).
    #[must_use]
    pub fn arg(&self, idx: usize, key: &str) -> Option<&str> {
        let raw = self.kwargs.iter().find(|(k, _)| k == key).map_or_else(
            || self.positional.get(idx).map(String::as_str),
            |(_, v)| Some(v.as_str()),
        );
        raw.map(str::trim).filter(|s| !s.is_empty())
    }

    /// Positional argument `idx` verbatim (aliases are unquoted),
    /// empty ⇒ `None`.
    #[must_use]
    pub fn pos(&self, idx: usize) -> Option<&str> {
        self.positional
            .get(idx)
            .map(String::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }
}

/// Splits the inside of a call on top-level commas (quote-aware), then
/// separates `$key = value` kwargs from positional arguments.
fn split_args(inner: &str) -> (Vec<String>, Vec<(String, String)>) {
    let mut pieces: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut prev_tilde = false;
    for c in inner.chars() {
        if quoted {
            current.push(c);
            if c == '"' && !prev_tilde {
                quoted = false;
            }
            prev_tilde = c == '~';
            continue;
        }
        match c {
            '~' => {
                current.push(c);
                prev_tilde = true;
            }
            '"' if !prev_tilde => {
                current.push(c);
                quoted = true;
            }
            ',' => {
                pieces.push(std::mem::take(&mut current));
                prev_tilde = false;
            }
            _ => {
                current.push(c);
                prev_tilde = false;
            }
        }
    }
    pieces.push(current);
    let mut positional = Vec::new();
    let mut kwargs = Vec::new();
    for piece in pieces {
        let piece = piece.trim().to_owned();
        if let Some(rest) = piece.strip_prefix('$') {
            let key: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !key.is_empty() && rest[key.len()..].trim_start().strip_prefix('=').is_some() {
                let value = rest[key.len()..].trim_start()[1..].trim();
                kwargs.push((key, unquote_str(value)));
                continue;
            }
        }
        positional.push(unquote_str(&piece));
    }
    (positional, kwargs)
}

/// Strips a surrounding pair of double quotes (if present) and undoes the
/// `PlantUML` `~"` literal-quote escape.
#[must_use]
pub fn unquote_str(s: &str) -> String {
    let inner = s.strip_prefix('"').and_then(|x| x.strip_suffix('"'));
    match inner {
        Some(inner) => inner.replace("~\"", "\""),
        None => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn comments_stripped_where_plantuml_strips_them() {
        let src = "Person(p, \"It's me\") ' a comment\n/' block start\nSystem(s, \"S\") '/\nrem gone\n!$x = 'keep'\n";
        let lines: Vec<String> = strip_comments(src)
            .lines()
            .map(|l| l.trim_end().to_owned())
            .collect();
        // The block comment SWALLOWS the System line (it ends at `'/`),
        // `'` never comments inside a string, `rem` lines vanish and
        // `!` lines keep apostrophe literals. Newlines survive, so the
        // swallowed/commented lines stay as blanks.
        assert_eq!(
            lines,
            ["Person(p, \"It's me\")", "", "", "", "!$x = 'keep'"]
        );
    }

    #[test]
    fn preprocessor_variables_and_directives() {
        let mut pp = Preprocessor::default();
        assert_eq!(pp.feed("!include C4_Container.puml"), None);
        assert_eq!(pp.feed("!$who = \"Customer\""), None);
        assert_eq!(pp.feed("!$who ?= \"Other\""), None);
        assert_eq!(
            pp.feed("Person(p, $who)").as_deref(),
            Some("Person(p, Customer)")
        );
        assert_eq!(pp.feed("!procedure $custom($a)"), None);
        assert_eq!(pp.feed("Person(evil, \"inside\")"), None);
        assert_eq!(pp.feed("!endprocedure"), None);
        assert_eq!(
            pp.feed("Person(ok, \"yes\")").as_deref(),
            Some("Person(ok, \"yes\")")
        );
        // Unknown identifiers survive substitution untouched.
        assert_eq!(
            pp.feed("Rel(p, $other, \"x\")").as_deref(),
            Some("Rel(p, $other, \"x\")")
        );
    }

    #[test]
    fn logical_lines_join_open_parens() {
        let joined = join_logical_lines(["Rel(a, b,", "\"uses\", \"HTTPS\")", "Person(p, \"P\")"]);
        assert_eq!(
            joined,
            ["Rel(a, b, \"uses\", \"HTTPS\")", "Person(p, \"P\")"]
        );
    }

    #[test]
    fn call_parse_handles_quotes_escapes_and_kwargs() {
        let (call, end) =
            Call::parse("  Rel_U(a, b, \"uses, really\", $techn=\"HTTPS ~\"X~\"\") {").unwrap();
        assert_eq!(call.name, "Rel_U");
        assert_eq!(call.pos(0), Some("a"));
        assert_eq!(call.pos(2), Some("uses, really"));
        // `~"…~"` is the literal-quote escape pair INSIDE strings.
        assert_eq!(call.arg(3, "techn"), Some("HTTPS \"X\""));
        assert_eq!(
            &"  Rel_U(a, b, \"uses, really\", $techn=\"HTTPS ~\"X~\"\") {"[..end],
            "  Rel_U(a, b, \"uses, really\", $techn=\"HTTPS ~\"X~\"\")"
        );
        // Not a call:
        assert!(Call::parse("skinparam param 1").is_none());
        assert!(Call::parse("2bad(x)").is_none());
        assert!(Call::parse("open(a").is_none());
    }

    #[test]
    fn kwarg_precedence_and_empty_slots() {
        let (call, _) = Call::parse("System(s, \"S\", \"\", $descr=\"D\")").unwrap();
        // Empty positional slots read as None (C4 defaults every optional
        // to ""), and the keyword form beats the positional one.
        assert_eq!(call.arg(2, "descr"), Some("D"));
        let (call, _) = Call::parse("System(s, \"S\", \"plain\")").unwrap();
        assert_eq!(call.arg(2, "descr"), Some("plain"));
    }
}
